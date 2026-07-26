use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::ast::Actor;
use crate::codegen::common::{
    actor_mod, emit_actor, emit_shared_decls, ident, inst_var, instance_args, out_port_ctor,
    port_field, rust_type, type_ident,
};
use crate::codegen::{CodeGenerator, Program};
use crate::network_ffi::ffi::Instance;

pub struct Tokio {
    pub cap: usize,
}

impl CodeGenerator for Tokio {
    fn name(&self) -> &'static str {
        "tokio"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path, orcc: bool) -> io::Result<()> {
        let src_dir = out_dir.join("src");
        for (name, source) in emit_files(program, self.cap, orcc) {
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
        let deps =
            "tokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"sync\"] }\n";
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

fn emit_files(program: &Program<'_>, cap: usize, orcc: bool) -> Vec<(String, String)> {
    let unbounded = cap == 0;
    let mut files = Vec::new();

    let mut classes: Vec<&String> = program.actors.keys().collect();
    classes.sort();

    for class in &classes {
        let actor = &program.actors[*class];
        let mut src = String::new();
        src.push_str("#![allow(warnings)]\n");
        src.push_str("use std::collections::VecDeque;\n");
        src.push_str("use super::*;\n\n");
        src.push_str(&emit_actor(actor));
        src.push('\n');
        src.push_str(&emit_task_run(actor));
        files.push((format!("{}.rs", actor_mod(&actor.name)), src));
    }

    let mut main = String::new();
    main.push_str("#![allow(warnings)]\n");
    main.push_str("use std::collections::VecDeque;\n\n");
    for class in &classes {
        let actor = &program.actors[*class];
        let _ = writeln!(main, "mod {};", actor_mod(&actor.name));
    }
    main.push('\n');
    let _ = writeln!(main, "const CAP: usize = {cap};\n");
    main.push_str(&emit_ports(unbounded));
    main.push('\n');
    main.push_str(&emit_shared_decls(program, orcc));
    main.push_str(&emit_main(program, unbounded));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_ports(unbounded: bool) -> String {
    let (tx_ty, rx_ty, send_await) = if unbounded {
        (
            "tokio::sync::mpsc::UnboundedSender",
            "tokio::sync::mpsc::UnboundedReceiver",
            "",
        )
    } else {
        (
            "tokio::sync::mpsc::Sender",
            "tokio::sync::mpsc::Receiver",
            ".await",
        )
    };
    format!(
        r"pub type Tx<T> = {tx_ty}<Vec<T>>;
pub type Rx<T> = {rx_ty}<Vec<T>>;

pub struct InPort<T> {{
    buf: VecDeque<T>,
}}

impl<T: Clone> InPort<T> {{
    pub fn new() -> Self {{
        Self {{ buf: VecDeque::new() }}
    }}
    pub fn avail(&mut self, n: usize) -> bool {{
        self.buf.len() >= n
    }}
    pub fn peek(&self, i: usize) -> T {{
        self.buf[i].clone()
    }}
    pub fn recv(&mut self) -> T {{
        self.buf.pop_front().unwrap()
    }}
    pub fn pop_front(&mut self) -> Option<T> {{
        self.buf.pop_front()
    }}
    pub fn extend(&mut self, chunk: Vec<T>) {{
        self.buf.extend(chunk);
    }}
}}

pub enum Txs<T> {{
    None,
    One(Tx<T>),
    Many(Vec<Tx<T>>),
}}

pub struct OutPort<T> {{
    txs: Txs<T>,
    buf: VecDeque<T>,
}}

impl<T: Clone> OutPort<T> {{
    pub fn none() -> Self {{
        Self {{ txs: Txs::None, buf: VecDeque::new() }}
    }}
    pub fn one(tx: Tx<T>) -> Self {{
        Self {{ txs: Txs::One(tx), buf: VecDeque::new() }}
    }}
    pub fn many(txs: Vec<Tx<T>>) -> Self {{
        Self {{ txs: Txs::Many(txs), buf: VecDeque::new() }}
    }}
    pub fn has_room(&mut self) -> bool {{
        CAP == 0 || self.buf.len() < CAP
    }}
    pub fn push_back(&mut self, value: T) {{
        self.buf.push_back(value);
    }}
    pub async fn flush(&mut self) {{
        if self.buf.is_empty() {{
            return;
        }}
        let mut chunk: Vec<T> = self.buf.drain(..).collect();
        let targets: &[Tx<T>] = match &self.txs {{
            Txs::None => &[],
            Txs::One(tx) => core::slice::from_ref(tx),
            Txs::Many(txs) => txs,
        }};
        for (i, tx) in targets.iter().enumerate() {{
            let payload = if i + 1 == targets.len() {{
                core::mem::take(&mut chunk)
            }} else {{
                chunk.clone()
            }};
            let _ = tx.send(payload){send_await};
        }}
    }}
}}
"
    )
}

fn emit_flush(actor: &Actor) -> String {
    let mut out = String::new();
    for p in &actor.outports {
        let _ = writeln!(out, "__actor.{}.flush().await;", port_field(&p.name));
    }
    out
}

fn emit_task_run(actor: &Actor) -> String {
    let ty = type_ident(&actor.name);
    let run = format!("run_{}", ident(&actor.name));

    let mut params = vec![format!("mut __actor: {ty}")];
    for p in &actor.inports {
        params.push(format!(
            "mut rx_{}: Rx<{}>",
            ident(&p.name),
            rust_type(&p.typ)
        ));
    }
    let sig = params.join(", ");
    let flush = emit_flush(actor);

    let mut body = String::new();
    for p in &actor.inports {
        let _ = writeln!(body, "let mut open_{} = true;", ident(&p.name));
    }

    if actor.init.is_some() {
        body.push_str("__actor.init();\n");
        body.push_str(&flush);
    }

    if actor.inports.is_empty() {
        let _ = writeln!(body, "while __actor.fire() {{\n{flush}}}");
    } else {
        body.push_str("loop {\n");
        let _ = writeln!(body, "while __actor.fire() {{\n{flush}}}");
        let all_closed = actor
            .inports
            .iter()
            .map(|p| format!("!open_{}", ident(&p.name)))
            .collect::<Vec<_>>()
            .join(" && ");
        let _ = writeln!(body, "if {all_closed} {{ break; }}");
        body.push_str("tokio::select! {\n");
        body.push_str("biased;\n");
        for p in &actor.inports {
            let id = ident(&p.name);
            let _ = writeln!(
                body,
                "__m = rx_{id}.recv(), if open_{id} => {{ match __m {{ Some(__c) => {{ __actor.{}.extend(__c); }} None => {{ open_{id} = false; }} }} }}",
                port_field(&p.name)
            );
        }
        body.push_str("else => { break; }\n");
        body.push_str("}\n");
        body.push_str("}\n");
    }

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
        let mut ctor_args = vec![instance_args(inst, actor)];
        ctor_args.retain(|a| !a.is_empty());
        for _ in &actor.inports {
            ctor_args.push("InPort::new()".to_string());
        }
        for p in &actor.outports {
            let clones: Vec<String> = network
                .edges
                .iter()
                .filter(|e| e.src_id == inst.id && e.src_port == p.name)
                .map(|e| format!("tx_{}_{}.clone()", ident(&e.dst_id), ident(&e.dst_port)))
                .collect();
            ctor_args.push(out_port_ctor(&clones));
        }
        let _ = writeln!(
            out,
            "    let {} = {}::{}::new({});",
            inst_var(&inst.id),
            actor_mod(&actor.name),
            type_ident(&actor.name),
            ctor_args.join(", ")
        );
    }

    out.push_str("    let mut __set = tokio::task::JoinSet::new();\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut args = vec![inst_var(&inst.id)];
        for p in &actor.inports {
            args.push(format!("rx_{}_{}", ident(&inst.id), ident(&p.name)));
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
