use std::collections::HashSet;
use std::fmt::Write as _;

use crate::ast::{Expr, Generator, Stmt, VarDef};

use super::{fsm_variant, ident, port_ref, rust_type, type_ident, var_default, var_rust_type};

pub fn emit_function(f: &crate::ast::Function) -> String {
    let params = f
        .parameters
        .iter()
        .map(|p| format!("{}: {}", ident(&p.name), rust_type(&p.typ)))
        .collect::<Vec<_>>()
        .join(", ");
    let locals: HashSet<String> = f.parameters.iter().map(|p| p.name.clone()).collect();
    let body = emit_expr(&f.expr, &HashSet::new(), &locals);
    format!(
        "fn {}({params}) -> {} {{\n    {body}\n}}\n",
        ident(&f.name),
        rust_type(&f.ret_type)
    )
}

pub fn emit_procedure(p: &crate::ast::Procedure) -> String {
    let params = p
        .parameters
        .iter()
        .map(|pp| format!("{}: {}", ident(&pp.name), rust_type(&pp.typ)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut locals: HashSet<String> = p.parameters.iter().map(|pp| pp.name.clone()).collect();
    for v in &p.vars {
        locals.insert(v.name.clone());
    }
    let decls = emit_vardefs(&p.vars, &HashSet::new(), &locals);
    let body: String = p
        .stmts
        .iter()
        .map(|s| emit_stmt(s, &HashSet::new(), &locals))
        .collect();
    format!("fn {}({params}) {{\n{decls}{body}}}\n", ident(&p.name))
}

pub fn emit_vardefs(vars: &[VarDef], state: &HashSet<String>, locals: &HashSet<String>) -> String {
    let mut out = String::new();
    for v in vars {
        let init = match &v.assign {
            Some(expr) => emit_expr(expr, state, locals),
            None => var_default(v),
        };
        let _ = writeln!(
            out,
            "            let mut {}: {} = {init};",
            ident(&v.name),
            var_rust_type(v)
        );
    }
    out
}

pub fn emit_stmt(stmt: &Stmt, state: &HashSet<String>, locals: &HashSet<String>) -> String {
    match stmt {
        Stmt::If { cond, then, els } => {
            let then_block: String = then.iter().map(|s| emit_stmt(s, state, locals)).collect();
            let mut out = format!(
                "            if {} {{\n{then_block}            }}",
                emit_expr(cond, state, locals)
            );
            if !els.is_empty() {
                let else_block: String = els.iter().map(|s| emit_stmt(s, state, locals)).collect();
                let _ = write!(out, " else {{\n{else_block}            }}");
            }
            out.push('\n');
            out
        }
        Stmt::Call { name, args } if name == "println" || name == "print" => {
            emit_print(name, args, state, locals)
        }
        Stmt::Call { name, args } => {
            let args = args
                .iter()
                .map(|a| emit_expr(a, state, locals))
                .collect::<Vec<_>>()
                .join(", ");
            format!("            {}({args});\n", ident(name))
        }
        Stmt::Block { vars, stmts } => emit_scope(vars, stmts, state, locals),
        Stmt::While { cond, vars, stmts } => {
            let mut inner = locals.clone();
            inner.extend(vars.iter().map(|v| v.name.clone()));
            let decls = emit_vardefs(vars, state, &inner);
            let body: String = stmts.iter().map(|s| emit_stmt(s, state, &inner)).collect();
            format!(
                "            while {} {{\n{decls}{body}            }}\n",
                emit_expr(cond, state, locals)
            )
        }
        Stmt::Foreach {
            generators,
            vars,
            stmts,
        } => {
            let mut inner = locals.clone();
            inner.extend(generators.iter().map(|g| g.identifier.clone()));
            inner.extend(vars.iter().map(|v| v.name.clone()));
            let decls = emit_vardefs(vars, state, &inner);
            let body: String = stmts.iter().map(|s| emit_stmt(s, state, &inner)).collect();
            let mut out = String::new();
            for g in generators {
                out.push_str(&emit_for_header(g, state, locals));
            }
            out.push_str(&decls);
            out.push_str(&body);
            for _ in generators {
                out.push_str("            }\n");
            }
            out
        }
        Stmt::OutputWrite { port, expr } => format!(
            "            {}.push_back({});\n",
            port_ref(port),
            emit_expr(expr, state, locals)
        ),
        Stmt::InputRead {
            port, identifier, ..
        } => format!(
            "            let {} = {}.pop_front().unwrap();\n",
            ident(identifier),
            port_ref(port)
        ),
        Stmt::Assign {
            identifier,
            indices,
            value,
            ..
        } => {
            let mut target = resolve(identifier, state, locals);
            for index in indices {
                target = format!("{target}[({}) as usize]", emit_expr(index, state, locals));
            }
            match value {
                Some(expr) => format!(
                    "            {target} = {};\n",
                    emit_expr(expr, state, locals)
                ),
                None => String::new(),
            }
        }
        Stmt::Return => "            return false;\n".to_string(),
        Stmt::TerminateLoop => "            break;\n".to_string(),
    }
}

fn emit_scope(
    vars: &[VarDef],
    stmts: &[Stmt],
    state: &HashSet<String>,
    locals: &HashSet<String>,
) -> String {
    let mut inner = locals.clone();
    inner.extend(vars.iter().map(|v| v.name.clone()));
    let decls = emit_vardefs(vars, state, &inner);
    let body: String = stmts.iter().map(|s| emit_stmt(s, state, &inner)).collect();
    format!("            {{\n{decls}{body}            }}\n")
}

fn emit_for_header(g: &Generator, state: &HashSet<String>, locals: &HashSet<String>) -> String {
    format!(
        "            for {} in ({})..=({}) {{\n",
        ident(&g.identifier),
        emit_expr(&g.start, state, locals),
        emit_expr(&g.end, state, locals)
    )
}

fn emit_print(
    name: &str,
    args: &[Expr],
    state: &HashSet<String>,
    locals: &HashSet<String>,
) -> String {
    let macro_name = if name == "println" {
        "println"
    } else {
        "print"
    };
    let mut operands = Vec::new();
    for arg in args {
        flatten_concat(arg, &mut operands);
    }
    let fmt = "{}".repeat(operands.len());
    let rendered = operands
        .iter()
        .map(|e| emit_expr(e, state, locals))
        .collect::<Vec<_>>()
        .join(", ");
    if rendered.is_empty() {
        format!("            {macro_name}!(\"{fmt}\");\n")
    } else {
        format!("            {macro_name}!(\"{fmt}\", {rendered});\n")
    }
}

fn flatten_concat<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    match expr {
        Expr::Paren(inner) => flatten_concat(inner, out),
        Expr::BinOp { op, left, right } if op == "+" => {
            flatten_concat(left, out);
            flatten_concat(right, out);
        }
        _ => out.push(expr),
    }
}

pub fn emit_expr(expr: &Expr, state: &HashSet<String>, locals: &HashSet<String>) -> String {
    match expr {
        Expr::Paren(inner) => format!("({})", emit_expr(inner, state, locals)),
        Expr::BinOp { op, left, right } => format!(
            "{} {} {}",
            emit_expr(left, state, locals),
            map_op(op),
            emit_expr(right, state, locals)
        ),
        Expr::Literal { negation, value } => {
            if value.starts_with('"') || value.starts_with('\'') {
                value.clone()
            } else if *negation {
                format!("-{value}")
            } else {
                value.clone()
            }
        }
        Expr::Identifier {
            name,
            unary_left,
            indices,
            call,
            ..
        } => {
            let mut base = match call {
                Some(args) => {
                    let rendered = args
                        .iter()
                        .map(|a| emit_expr(a, state, locals))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}({rendered})", ident(name))
                }
                None => resolve(name, state, locals),
            };
            for index in indices {
                base = format!("{base}[({}) as usize]", emit_expr(index, state, locals));
            }
            if let Some(op) = unary_left {
                base = format!("{}{base}", map_unary(op));
            }
            base
        }
        Expr::FsmEnumElement { enum_name, element } => {
            format!("{}::{}", type_ident(enum_name), fsm_variant(element))
        }
        Expr::PortPreview { port, index, .. } => match index {
            Some(index) => format!(
                "{}[({}) as usize]",
                ident(port),
                emit_expr(index, state, locals)
            ),
            None => format!("{}[0]", ident(port)),
        },
        Expr::PortSize { port } => format!("({}.len() as i64)", ident(port)),
        Expr::PortFree { .. } => "0".to_string(),
        Expr::Ternary { cond, then, els } => format!(
            "if {} {{ {} }} else {{ {} }}",
            emit_expr(cond, state, locals),
            emit_expr(then, state, locals),
            emit_expr(els, state, locals)
        ),
        Expr::ListComprehension {
            expressions,
            generators,
        } => {
            if generators.is_empty() {
                let elems = expressions
                    .iter()
                    .map(|e| emit_expr(e, state, locals))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("vec![{elems}]")
            } else {
                let mut inner = locals.clone();
                for g in generators {
                    inner.insert(g.identifier.clone());
                }
                let mut body = String::new();
                for g in generators {
                    let _ = write!(
                        body,
                        "for {} in ({})..=({}) {{ ",
                        ident(&g.identifier),
                        emit_expr(&g.start, state, &inner),
                        emit_expr(&g.end, state, &inner)
                    );
                }
                for e in expressions {
                    let _ = write!(body, "__lc.push({}); ", emit_expr(e, state, &inner));
                }
                for _ in generators {
                    body.push_str("} ");
                }
                format!("{{ let mut __lc = Vec::new(); {body}__lc }}")
            }
        }
    }
}

fn resolve(name: &str, state: &HashSet<String>, locals: &HashSet<String>) -> String {
    if locals.contains(name) {
        ident(name)
    } else if state.contains(name) {
        format!("self.{}", ident(name))
    } else {
        ident(name)
    }
}

fn map_op(op: &str) -> &str {
    match op {
        "=" => "==",
        "and" => "&&",
        "or" => "||",
        "not" => "!",
        "mod" => "%",
        "div" => "/",
        other => other,
    }
}

fn map_unary(op: &str) -> &str {
    match op {
        "not" => "!",
        other => other,
    }
}
