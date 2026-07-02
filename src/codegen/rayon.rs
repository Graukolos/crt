use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::codegen::common::*;
use crate::codegen::{CodeGenerator, Program};

pub struct Rayon;

impl CodeGenerator for Rayon {
    fn name(&self) -> &'static str {
        "rayon"
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
        super::write_cargo_toml(out_dir, &program.network.name, program.has_natives(), "rayon = \"1\"\n")?;
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
    main.push_str("use std::collections::VecDeque;\n");
    main.push_str("use std::sync::atomic::{AtomicBool, Ordering};\n\n");
    for class in &classes {
        let actor = &program.actors[*class];
        let _ = writeln!(main, "mod {};", actor_mod(&actor.name));
    }
    main.push('\n');
    main.push_str("const ROUND_BUDGET: usize = 1024;\n\n");
    main.push_str(&emit_shared_decls(program));
    main.push_str(&emit_main(program));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>) -> String {
    let (instances, mut out) = emit_main_prelude(program);

    out.push_str("    loop {\n");
    out.push_str("        let progress = AtomicBool::new(false);\n");
    out.push_str("        rayon::scope(|s| {\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let _ = writeln!(
            out,
            "            s.spawn(|_| {{ let mut __n = 0usize; while __n < ROUND_BUDGET && {}.fire({}) {{ progress.store(true, Ordering::Relaxed); __n += 1; }} }});",
            inst_var(&inst.id),
            fire_args(inst, actor)
        );
    }
    out.push_str("        });\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        out.push_str(&distribute(program, inst, actor));
    }
    out.push_str("        if !progress.load(Ordering::Relaxed) {\n            break;\n        }\n    }\n}\n");
    out
}
