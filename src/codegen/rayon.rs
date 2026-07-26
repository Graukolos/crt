use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::codegen::common::{
    Channels, actor_mod, emit_actor, emit_main_prelude, emit_shared_decls, inst_var, port_field,
};
use crate::codegen::{CodeGenerator, Program};

pub struct Rayon {
    pub cap: usize,
}

const PORTS_RS: &str = r"pub struct InPort<T> {
    rx: crossbeam_channel::Receiver<T>,
    buf: VecDeque<T>,
}

impl<T: Clone> InPort<T> {
    pub fn new(rx: crossbeam_channel::Receiver<T>) -> Self {
        Self { rx, buf: VecDeque::new() }
    }
    pub fn avail(&mut self, n: usize) -> bool {
        while self.buf.len() < n {
            match self.rx.try_recv() {
                Ok(value) => self.buf.push_back(value),
                Err(_) => break,
            }
        }
        self.buf.len() >= n
    }
    pub fn peek(&self, i: usize) -> T {
        self.buf[i].clone()
    }
    pub fn recv(&mut self) -> T {
        self.buf.pop_front().unwrap()
    }
    pub fn pop_front(&mut self) -> Option<T> {
        self.buf.pop_front()
    }
}

pub enum OutPort<T> {
    None,
    One(crossbeam_channel::Sender<T>, VecDeque<T>),
    Many(Vec<crossbeam_channel::Sender<T>>, Vec<VecDeque<T>>),
}

impl<T: Clone> OutPort<T> {
    pub fn none() -> Self {
        Self::None
    }
    pub fn one(tx: crossbeam_channel::Sender<T>) -> Self {
        Self::One(tx, VecDeque::new())
    }
    pub fn many(txs: Vec<crossbeam_channel::Sender<T>>) -> Self {
        let pending = txs.iter().map(|_| VecDeque::new()).collect();
        Self::Many(txs, pending)
    }
    fn drain(tx: &crossbeam_channel::Sender<T>, queue: &mut VecDeque<T>) {
        while let Some(value) = queue.front() {
            match tx.try_send(value.clone()) {
                Ok(()) => {
                    queue.pop_front();
                }
                Err(crossbeam_channel::TrySendError::Full(_)) => break,
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    queue.clear();
                    break;
                }
            }
        }
    }
    pub fn pump(&mut self) {
        match self {
            Self::None => {}
            Self::One(tx, pending) => Self::drain(tx, pending),
            Self::Many(txs, pending) => {
                for (tx, queue) in txs.iter().zip(pending.iter_mut()) {
                    Self::drain(tx, queue);
                }
            }
        }
    }
    pub fn has_room(&mut self) -> bool {
        self.pump();
        match self {
            Self::None => true,
            Self::One(tx, pending) => pending.is_empty() && !tx.is_full(),
            Self::Many(txs, pending) => {
                pending.iter().all(|q| q.is_empty()) && txs.iter().all(|tx| !tx.is_full())
            }
        }
    }
    pub fn push_back(&mut self, value: T) {
        match self {
            Self::None => {}
            Self::One(tx, pending) => {
                if pending.is_empty() {
                    match tx.try_send(value) {
                        Ok(()) => {}
                        Err(crossbeam_channel::TrySendError::Full(value)) => {
                            pending.push_back(value)
                        }
                        Err(crossbeam_channel::TrySendError::Disconnected(_)) => {}
                    }
                } else {
                    pending.push_back(value);
                    Self::drain(tx, pending);
                }
            }
            Self::Many(txs, pending) => {
                for queue in pending.iter_mut() {
                    queue.push_back(value.clone());
                }
                for (tx, queue) in txs.iter().zip(pending.iter_mut()) {
                    Self::drain(tx, queue);
                }
            }
        }
    }
}
";

impl CodeGenerator for Rayon {
    fn name(&self) -> &'static str {
        "rayon"
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
        super::write_cargo_toml(
            out_dir,
            &program.network.name,
            program.has_natives(),
            "rayon = \"1\"\ncrossbeam-channel = \"0.5\"\n",
            orcc,
        )?;
        if program.has_natives() {
            super::write_native_support(out_dir, program.native_sources, orcc)?;
        }
        Ok(())
    }
}

fn emit_files(program: &Program<'_>, cap: usize, orcc: bool) -> Vec<(String, String)> {
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
    let _ = writeln!(main, "const CAP: usize = {cap};");
    main.push_str("const ROUND_BUDGET: usize = 1024;\n\n");
    main.push_str(PORTS_RS);
    main.push('\n');
    main.push_str(&emit_shared_decls(program, orcc));
    main.push_str(&emit_main(program, orcc));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>, orcc: bool) -> String {
    let (instances, mut out) = emit_main_prelude(program, orcc, Channels::Crossbeam);

    out.push_str("    loop {\n");
    out.push_str("        rayon::scope(|s| {\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut pumps = String::new();
        for port in &actor.outports {
            let _ = write!(
                pumps,
                " {}.{}.pump();",
                inst_var(&inst.id),
                port_field(&port.name)
            );
        }
        let _ = writeln!(
            out,
            "            s.spawn(|_| {{ let mut __n = 0usize; while __n < ROUND_BUDGET && {}.fire() {{ __n += 1; }}{pumps} }});",
            inst_var(&inst.id)
        );
    }
    out.push_str("        });\n");
    out.push_str("    }\n}\n");
    out
}
