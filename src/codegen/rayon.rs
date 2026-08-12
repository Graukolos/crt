use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::codegen::common::{
    CHAN_MOD, actor_mod, actor_port, chan_use, channel_types, check_no_fanout,
    check_single_producer, emit_actor, emit_chan_file, emit_main_prelude, emit_ring_ports,
    emit_shared_decls, inst_var, ring_channels, ring_deps, ring_imports, ring_main_imports,
};
use crate::codegen::{CodeGenerator, Options, Program};

pub struct Rayon {
    pub options: Options,
}

impl CodeGenerator for Rayon {
    fn name(&self) -> &'static str {
        "rayon"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path, orcc: bool) -> io::Result<()> {
        check_no_fanout(program)?;
        if self.options.cap > 0 {
            check_single_producer(program)?;
        }
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
        super::write_cargo_toml(
            out_dir,
            &program.network.name,
            program.has_natives(),
            &format!("rayon = \"1\"\n{}", ring_deps(self.options.cap)),
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
    let mut files = Vec::new();

    let classes: Vec<&String> = program.actors.keys().collect();

    for class in &classes {
        let actor = &program.actors[*class];
        let mut src = String::new();
        src.push_str("#![allow(warnings)]\n");
        src.push_str("use std::collections::VecDeque;\n");
        src.push_str("use super::*;\n\n");
        src.push_str(&emit_actor(actor, typestate));
        files.push((format!("{}.rs", actor_mod(&actor.name)), src));
    }

    files.push((
        format!("{CHAN_MOD}.rs"),
        emit_chan_file(
            ring_imports(options.cap),
            options.cap,
            &emit_ring_ports(options.cap, &channel_types(program)),
        ),
    ));

    let mut main = String::new();
    main.push_str("#![allow(warnings)]\n");
    main.push_str(ring_main_imports(options.cap));
    main.push('\n');
    main.push_str(&chan_use());
    for class in &classes {
        let actor = &program.actors[*class];
        let _ = writeln!(main, "mod {};", actor_mod(&actor.name));
    }
    main.push('\n');
    let _ = writeln!(
        main,
        "const FIRE_BUDGET: usize = {};\n",
        options.fire_budget_literal()
    );
    main.push_str(&emit_shared_decls(program, orcc));
    main.push_str(&emit_main(program, options, orcc, typestate));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>, options: Options, orcc: bool, typestate: bool) -> String {
    let (instances, mut out) =
        emit_main_prelude(program, orcc, ring_channels(options.cap), typestate);

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
            "            s.spawn(|_| {{ {}.schedule();{pumps} }});",
            inst_var(&inst.id)
        );
    }
    out.push_str("        });\n");
    out.push_str("    }\n}\n");
    out
}
