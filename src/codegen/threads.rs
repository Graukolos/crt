use std::fmt::Write as _;

use crate::codegen::common::{SHARED_HANDLE, actor_port, emit_main_prelude, inst_var};
use crate::codegen::{Options, Program};

pub fn emit_main(program: &Program<'_>, options: Options) -> String {
    let (instances, mut out) = emit_main_prelude(program, options, SHARED_HANDLE);

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
            let _ = write!(
                pumps,
                " {}.pump();",
                actor_port(actor, options.typestate, "__actor", &port.name)
            );
        }
        let _ = writeln!(
            out,
            "                    if let Ok(mut __actor) = {}.try_lock() {{ let __n = __actor.schedule();{pumps} __idle &= __n == 0; }}",
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
