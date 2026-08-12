use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::codegen::common::{
    CHAN_MOD, Channels, actor_mod, chan_use, channel_types, check_no_fanout, emit_actor,
    emit_chan_file, emit_main_prelude, emit_shared_decls, inst_var, local_chan_imports,
    local_ports,
};
use crate::codegen::{CodeGenerator, Options, Program};

pub struct Naive {
    pub options: Options,
}

impl CodeGenerator for Naive {
    fn name(&self) -> &'static str {
        "naive"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path, orcc: bool) -> io::Result<()> {
        check_no_fanout(program)?;
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
            "",
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
            local_chan_imports(options.cap),
            options.cap,
            &local_ports(options.cap, &channel_types(program)),
        ),
    ));

    let mut main = String::new();
    main.push_str("#![allow(warnings)]\n");
    main.push_str("use std::collections::VecDeque;\n");
    main.push_str("use std::rc::Rc;\n\n");
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
    main.push_str(&emit_main(program, orcc, typestate));
    files.push(("main.rs".to_string(), main));

    files
}

fn emit_main(program: &Program<'_>, orcc: bool, typestate: bool) -> String {
    let (instances, mut out) = emit_main_prelude(program, orcc, Channels::Local, typestate);

    out.push_str("    loop {\n");
    for inst in &instances {
        let _ = writeln!(out, "        {}.schedule();", inst_var(&inst.id));
    }
    out.push_str("    }\n}\n");
    out
}
