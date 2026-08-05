use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::ast::Actor;
use crate::codegen::common::{
    actor_mod, actor_port, actor_type, chan_credit, chan_rx, chan_tx, emit_actor,
    emit_shared_decls, ident, inst_var, instance_args, out_port_ctor, rust_type,
};
use crate::codegen::{CodeGenerator, Options, Program};
use crate::network_ffi::ffi::Instance;

pub struct Tokio {
    pub options: Options,
}

impl CodeGenerator for Tokio {
    fn name(&self) -> &'static str {
        "tokio"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path, orcc: bool) -> io::Result<()> {
        let src_dir = out_dir.join("src");
        for (name, source) in emit_files(program, self.options, orcc) {
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

fn emit_files(program: &Program<'_>, options: Options, orcc: bool) -> Vec<(String, String)> {
    let typestate = options.typestate;
    let unbounded = options.cap == 0;
    let mut files = Vec::new();

    let classes: Vec<&String> = program.actors.keys().collect();

    for class in &classes {
        let actor = &program.actors[*class];
        let mut src = String::new();
        src.push_str("#![allow(warnings)]\n");
        src.push_str("use std::collections::VecDeque;\n");
        src.push_str("use super::*;\n\n");
        src.push_str(&emit_actor(actor, typestate));
        src.push('\n');
        src.push_str(&emit_task_run(actor, typestate));
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
    let _ = writeln!(main, "const CAP: usize = {};\n", options.cap);
    main.push_str(&emit_ports(unbounded));
    main.push('\n');
    main.push_str(&emit_shared_decls(program, orcc));
    main.push_str(&emit_main(program, unbounded, orcc, typestate));
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
pub type Credit = std::sync::Arc<std::sync::atomic::AtomicUsize>;

const CREDIT_ORDER: std::sync::atomic::Ordering = std::sync::atomic::Ordering::Relaxed;

pub fn credit() -> Credit {{
    std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))
}}

{IN_PORT}{}",
        emit_out_port(send_await)
    )
}

const IN_PORT: &str = r"pub struct InPort<T> {
    buf: VecDeque<T>,
    credit: Credit,
}

impl<T: Clone> InPort<T> {
    pub fn new(credit: Credit) -> Self {
        Self { buf: VecDeque::new(), credit }
    }
    pub fn avail(&mut self, n: usize) -> bool {
        self.buf.len() >= n
    }
    pub fn peek(&self, i: usize) -> T {
        self.buf[i].clone()
    }
    pub fn recv(&mut self) -> T {
        let value = self.buf.pop_front().unwrap();
        self.credit.fetch_sub(1, CREDIT_ORDER);
        value
    }
    pub fn pop_front(&mut self) -> Option<T> {
        let value = self.buf.pop_front();
        if value.is_some() {
            self.credit.fetch_sub(1, CREDIT_ORDER);
        }
        value
    }
    pub fn extend(&mut self, chunk: Vec<T>) {
        self.buf.extend(chunk);
    }
}

pub enum Txs<T> {
    None,
    One(Tx<T>, Credit),
    Many(Vec<(Tx<T>, Credit)>),
}

pub struct OutPort<T> {
    txs: Txs<T>,
    buf: VecDeque<T>,
}
";

fn emit_out_port(send_await: &str) -> String {
    format!(
        r"
impl<T: Clone> OutPort<T> {{
    pub fn none() -> Self {{
        Self {{ txs: Txs::None, buf: VecDeque::new() }}
    }}
    pub fn one(target: (Tx<T>, Credit)) -> Self {{
        Self {{ txs: Txs::One(target.0, target.1), buf: VecDeque::new() }}
    }}
    pub fn many(targets: Vec<(Tx<T>, Credit)>) -> Self {{
        Self {{ txs: Txs::Many(targets), buf: VecDeque::new() }}
    }}
    pub fn has_room(&mut self) -> bool {{
        if CAP == 0 {{
            return true;
        }}
        let pending = self.buf.len();
        match &self.txs {{
            Txs::None => true,
            Txs::One(_, credit) => credit.load(CREDIT_ORDER) + pending < CAP,
            Txs::Many(targets) => targets
                .iter()
                .all(|(_, credit)| credit.load(CREDIT_ORDER) + pending < CAP),
        }}
    }}
    pub fn push_back(&mut self, value: T) {{
        self.buf.push_back(value);
    }}
    pub async fn flush(&mut self) {{
        if self.buf.is_empty() {{
            return;
        }}
        let mut chunk: Vec<T> = self.buf.drain(..).collect();
        let tokens = chunk.len();
        match &self.txs {{
            Txs::None => {{}}
            Txs::One(tx, credit) => {{
                credit.fetch_add(tokens, CREDIT_ORDER);
                let _ = tx.send(chunk){send_await};
            }}
            Txs::Many(targets) => {{
                for (i, (tx, credit)) in targets.iter().enumerate() {{
                    let payload = if i + 1 == targets.len() {{
                        core::mem::take(&mut chunk)
                    }} else {{
                        chunk.clone()
                    }};
                    credit.fetch_add(tokens, CREDIT_ORDER);
                    let _ = tx.send(payload){send_await};
                }}
            }}
        }}
    }}
}}
"
    )
}

fn emit_flush(actor: &Actor, typestate: bool) -> String {
    let mut out = String::new();
    for p in &actor.outports {
        let _ = writeln!(
            out,
            "{}.flush().await;",
            actor_port(actor, typestate, "__actor", &p.name)
        );
    }
    out
}

fn emit_room_probe(actor: &Actor, typestate: bool) -> Option<String> {
    if actor.outports.is_empty() {
        return None;
    }
    Some(
        actor
            .outports
            .iter()
            .map(|p| {
                format!(
                    "{}.has_room()",
                    actor_port(actor, typestate, "__actor", &p.name)
                )
            })
            .collect::<Vec<_>>()
            .join(" && "),
    )
}

fn emit_task_run(actor: &Actor, typestate: bool) -> String {
    let ty = actor_type(actor, typestate);
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
    let flush = emit_flush(actor, typestate);

    let mut body = String::new();
    for p in &actor.inports {
        let _ = writeln!(body, "let mut open_{} = true;", ident(&p.name));
    }

    if actor.init.is_some() {
        body.push_str("__actor.init();\n");
        body.push_str(&flush);
    }

    let room = emit_room_probe(actor, typestate);
    let await_room = room.as_ref().map_or_else(String::new, |expr| {
        format!(
            "if !({expr}) {{ tokio::task::yield_now().await; continue; }}\nif __actor.fire() {{\n{flush}continue;\n}}\n"
        )
    });

    if actor.inports.is_empty() {
        if room.is_none() {
            let _ = writeln!(body, "while __actor.fire() {{\n{flush}}}");
        } else {
            body.push_str("loop {\n");
            let _ = writeln!(body, "while __actor.fire() {{\n{flush}}}");
            body.push_str(&await_room);
            body.push_str("break;\n}\n");
        }
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
        body.push_str(&await_room);
        body.push_str("tokio::select! {\n");
        body.push_str("biased;\n");
        for p in &actor.inports {
            let id = ident(&p.name);
            let _ = writeln!(
                body,
                "__m = rx_{id}.recv(), if open_{id} => {{ match __m {{ Some(__c) => {{ {}.extend(__c); }} None => {{ open_{id} = false; }} }} }}",
                actor_port(actor, typestate, "__actor", &p.name)
            );
        }
        body.push_str("else => { break; }\n");
        body.push_str("}\n");
        body.push_str("}\n");
    }

    format!("pub async fn {run}({sig}) {{\n{body}}}\n")
}

fn emit_main(program: &Program<'_>, unbounded: bool, orcc: bool, typestate: bool) -> String {
    let network = program.network;
    let instances: Vec<&Instance> = network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect();

    let mut out = String::new();
    out.push_str("#[tokio::main]\nasync fn main() {\n");

    if orcc {
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
                "    let ({}, {}) = {ctor};",
                chan_tx(&inst.id, &p.name),
                chan_rx(&inst.id, &p.name),
            );
            let _ = writeln!(
                out,
                "    let {} = credit();",
                chan_credit(&inst.id, &p.name)
            );
        }
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut ctor_args = vec![instance_args(inst, actor)];
        ctor_args.retain(|a| !a.is_empty());
        for p in &actor.inports {
            ctor_args.push(format!(
                "InPort::new({}.clone())",
                chan_credit(&inst.id, &p.name)
            ));
        }
        for p in &actor.outports {
            let clones: Vec<String> = network
                .edges
                .iter()
                .filter(|e| e.src_id == inst.id && e.src_port == p.name)
                .map(|e| {
                    format!(
                        "({}.clone(), {}.clone())",
                        chan_tx(&e.dst_id, &e.dst_port),
                        chan_credit(&e.dst_id, &e.dst_port)
                    )
                })
                .collect();
            ctor_args.push(out_port_ctor(&clones));
        }
        let _ = writeln!(
            out,
            "    let {} = {}::{}::new({});",
            inst_var(&inst.id),
            actor_mod(&actor.name),
            actor_type(actor, typestate),
            ctor_args.join(", ")
        );
    }

    out.push_str("    let mut __set = tokio::task::JoinSet::new();\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut args = vec![inst_var(&inst.id)];
        for p in &actor.inports {
            args.push(chan_rx(&inst.id, &p.name));
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
            let _ = writeln!(out, "    drop({});", chan_tx(&inst.id, &p.name));
        }
    }

    out.push_str("    while __set.join_next().await.is_some() {}\n");
    out.push_str("}\n");
    out
}
