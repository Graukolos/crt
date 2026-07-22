use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::ast::Actor;
use crate::codegen::common::{
    actor_mod, emit_actor, emit_shared_decls, ident, inst_var, instance_args, port_ref, rust_type,
    type_ident,
};
use crate::codegen::{CodeGenerator, Program};
use crate::network_ffi::ffi::Instance;

pub struct Tokio {
    pub cap: usize,
}

const TERM_RS: &str = r"use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

struct Term {
    inner: Mutex<(usize, usize, usize)>,
    token: CancellationToken,
}

impl Term {
    fn new(total: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new((0, total, 0)),
            token: CancellationToken::new(),
        })
    }
    fn check(state: &(usize, usize, usize), token: &CancellationToken) {
        let (idle, alive, inflight) = *state;
        if inflight == 0 && idle >= alive {
            token.cancel();
        }
    }
    fn sent(&self) {
        self.inner.lock().unwrap().2 += 1;
    }
    fn got(&self) {
        self.inner.lock().unwrap().2 -= 1;
    }
    fn enter_idle(&self) {
        let mut state = self.inner.lock().unwrap();
        state.0 += 1;
        Self::check(&state, &self.token);
    }
    fn leave_idle(&self) {
        self.inner.lock().unwrap().0 -= 1;
    }
    fn finish(&self) {
        let mut state = self.inner.lock().unwrap();
        state.1 -= 1;
        Self::check(&state, &self.token);
    }
}
";

impl CodeGenerator for Tokio {
    fn name(&self) -> &'static str {
        "tokio"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path, orcc: bool) -> io::Result<()> {
        let src_dir = out_dir.join("src");
        for (name, source) in emit_files(program, self.cap) {
            let tokens = source.parse().map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "generated source for {name} failed to tokenize: {err}\n--- source ---\n{source}"
                    ),
                )
            })?;
            super::write_rust(&src_dir.join(&name), tokens)?;
        }
        let deps = "tokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"sync\"] }\ntokio-util = \"0.7\"\n";
        super::write_cargo_toml(
            out_dir,
            &program.network.name,
            program.has_natives(),
            deps,
            orcc,
        )?;
        if program.has_natives() {
            super::write_native_support(out_dir, program.native_sources, orcc)?;
        }
        Ok(())
    }
}

fn emit_files(program: &Program<'_>, cap: usize) -> Vec<(String, String)> {
    let unbounded = cap == 0;
    let mut files = Vec::new();

    let mut classes: Vec<&String> = program.actors.keys().collect();
    classes.sort();

    for class in &classes {
        let actor = &program.actors[*class];
        let mut src = String::new();
        src.push_str("#![allow(warnings)]\n");
        src.push_str("use std::collections::VecDeque;\n");
        src.push_str("use std::sync::Arc;\n");
        if unbounded {
            src.push_str("use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};\n");
        } else {
            src.push_str("use tokio::sync::mpsc::{Receiver, Sender};\n");
        }
        src.push_str("use super::*;\n\n");
        src.push_str(&emit_actor(actor));
        src.push('\n');
        src.push_str(&emit_task_run(actor, unbounded));
        files.push((format!("{}.rs", actor_mod(&actor.name)), src));
    }

    let mut main = String::new();
    main.push_str("#![allow(warnings)]\n\n");
    for class in &classes {
        let actor = &program.actors[*class];
        let _ = writeln!(main, "mod {};", actor_mod(&actor.name));
    }
    main.push('\n');
    if !unbounded {
        let _ = writeln!(main, "const CAP: usize = {cap};\n");
    }
    main.push_str(TERM_RS);
    main.push('\n');
    main.push_str(&emit_shared_decls(program));
    main.push_str(&emit_main(program, unbounded));
    files.push(("main.rs".to_string(), main));

    files
}

fn fire_args(actor: &Actor) -> String {
    actor
        .inports
        .iter()
        .chain(&actor.outports)
        .map(|p| format!("&mut {}", port_ref(&p.name)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn emit_flush(actor: &Actor, unbounded: bool) -> String {
    let send_await = if unbounded { "" } else { ".await" };
    let mut out = String::new();
    for p in &actor.outports {
        let buf = port_ref(&p.name);
        let id = ident(&p.name);
        let _ = writeln!(
            out,
            "if !{buf}.is_empty() {{ \
             let __chunk: Vec<_> = {buf}.drain(..).collect(); \
             for __tx in &tx_{id} {{ term.sent(); let _ = __tx.send(__chunk.clone()){send_await}; }} }}"
        );
    }
    out
}

fn emit_task_run(actor: &Actor, unbounded: bool) -> String {
    let (rx_ty, tx_ty) = if unbounded {
        ("UnboundedReceiver", "UnboundedSender")
    } else {
        ("Receiver", "Sender")
    };
    let ty = type_ident(&actor.name);
    let run = format!("run_{}", ident(&actor.name));

    let mut params = vec![format!("mut __actor: {ty}"), "term: Arc<Term>".to_string()];
    for p in &actor.inports {
        params.push(format!(
            "mut rx_{}: {rx_ty}<Vec<{}>>",
            ident(&p.name),
            rust_type(&p.typ)
        ));
    }
    for p in &actor.outports {
        params.push(format!(
            "tx_{}: Vec<{tx_ty}<Vec<{}>>>",
            ident(&p.name),
            rust_type(&p.typ)
        ));
    }
    let sig = params.join(", ");
    let args = fire_args(actor);
    let flush = emit_flush(actor, unbounded);

    let mut body = String::new();
    for p in actor.inports.iter().chain(&actor.outports) {
        let _ = writeln!(
            body,
            "let mut {}: VecDeque<{}> = VecDeque::new();",
            port_ref(&p.name),
            rust_type(&p.typ)
        );
    }
    for p in &actor.inports {
        let _ = writeln!(body, "let mut open_{} = true;", ident(&p.name));
    }

    if actor.init.is_some() {
        let _ = writeln!(body, "__actor.init({args});");
        body.push_str(&flush);
    }

    if actor.inports.is_empty() {
        let _ = writeln!(body, "while __actor.fire({args}) {{\n{flush}}}");
    } else {
        body.push_str("loop {\n");
        let _ = writeln!(body, "while __actor.fire({args}) {{\n{flush}}}");
        let all_closed = actor
            .inports
            .iter()
            .map(|p| format!("!open_{}", ident(&p.name)))
            .collect::<Vec<_>>()
            .join(" && ");
        let _ = writeln!(body, "if {all_closed} {{ break; }}");
        body.push_str("if term.token.is_cancelled() { break; }\n");
        body.push_str("term.enter_idle();\n");
        body.push_str("tokio::select! {\n");
        body.push_str("biased;\n");
        body.push_str("_ = term.token.cancelled() => { term.leave_idle(); break; }\n");
        for p in &actor.inports {
            let id = ident(&p.name);
            let _ = writeln!(
                body,
                "__m = rx_{id}.recv(), if open_{id} => {{ term.leave_idle(); match __m {{ Some(__c) => {{ term.got(); {}.extend(__c); }} None => {{ open_{id} = false; }} }} }}",
                port_ref(&p.name)
            );
        }
        body.push_str("else => { term.leave_idle(); break; }\n");
        body.push_str("}\n");
        body.push_str("}\n");
    }
    body.push_str("term.finish();\n");

    format!("pub async fn {run}({sig}) {{\n{body}}}\n")
}

fn emit_main(program: &Program<'_>, unbounded: bool) -> String {
    let network = program.network;
    let instances: Vec<&Instance> = network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect();

    let mut out = String::new();
    out.push_str("#[tokio::main]\nasync fn main() {\n");

    if program.has_natives() {
        out.push_str(super::orcc::MAIN_SETUP);
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        for p in &actor.inports {
            let ctor = if unbounded {
                format!(
                    "tokio::sync::mpsc::unbounded_channel::<Vec<{}>>()",
                    rust_type(&p.typ)
                )
            } else {
                format!(
                    "tokio::sync::mpsc::channel::<Vec<{}>>(CAP)",
                    rust_type(&p.typ)
                )
            };
            let _ = writeln!(
                out,
                "    let (tx_{0}_{1}, rx_{0}_{1}) = {ctor};",
                ident(&inst.id),
                ident(&p.name),
            );
        }
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let ctor_args = instance_args(inst, actor);
        let _ = writeln!(
            out,
            "    let {} = {}::{}::new({ctor_args});",
            inst_var(&inst.id),
            actor_mod(&actor.name),
            type_ident(&actor.name)
        );
    }

    let _ = writeln!(out, "    let __term = Term::new({});", instances.len());
    out.push_str("    let mut __set = tokio::task::JoinSet::new();\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut args = vec![inst_var(&inst.id), "__term.clone()".to_string()];
        for p in &actor.inports {
            args.push(format!("rx_{}_{}", ident(&inst.id), ident(&p.name)));
        }
        for p in &actor.outports {
            let clones: Vec<String> = network
                .edges
                .iter()
                .filter(|e| e.src_id == inst.id && e.src_port == p.name)
                .map(|e| format!("tx_{}_{}.clone()", ident(&e.dst_id), ident(&e.dst_port)))
                .collect();
            args.push(format!("vec![{}]", clones.join(", ")));
        }
        let _ = writeln!(
            out,
            "    __set.spawn({}::run_{}({}));",
            actor_mod(&actor.name),
            ident(&actor.name),
            args.join(", ")
        );
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        for p in &actor.inports {
            let _ = writeln!(out, "    drop(tx_{}_{});", ident(&inst.id), ident(&p.name));
        }
    }

    out.push_str("    while __set.join_next().await.is_some() {}\n");
    out.push_str("}\n");
    out
}
