use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::codegen::common::{
    CROSSBEAM_PORTS, Channels, actor_mod, actor_port, emit_actor, emit_main_prelude,
    emit_shared_decls, inst_var,
};
use crate::codegen::{CodeGenerator, Program};

pub struct Rayon {
    pub cap: usize,
    pub typestate: bool,
}

impl CodeGenerator for Rayon {
    fn name(&self) -> &'static str {
        "rayon"
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
            "rayon = \"1\"\ncrossbeam-channel = \"0.5\"\n",
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
    main.push_str("use std::collections::VecDeque;\n\n");
    for class in &classes {
        let actor = &program.actors[*class];
        let _ = writeln!(main, "mod {};", actor_mod(&actor.name));
    }
    main.push('\n');
    let _ = writeln!(main, "const CAP: usize = {cap};");
    main.push_str("const ROUND_BUDGET: usize = 1024;\n\n");
    main.push_str(CROSSBEAM_PORTS);
    main.push('\n');
    main.push_str(&emit_shared_decls(program, orcc));
    main.push_str(&emit_main(program, orcc, typestate));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>, orcc: bool, typestate: bool) -> String {
    let (instances, mut out) = emit_main_prelude(program, orcc, Channels::Crossbeam, typestate);

    out.push_str("    loop {\n");
    out.push_str("        rayon::scope(|s| {\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut pumps = String::new();
        for port in &actor.outports {
            let _ = write!(
                pumps,
                " {}.pump();",
                actor_port(actor, typestate, &inst_var(&inst.id), &port.name)
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
