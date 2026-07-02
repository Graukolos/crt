use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::ast::Actor;
use crate::codegen::common::*;
use crate::codegen::{CodeGenerator, Program};
use crate::network_ffi::ffi::Instance;

pub struct Naive;

impl CodeGenerator for Naive {
    fn name(&self) -> &'static str {
        "naive"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path) -> io::Result<()> {
        let src_dir = out_dir.join("src");
        for (name, source) in emit_files(program) {
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
        super::write_cargo_toml(out_dir, &program.network.name, program.has_natives(), "")?;
        if program.has_natives() {
            super::write_native_support(out_dir, program.native_sources)?;
        }
        Ok(())
    }
}

fn emit_files(program: &Program<'_>) -> Vec<(String, String)> {
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
    main.push_str(&emit_shared_decls(program));
    main.push_str(&emit_main(program));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>) -> String {
    let network = program.network;
    let instances: Vec<&Instance> = network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect();

    let mut out = String::from("fn main() {\n");

    if program.has_natives() {
        out.push_str(super::orcc::MAIN_SETUP);
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        for port in &actor.inports {
            let _ = writeln!(
                out,
                "    let mut {}: VecDeque<{}> = VecDeque::new();",
                fifo_in(&inst.id, &port.name),
                rust_type(&port.typ)
            );
        }
        for port in &actor.outports {
            let _ = writeln!(
                out,
                "    let mut {}: VecDeque<{}> = VecDeque::new();",
                fifo_out(&inst.id, &port.name),
                rust_type(&port.typ)
            );
        }
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let args = instance_args(inst, actor);
        let _ = writeln!(
            out,
            "    let mut {} = {}::{}::new({args});",
            inst_var(&inst.id),
            actor_mod(&actor.name),
            type_ident(&actor.name)
        );
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        if actor.init.is_some() {
            let _ = writeln!(
                out,
                "    {}.init({});",
                inst_var(&inst.id),
                fire_args(inst, actor)
            );
        }
    }

    out.push_str("    loop {\n        let mut progress = false;\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let _ = write!(
            out,
            "        if {}.fire({}) {{\n{}            progress = true;\n        }}\n",
            inst_var(&inst.id),
            fire_args(inst, actor),
            distribute(program, inst, actor)
        );
    }
    out.push_str("        if !progress {\n            break;\n        }\n    }\n}\n");
    out
}

fn fire_args(inst: &Instance, actor: &Actor) -> String {
    let mut parts = Vec::new();
    for port in &actor.inports {
        parts.push(format!("&mut {}", fifo_in(&inst.id, &port.name)));
    }
    for port in &actor.outports {
        parts.push(format!("&mut {}", fifo_out(&inst.id, &port.name)));
    }
    parts.join(", ")
}

fn distribute(program: &Program<'_>, inst: &Instance, actor: &Actor) -> String {
    let mut out = String::new();
    for port in &actor.outports {
        let targets: Vec<String> = program
            .network
            .edges
            .iter()
            .filter(|e| e.src_id == inst.id && e.src_port == port.name)
            .map(|e| fifo_in(&e.dst_id, &e.dst_port))
            .collect();
        let staging = fifo_out(&inst.id, &port.name);
        if targets.is_empty() {
            let _ = writeln!(out, "            {staging}.clear();");
        } else {
            let _ = writeln!(
                out,
                "            while let Some(token) = {staging}.pop_front() {{"
            );
            for target in &targets {
                let _ = writeln!(out, "                {target}.push_back(token);");
            }
            out.push_str("            }\n");
        }
    }
    out
}
