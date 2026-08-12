use std::fmt::Write as _;

use crate::ast::Actor;
use crate::codegen::common::{
    actor_mod, actor_port, actor_type, chan_credit, chan_rx, chan_tx, ident, inst_var,
    instance_args, out_port_ctor, output_burst, rust_type,
};
use crate::codegen::{Options, Program};
use crate::network_ffi::ffi::Instance;

pub fn ports(_program: &Program<'_>, _options: Options) -> String {
    format!(
        r"pub type Tx<T> = tokio::sync::mpsc::Sender<Vec<T>>;
pub type Rx<T> = tokio::sync::mpsc::Receiver<Vec<T>>;
pub type Credit = std::sync::Arc<std::sync::atomic::AtomicUsize>;

const CREDIT_ORDER: std::sync::atomic::Ordering = std::sync::atomic::Ordering::Relaxed;

pub fn credit() -> Credit {{
    std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))
}}

{IN_PORT}{OUT_PORT}"
    )
}

pub fn actor_extra(actor: &Actor, options: Options) -> String {
    format!("\n{}", emit_task_run(actor, options.typestate))
}

const IN_PORT: &str = r"pub struct InPort<T> {
    buf: VecDeque<T>,
    credit: Credit,
}

impl<T: Clone> InPort<T> {
    pub fn new(credit: Credit) -> Self {
        Self { buf: VecDeque::new(), credit }
    }
    pub fn len(&mut self) -> usize {
        self.buf.len()
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
}

pub struct OutPort<T> {
    txs: Txs<T>,
    buf: VecDeque<T>,
}
";

const OUT_PORT: &str = r"
impl<T: Clone> OutPort<T> {
    pub fn none() -> Self {
        Self { txs: Txs::None, buf: VecDeque::new() }
    }
    pub fn one(target: (Tx<T>, Credit)) -> Self {
        Self { txs: Txs::One(target.0, target.1), buf: VecDeque::new() }
    }
    pub fn room(&mut self) -> usize {
        let pending = self.buf.len();
        match &self.txs {
            Txs::None => usize::MAX,
            Txs::One(_, credit) => CAP.saturating_sub(credit.load(CREDIT_ORDER) + pending),
        }
    }
    pub fn has_room(&mut self) -> bool {
        self.room() > 0
    }
    pub fn push_back(&mut self, value: T) {
        self.buf.push_back(value);
    }
    pub async fn flush(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let chunk: Vec<T> = self.buf.drain(..).collect();
        let tokens = chunk.len();
        match &self.txs {
            Txs::None => {}
            Txs::One(tx, credit) => {
                credit.fetch_add(tokens, CREDIT_ORDER);
                let _ = tx.send(chunk).await;
            }
        }
    }
}
";

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
                let port = actor_port(actor, typestate, "__actor", &p.name);
                match output_burst(actor, &p.name) {
                    Some(burst) => format!("{port}.room() >= {burst}"),
                    None => format!("{port}.has_room()"),
                }
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

    let drain = format!(
        "loop {{\n    let __n = __actor.schedule();\n{flush}    if __n == 0 {{ break; }}\n}}\n"
    );

    let room = emit_room_probe(actor, typestate);
    let await_room = room.as_ref().map_or_else(String::new, |expr| {
        format!(
            "if !({expr}) {{ tokio::task::yield_now().await; continue; }}\nlet __n = __actor.schedule();\n{flush}if __n > 0 {{ continue; }}\n"
        )
    });

    if actor.inports.is_empty() {
        if room.is_none() {
            body.push_str(&drain);
        } else {
            body.push_str("loop {\n");
            body.push_str(&drain);
            body.push_str(&await_room);
            body.push_str("break;\n}\n");
        }
    } else {
        body.push_str("loop {\n");
        body.push_str(&drain);
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

pub fn emit_main(program: &Program<'_>, options: Options) -> String {
    let typestate = options.typestate;
    let network = program.network;
    let instances: Vec<&Instance> = network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect();

    let mut out = String::new();
    out.push_str("#[tokio::main]\nasync fn main() {\n");

    if options.orcc {
        out.push_str(super::orcc::MAIN_SETUP);
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        for p in &actor.inports {
            let ctor = format!(
                "tokio::sync::mpsc::channel::<Vec<{}>>(CAP)",
                rust_type(&p.typ)
            );
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
