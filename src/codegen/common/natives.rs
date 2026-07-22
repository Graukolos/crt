use std::collections::HashSet;
use std::fmt::Write as _;

use crate::ast::{Expr, Type};
use crate::codegen::Program;

use super::{ident, rust_type};

pub fn emit_natives(program: &Program<'_>) -> String {
    use crate::ast::{NativeFunction, NativeProcedure};

    let mut funcs: Vec<&NativeFunction> = Vec::new();
    let mut procs: Vec<&NativeProcedure> = Vec::new();
    let mut seen = HashSet::new();
    for unit in program.units {
        for f in &unit.native_functions {
            if seen.insert(f.name.clone()) {
                funcs.push(f);
            }
        }
        for p in &unit.native_procedures {
            if seen.insert(p.name.clone()) {
                procs.push(p);
            }
        }
    }
    for actor in program.actors.values() {
        for f in &actor.native_functions {
            if seen.insert(f.name.clone()) {
                funcs.push(f);
            }
        }
        for p in &actor.native_procedures {
            if seen.insert(p.name.clone()) {
                procs.push(p);
            }
        }
    }

    let mut out = String::new();

    out.push_str("unsafe extern \"C\" {\n");
    for f in &funcs {
        let params = c_param_list(&f.parameters);
        let _ = writeln!(
            out,
            "    #[link_name = \"{0}\"]\n    fn __crt_ffi_{1}({params}) -> {2};",
            f.name,
            ident(&f.name),
            c_type(&f.ret_type)
        );
    }
    for p in &procs {
        let params = c_param_list(&p.parameters);
        let _ = writeln!(
            out,
            "    #[link_name = \"{0}\"]\n    fn __crt_ffi_{1}({params});",
            p.name,
            ident(&p.name)
        );
    }
    out.push_str("}\n\n");

    for f in &funcs {
        let params = wrapper_param_list(&f.parameters);
        let args = wrapper_call_args(&f.parameters);
        let ret = rust_type(&f.ret_type);
        let body = if ret == "bool" {
            format!("unsafe {{ __crt_ffi_{}({args}) != 0 }}", ident(&f.name))
        } else {
            format!("unsafe {{ __crt_ffi_{}({args}) as {ret} }}", ident(&f.name))
        };
        let _ = writeln!(
            out,
            "fn {0}({params}) -> {ret} {{ {body} }}",
            ident(&f.name)
        );
    }
    for p in &procs {
        let params = wrapper_param_list(&p.parameters);
        let args = wrapper_call_args(&p.parameters);
        let _ = writeln!(
            out,
            "fn {0}({params}) {{ unsafe {{ __crt_ffi_{0}({args}); }} }}",
            ident(&p.name)
        );
    }

    out.push('\n');
    out.push_str(crate::codegen::orcc::OPTIONS_RS);
    out
}

fn c_param_list(params: &[crate::ast::Parameter]) -> String {
    params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("a{i}: {}", c_type(&p.typ)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn wrapper_param_list(params: &[crate::ast::Parameter]) -> String {
    params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("a{i}: {}", rust_type(&p.typ)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn wrapper_call_args(params: &[crate::ast::Parameter]) -> String {
    params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("a{i} as {}", c_type(&p.typ)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn c_type(t: &Type) -> String {
    if t.name == "bool" {
        return "core::ffi::c_int".to_string();
    }
    if t.list.is_some() {
        return "core::ffi::c_int".to_string();
    }
    let unsigned = t.name.starts_with("uint");
    let bits = t.size.as_ref().and_then(|e| eval_lit(e)).unwrap_or(32);
    let base = match bits {
        0..=8 => {
            if unsigned {
                "c_uchar"
            } else {
                "c_char"
            }
        }
        9..=16 => {
            if unsigned {
                "c_ushort"
            } else {
                "c_short"
            }
        }
        17..=32 => {
            if unsigned {
                "c_uint"
            } else {
                "c_int"
            }
        }
        _ => {
            if unsigned {
                "c_ulonglong"
            } else {
                "c_longlong"
            }
        }
    };
    format!("core::ffi::{base}")
}

fn eval_lit(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::Paren(inner) => eval_lit(inner),
        Expr::Literal { value, .. } => {
            let value = value.trim();
            if let Some(hex) = value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"))
            {
                u32::from_str_radix(hex, 16).ok()
            } else {
                value.parse::<u32>().ok()
            }
        }
        _ => None,
    }
}
