use std::collections::HashSet;
use std::fmt::Write as _;

use crate::ast::{Expr, Type};
use crate::codegen::Program;

use super::{ident, rust_type};

pub fn emit_natives(program: &Program<'_>, orcc: bool) -> String {
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
    if orcc {
        out.push_str(crate::codegen::orcc::OPTIONS_RS);
    }
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

pub fn check_natives(program: &Program<'_>) -> std::io::Result<()> {
    let signatures = program
        .units
        .iter()
        .flat_map(|u| &u.native_functions)
        .map(|f| (&f.name, f.parameters.as_slice(), Some(&f.ret_type)))
        .chain(program.actors.values().flat_map(|a| {
            a.native_functions
                .iter()
                .map(|f| (&f.name, f.parameters.as_slice(), Some(&f.ret_type)))
        }))
        .chain(
            program
                .units
                .iter()
                .flat_map(|u| &u.native_procedures)
                .chain(program.actors.values().flat_map(|a| &a.native_procedures))
                .map(|p| (&p.name, p.parameters.as_slice(), None)),
        );

    let mut offenders = Vec::new();
    for (name, params, ret) in signatures {
        let unsupported: Vec<String> = params
            .iter()
            .map(|p| (p.name.as_str(), &p.typ))
            .chain(ret.map(|t| ("return value", t)))
            .filter(|(_, t)| !c_compatible(t))
            .map(|(what, t)| format!("{what}: {}", t.name))
            .collect();
        if !unsupported.is_empty() {
            offenders.push(format!("{name} ({})", unsupported.join(", ")));
        }
    }
    if offenders.is_empty() {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "native functions with types that cannot cross the C boundary: {}",
            offenders.join("; ")
        ),
    ))
}

fn c_compatible(t: &Type) -> bool {
    t.list.is_none()
        && matches!(
            t.name.as_str(),
            "int" | "uint" | "bool" | "float" | "double" | "half"
        )
}

fn c_type(t: &Type) -> String {
    let bits = t.size.as_ref().and_then(|e| eval_lit(e));
    match t.name.as_str() {
        "bool" => return "core::ffi::c_int".to_string(),
        "double" => return "core::ffi::c_double".to_string(),
        "float" | "half" => {
            return if bits.is_some_and(|b| b > 32) {
                "core::ffi::c_double".to_string()
            } else {
                "core::ffi::c_float".to_string()
            };
        }
        _ => {}
    }
    let unsigned = t.name.starts_with("uint");
    let base = match bits.unwrap_or(32) {
        0..=8 => {
            if unsigned {
                "c_uchar"
            } else {
                "c_schar"
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
