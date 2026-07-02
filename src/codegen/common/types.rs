use std::collections::HashSet;

use crate::ast::{Expr, Type, VarDef};

use super::{emit_expr, ident};

pub fn rust_type(t: &Type) -> String {
    if let Some(inner) = &t.list {
        return format!("Vec<{}>", rust_type(inner));
    }
    match t.name.as_str() {
        "bool" => "bool".to_string(),
        "String" | "string" => "String".to_string(),
        "float" | "double" | "half" => "f64".to_string(),
        _ => "i64".to_string(),
    }
}

pub fn default_value(t: &Type) -> String {
    if let Some(inner) = &t.list {
        return match &t.size {
            Some(size) => format!(
                "vec![{}; ({}) as usize]",
                default_value(inner),
                emit_expr(size, &HashSet::new(), &HashSet::new())
            ),
            None => "Vec::new()".to_string(),
        };
    }
    match t.name.as_str() {
        "bool" => "false".to_string(),
        "String" | "string" => "String::new()".to_string(),
        "float" | "double" | "half" => "0.0".to_string(),
        _ => "0".to_string(),
    }
}

pub fn emit_const(v: &VarDef) -> String {
    if !v.arrays.is_empty() {
        let mut ty = rust_type(&v.typ);
        for _ in &v.arrays {
            ty = format!("&[{ty}]");
        }
        let value = match &v.assign {
            Some(expr) => const_array_value(expr),
            None => "&[]".to_string(),
        };
        return format!("static {}: {ty} = {value};\n", ident(&v.name));
    }

    let value = match &v.assign {
        Some(expr) => emit_expr(expr, &HashSet::new(), &HashSet::new()),
        None => default_value(&v.typ),
    };
    format!(
        "const {}: {} = {value};\n",
        ident(&v.name),
        rust_type(&v.typ)
    )
}

fn const_array_value(expr: &Expr) -> String {
    if let Expr::ListComprehension {
        expressions,
        generators,
    } = expr
        && generators.is_empty()
    {
        let elems = expressions
            .iter()
            .map(const_array_value)
            .collect::<Vec<_>>()
            .join(", ");
        return format!("&[{elems}]");
    }
    emit_expr(expr, &HashSet::new(), &HashSet::new())
}

pub fn var_init(v: &VarDef) -> String {
    match &v.assign {
        Some(expr) => emit_expr(expr, &HashSet::new(), &HashSet::new()),
        None => var_default(v),
    }
}

pub fn var_rust_type(v: &VarDef) -> String {
    let mut ty = rust_type(&v.typ);
    for _ in &v.arrays {
        ty = format!("Vec<{ty}>");
    }
    ty
}

pub fn var_default(v: &VarDef) -> String {
    let mut val = default_value(&v.typ);
    for dim in v.arrays.iter().rev() {
        val = format!(
            "vec![{val}; ({}) as usize]",
            emit_expr(dim, &HashSet::new(), &HashSet::new())
        );
    }
    val
}

pub fn param_value(t: &Type, value: &str) -> String {
    if value.is_empty() {
        return default_value(t);
    }
    match rust_type(t).as_str() {
        "String" => format!("{value:?}.to_string()"),
        _ => value.to_string(),
    }
}

