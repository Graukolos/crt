use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

use crate::ast::{Action, Actor, Expr, Generator, InputPattern, Stmt, Type, VarDef};
use crate::codegen::Program;
use crate::network_ffi::ffi::Instance;

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
    out.push_str(super::orcc::OPTIONS_RS);
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

fn var_default(v: &VarDef) -> String {
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

pub fn ident(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

pub fn type_ident(name: &str) -> String {
    ident(name)
}

pub fn fsm_variant(state: &str) -> String {
    format!("St_{}", ident(state))
}

pub fn inst_var(id: &str) -> String {
    format!("inst_{}", ident(id))
}

pub fn port_ref(name: &str) -> String {
    format!("port_{}", ident(name))
}

pub fn fifo_in(id: &str, port: &str) -> String {
    format!("fin_{}_{}", ident(id), ident(port))
}

pub fn fifo_out(id: &str, port: &str) -> String {
    format!("fout_{}_{}", ident(id), ident(port))
}

pub fn actor_mod(name: &str) -> String {
    format!("m_{}", ident(name))
}

pub fn emit_shared_decls(program: &Program<'_>) -> String {
    let mut out = String::new();

    let mut consts = String::new();
    for unit in program.units {
        for v in &unit.vars {
            consts.push_str(&emit_const(v));
        }
    }
    if !consts.is_empty() {
        out.push_str(&consts);
        out.push('\n');
    }

    if program.has_natives() {
        out.push_str(&emit_natives(program));
        out.push('\n');
    }

    let mut funcs = String::new();
    let mut seen_fns: HashSet<String> = HashSet::new();
    for unit in program.units {
        for f in &unit.functions {
            if seen_fns.insert(f.name.clone()) {
                funcs.push_str(&emit_function(f));
            }
        }
        for p in &unit.procedures {
            if seen_fns.insert(p.name.clone()) {
                funcs.push_str(&emit_procedure(p));
            }
        }
    }
    for actor in program.actors.values() {
        for f in &actor.functions {
            if seen_fns.insert(f.name.clone()) {
                funcs.push_str(&emit_function(f));
            }
        }
        for p in &actor.procedures {
            if seen_fns.insert(p.name.clone()) {
                funcs.push_str(&emit_procedure(p));
            }
        }
    }
    if !funcs.is_empty() {
        out.push_str(&funcs);
        out.push('\n');
    }

    out
}

pub fn instance_args(inst: &Instance, actor: &Actor) -> String {
    actor
        .parameters
        .iter()
        .map(|p| {
            let value = inst.parameters.iter().find(|param| param.key == p.name);
            match value {
                Some(param) => param_value(&p.typ, &param.value),
                None => match &p.default {
                    Some(expr) => emit_expr(expr, &HashSet::new(), &HashSet::new()),
                    None => default_value(&p.typ),
                },
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn emit_actor(actor: &Actor) -> String {
    let ty = type_ident(&actor.name);
    let state = actor_state(actor);
    let mut out = String::new();

    if let Some(fsm) = &actor.fsm {
        let mut states = BTreeSet::new();
        states.insert(fsm.initial_state.clone());
        for t in &fsm.transitions {
            states.insert(t.state.clone());
            states.insert(t.next.clone());
        }
        let variants = states
            .iter()
            .map(|s| format!("    {},", fsm_variant(s)))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = write!(
            out,
            "#[derive(Clone, Copy)]\nenum {ty}State {{\n{variants}\n}}\n\n"
        );
    }

    let mut fields = Vec::new();
    for p in &actor.parameters {
        fields.push(format!("    {}: {},", ident(&p.name), rust_type(&p.typ)));
    }
    for v in &actor.vars {
        fields.push(format!("    {}: {},", ident(&v.name), var_rust_type(v)));
    }
    if actor.fsm.is_some() {
        fields.push(format!("    state: {ty}State,"));
    }
    let _ = write!(out, "pub struct {ty} {{\n{}\n}}\n\n", fields.join("\n"));

    let params = actor
        .parameters
        .iter()
        .map(|p| format!("{}: {}", ident(&p.name), rust_type(&p.typ)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut lets = String::new();
    for v in &actor.vars {
        let _ = writeln!(lets, "        let {} = {};", ident(&v.name), var_init(v));
    }
    let mut inits = Vec::new();
    for p in &actor.parameters {
        inits.push(format!("            {},", ident(&p.name)));
    }
    for v in &actor.vars {
        inits.push(format!("            {},", ident(&v.name)));
    }
    if let Some(fsm) = &actor.fsm {
        inits.push(format!(
            "            state: {ty}State::{},",
            fsm_variant(&fsm.initial_state)
        ));
    }
    let _ = write!(
        out,
        "impl {ty} {{\n    pub fn new({params}) -> Self {{\n{lets}        Self {{\n{}\n        }}\n    }}\n\n",
        inits.join("\n")
    );

    if let Some(init) = &actor.init {
        let body = emit_action_body(init, &state, None);
        let _ = write!(
            out,
            "    pub fn init(&mut self{}) {{\n{body}\n    }}\n\n",
            port_params(actor)
        );
    }

    let _ = write!(
        out,
        "    pub fn fire(&mut self{}) -> bool {{\n{}\n        false\n    }}\n}}\n",
        port_params(actor),
        emit_fire(actor, &state, &ty)
    );

    out
}

pub fn actor_state(actor: &Actor) -> HashSet<String> {
    actor
        .parameters
        .iter()
        .map(|p| p.name.clone())
        .chain(actor.vars.iter().map(|v| v.name.clone()))
        .collect()
}

pub fn port_params(actor: &Actor) -> String {
    let mut out = String::new();
    for port in actor.inports.iter().chain(&actor.outports) {
        let _ = write!(
            out,
            ", {}: &mut VecDeque<{}>",
            port_ref(&port.name),
            rust_type(&port.typ)
        );
    }
    out
}

fn emit_fire(actor: &Actor, state: &HashSet<String>, ty: &str) -> String {
    let lookup = |name: &str| actor.actions.iter().find(|a| a.name == name);

    if let Some(fsm) = &actor.fsm {
        let mut states = BTreeSet::new();
        for t in &fsm.transitions {
            states.insert(t.state.clone());
        }
        let mut arms = String::new();
        for s in &states {
            let mut tries = String::new();
            for t in fsm.transitions.iter().filter(|t| &t.state == s) {
                let next = format!("self.state = {ty}State::{};", fsm_variant(&t.next));
                for action_name in &t.actions {
                    if let Some(action) = lookup(action_name) {
                        tries.push_str(&emit_action(action, state, Some(&next)));
                    }
                }
            }
            let _ = write!(
                arms,
                "            {ty}State::{} => {{\n{tries}\n            }}\n",
                fsm_variant(s)
            );
        }
        format!("        match self.state {{\n{arms}        }}")
    } else {
        actor
            .actions
            .iter()
            .map(|a| emit_action(a, state, None))
            .collect()
    }
}

fn pattern_token_count(
    p: &InputPattern,
    state: &HashSet<String>,
    locals: &HashSet<String>,
) -> String {
    match &p.repeat {
        Some(repeat) => format!(
            "({} * ({})) as usize",
            p.ids.len(),
            emit_expr(repeat, state, locals)
        ),
        None => p.ids.len().to_string(),
    }
}

fn emit_action(action: &Action, state: &HashSet<String>, fsm_next: Option<&str>) -> String {
    let body = emit_action_body(action, state, fsm_next);
    let commit = format!("{{\n{body}\n            return true;\n        }}");

    let mut locals = HashSet::new();
    for pattern in &action.input_patterns {
        for id in &pattern.ids {
            locals.insert(id.clone());
        }
    }
    for v in &action.vars {
        locals.insert(v.name.clone());
    }

    let guarded = if action.guards.is_empty() {
        commit
    } else {
        let cond = action
            .guards
            .iter()
            .map(|g| emit_expr(g, state, &locals))
            .collect::<Vec<_>>()
            .join(" && ");
        format!("if {cond} {commit}")
    };

    if action.input_patterns.is_empty() {
        return format!("        {guarded}\n");
    }

    let avail = action
        .input_patterns
        .iter()
        .map(|p| {
            format!(
                "{}.len() >= {}",
                port_ref(&p.port),
                pattern_token_count(p, state, &locals)
            )
        })
        .collect::<Vec<_>>()
        .join(" && ");
    let mut peeks = String::new();
    for pattern in &action.input_patterns {
        let stride = pattern.ids.len();
        for (i, id) in pattern.ids.iter().enumerate() {
            if let Some(repeat) = &pattern.repeat {
                let _ = writeln!(
                    peeks,
                    "            let mut {}: Vec<_> = ({i}..({stride} * ({})) as usize).step_by({stride}).map(|__j| {}[__j]).collect();",
                    ident(id),
                    emit_expr(repeat, state, &locals),
                    port_ref(&pattern.port)
                );
            } else {
                let _ = writeln!(
                    peeks,
                    "            let mut {} = {}[{i}];",
                    ident(id),
                    port_ref(&pattern.port)
                );
            }
        }
    }
    format!("        if {avail} {{\n{peeks}            {guarded}\n        }}\n")
}

fn emit_action_body(action: &Action, state: &HashSet<String>, fsm_next: Option<&str>) -> String {
    let mut locals: HashSet<String> = action
        .input_patterns
        .iter()
        .flat_map(|p| p.ids.iter().cloned())
        .collect();
    let mut out = String::new();

    for pattern in &action.input_patterns {
        let _ = writeln!(
            out,
            "            for _ in 0..{} {{ {}.pop_front(); }}",
            pattern_token_count(pattern, state, &locals),
            port_ref(&pattern.port)
        );
    }

    for v in &action.vars {
        locals.insert(v.name.clone());
    }
    out.push_str(&emit_vardefs(&action.vars, state, &locals));

    for stmt in &action.stmts {
        out.push_str(&emit_stmt(stmt, state, &locals));
    }
    for output in &action.output_expressions {
        for expr in &output.expressions {
            if output.repeat.is_some() {
                let _ = writeln!(
                    out,
                    "            for __tok in ({}).clone() {{ {}.push_back(__tok); }}",
                    emit_expr(expr, state, &locals),
                    port_ref(&output.port)
                );
            } else {
                let _ = writeln!(
                    out,
                    "            {}.push_back({});",
                    port_ref(&output.port),
                    emit_expr(expr, state, &locals)
                );
            }
        }
    }

    if let Some(transition) = fsm_next {
        let _ = writeln!(out, "            {transition}");
    }
    out
}
