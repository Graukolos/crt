use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::codegen::common::{
    Channels, actor_mod, emit_actor, emit_main_prelude, emit_shared_decls, inst_var,
};
use crate::codegen::{CodeGenerator, Program};

pub struct Naive {
    pub cap: usize,
    pub typestate: bool,
}

const PORTS_RS: &str = r"pub struct InPort<T> {
    chan: Rc<RefCell<VecDeque<T>>>,
}

impl<T: Clone> InPort<T> {
    pub fn new(chan: Rc<RefCell<VecDeque<T>>>) -> Self {
        Self { chan }
    }
    pub fn avail(&mut self, n: usize) -> bool {
        self.chan.borrow().len() >= n
    }
    pub fn peek(&self, i: usize) -> T {
        self.chan.borrow()[i].clone()
    }
    pub fn recv(&mut self) -> T {
        self.chan.borrow_mut().pop_front().unwrap()
    }
    pub fn pop_front(&mut self) -> Option<T> {
        self.chan.borrow_mut().pop_front()
    }
}

pub enum OutPort<T> {
    None,
    One(Rc<RefCell<VecDeque<T>>>),
    Many(Vec<Rc<RefCell<VecDeque<T>>>>),
}

impl<T: Clone> OutPort<T> {
    pub fn none() -> Self {
        Self::None
    }
    pub fn one(target: Rc<RefCell<VecDeque<T>>>) -> Self {
        Self::One(target)
    }
    pub fn many(targets: Vec<Rc<RefCell<VecDeque<T>>>>) -> Self {
        Self::Many(targets)
    }
    pub fn has_room(&mut self) -> bool {
        if CAP == 0 {
            return true;
        }
        match self {
            Self::None => true,
            Self::One(target) => target.borrow().len() < CAP,
            Self::Many(targets) => targets.iter().all(|t| t.borrow().len() < CAP),
        }
    }
    pub fn push_back(&mut self, value: T) {
        match self {
            Self::None => {}
            Self::One(target) => target.borrow_mut().push_back(value),
            Self::Many(targets) => {
                for target in targets {
                    target.borrow_mut().push_back(value.clone());
                }
            }
        }
    }
}
";

impl CodeGenerator for Naive {
    fn name(&self) -> &'static str {
        "naive"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path, orcc: bool) -> io::Result<()> {
        let src_dir = out_dir.join("src");
        for (name, source) in emit_files(program, self.cap, orcc, self.typestate) {
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
            "",
            orcc,
        )?;
        if program.has_natives() {
            super::write_native_support(out_dir, program.native_sources, orcc)?;
        }
        Ok(())
    }
}

fn emit_files(
    program: &Program<'_>,
    cap: usize,
    orcc: bool,
    typestate: bool,
) -> Vec<(String, String)> {
    let mut files = Vec::new();

    let mut classes: Vec<&String> = program.actors.keys().collect();
    classes.sort();

    for class in &classes {
        let actor = &program.actors[*class];
        let mut src = String::new();
        src.push_str("#![allow(warnings)]\n");
        src.push_str("use std::collections::VecDeque;\n");
        src.push_str("use super::*;\n\n");
        src.push_str(&emit_actor(actor, typestate));
        files.push((format!("{}.rs", actor_mod(&actor.name)), src));
    }

    let mut main = String::new();
    main.push_str("#![allow(warnings)]\n");
    main.push_str("use std::cell::RefCell;\n");
    main.push_str("use std::collections::VecDeque;\n");
    main.push_str("use std::rc::Rc;\n\n");
    for class in &classes {
        let actor = &program.actors[*class];
        let _ = writeln!(main, "mod {};", actor_mod(&actor.name));
    }
    main.push('\n');
    let _ = writeln!(main, "const CAP: usize = {cap};\n");
    main.push_str(PORTS_RS);
    main.push('\n');
    main.push_str(&emit_shared_decls(program, orcc));
    main.push_str(&emit_main(program, orcc, typestate));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>, orcc: bool, typestate: bool) -> String {
    let (instances, mut out) = emit_main_prelude(program, orcc, Channels::Local, typestate);

    out.push_str("    loop {\n");
    for inst in &instances {
        let _ = writeln!(out, "        {}.fire();", inst_var(&inst.id));
    }
    out.push_str("    }\n}\n");
    out
}
