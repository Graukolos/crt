use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::codegen::common::{
    CROSSBEAM_PORTS, Channels, actor_mod, emit_actor, emit_main_prelude, emit_shared_decls,
    inst_var, port_field,
};
use crate::codegen::{CodeGenerator, Program};

pub struct Threads {
    pub cap: usize,
}

impl CodeGenerator for Threads {
    fn name(&self) -> &'static str {
        "threads"
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
            "crossbeam-channel = \"0.5\"\n",
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
    main.push_str("const FIRE_BUDGET: usize = 1024;\n\n");
    main.push_str(CROSSBEAM_PORTS);
    main.push('\n');
    main.push_str(&emit_shared_decls(program, orcc));
    main.push_str(&emit_main(program, orcc));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>, orcc: bool) -> String {
    let (instances, mut out) = emit_main_prelude(program, orcc, Channels::Crossbeam);

    for inst in &instances {
        let var = inst_var(&inst.id);
        let _ = writeln!(out, "    let {var} = std::sync::Mutex::new({var});");
    }

    out.push_str(
        "    let __threads = std::env::var(\"CRT_THREADS\")\n        \
         .ok()\n        \
         .and_then(|value| value.parse::<usize>().ok())\n        \
         .filter(|count| *count > 0)\n        \
         .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |count| count.get()));\n",
    );

    out.push_str("    std::thread::scope(|s| {\n");
    out.push_str("        for _ in 0..__threads {\n");
    out.push_str("            s.spawn(|| {\n");
    out.push_str("                loop {\n");
    out.push_str("                    let mut __idle = true;\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut pumps = String::new();
        for port in &actor.outports {
            let _ = write!(pumps, " __actor.{}.pump();", port_field(&port.name));
        }
        let _ = writeln!(
            out,
            "                    if let Ok(mut __actor) = {}.try_lock() {{ let mut __n = 0usize; while __n < FIRE_BUDGET && __actor.fire() {{ __n += 1; }}{pumps} __idle &= __n == 0; }}",
            inst_var(&inst.id)
        );
    }
    out.push_str("                    if __idle { std::thread::yield_now(); }\n");
    out.push_str("                }\n");
    out.push_str("            });\n");
    out.push_str("        }\n");
    out.push_str("    });\n}\n");
    out
}
