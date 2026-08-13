use std::fmt::Write as _;

use crate::codegen::common::{SHARED_HANDLE, actor_port, emit_main_prelude, inst_var};
use crate::codegen::{Options, Program};

pub fn emit_main(program: &Program<'_>, options: Options) -> String {
    let (instances, mut out) = emit_main_prelude(program, options, SHARED_HANDLE);

    out.push_str("    loop {\n");
    out.push_str("        rayon::scope(|s| {\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut commits = String::new();
        for port in actor.outports.iter().chain(actor.inports.iter()) {
            let _ = write!(
                commits,
                " {}.commit();",
                actor_port(actor, options.typestate, &inst_var(&inst.id), &port.name)
            );
        }
        let _ = writeln!(
            out,
            "            s.spawn(|_| {{ {}.schedule();{commits} }});",
            inst_var(&inst.id)
        );
    }
    out.push_str("        });\n");
    out.push_str("    }\n}\n");
    out
}
