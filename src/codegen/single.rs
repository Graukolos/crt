use std::fmt::Write as _;

use crate::codegen::common::{LOCAL_HANDLE, emit_main_prelude, inst_var};
use crate::codegen::{Options, Program};

pub fn emit_main(program: &Program<'_>, options: Options) -> String {
    let (instances, mut out) = emit_main_prelude(program, options, LOCAL_HANDLE);

    out.push_str("    loop {\n");
    for inst in &instances {
        let _ = writeln!(out, "        {}.schedule();", inst_var(&inst.id));
    }
    out.push_str("    }\n}\n");
    out
}
