use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

use crate::ast::{Action, Actor, Expr, InputPattern};

use super::{
    emit_expr, emit_stmt, emit_vardefs, fsm_variant, ident, port_field, port_ref, rust_type,
    type_ident, var_init, var_rust_type,
};

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
    for (name, ty) in port_types(actor) {
        fields.push(format!("    pub {}: {ty},", port_field(&name)));
    }
    let _ = write!(out, "pub struct {ty} {{\n{}\n}}\n\n", fields.join("\n"));

    let mut params: Vec<String> = actor
        .parameters
        .iter()
        .map(|p| format!("{}: {}", ident(&p.name), rust_type(&p.typ)))
        .collect();
    for (name, ty) in port_types(actor) {
        params.push(format!("{}: {ty}", port_field(&name)));
    }
    let params = params.join(", ");
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
    for (name, _) in port_types(actor) {
        inits.push(format!("            {},", port_field(&name)));
    }
    let _ = write!(
        out,
        "impl {ty} {{\n    pub fn new({params}) -> Self {{\n{lets}        Self {{\n{}\n        }}\n    }}\n\n",
        inits.join("\n")
    );

    if let Some(init) = &actor.init {
        for pattern in &init.input_patterns {
            eprintln!(
                "warning: actor {}: initialize action consumes from port {}; it fires only if tokens are already available at startup",
                actor.name, pattern.port
            );
        }
        let _ = write!(
            out,
            "    pub fn init(&mut self) {{\n{}{}    }}\n\n",
            room_snapshots(std::iter::once(init)),
            emit_action(init, &state, None, Commit::Fallthrough)
        );
    }

    let _ = write!(
        out,
        "    pub fn fire(&mut self) -> bool {{\n{}\n        false\n    }}\n}}\n",
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

pub fn port_types(actor: &Actor) -> Vec<(String, String)> {
    let ins = actor
        .inports
        .iter()
        .map(|p| (p.name.clone(), format!("InPort<{}>", rust_type(&p.typ))));
    let outs = actor
        .outports
        .iter()
        .map(|p| (p.name.clone(), format!("OutPort<{}>", rust_type(&p.typ))));
    ins.chain(outs).collect()
}

fn room_snapshot(port: &str) -> String {
    format!("__room_{}", port_field(port))
}

fn room_snapshots<'a>(actions: impl Iterator<Item = &'a Action>) -> String {
    let mut ports = BTreeSet::new();
    for action in actions {
        for output in &action.output_expressions {
            ports.insert(output.port.clone());
        }
    }
    let mut out = String::new();
    for port in ports {
        let _ = writeln!(
            out,
            "        let {} = {}.has_room();",
            room_snapshot(&port),
            port_ref(&port)
        );
    }
    out
}

fn emit_fire(actor: &Actor, state: &HashSet<String>, ty: &str) -> String {
    let lookup = |name: &str| actor.actions.iter().find(|a| a.name == name);

    let Some(fsm) = &actor.fsm else {
        let body: String = actor
            .actions
            .iter()
            .map(|a| emit_action(a, state, None, Commit::Fired))
            .collect();
        return format!("{}{body}", room_snapshots(actor.actions.iter()));
    };

    let mut states = BTreeSet::new();
    for t in &fsm.transitions {
        states.insert(t.state.clone());
    }
    let mut arms = String::new();
    for s in &states {
        let mut reachable = Vec::new();
        let mut tries = String::new();
        for t in fsm.transitions.iter().filter(|t| &t.state == s) {
            let next = format!("self.state = {ty}State::{};", fsm_variant(&t.next));
            for action_name in &t.actions {
                if let Some(action) = lookup(action_name) {
                    reachable.push(action);
                    tries.push_str(&emit_action(action, state, Some(&next), Commit::Fired));
                }
            }
        }
        let _ = write!(
            arms,
            "            {ty}State::{} => {{\n{}{tries}\n            }}\n",
            fsm_variant(s),
            room_snapshots(reachable.into_iter())
        );
    }
    format!("        match self.state {{\n{arms}        }}")
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

#[derive(Copy, Clone, PartialEq, Eq)]
enum Commit {
    Fired,
    Fallthrough,
}

fn reads_tokens(expr: &Expr, tokens: &HashSet<String>) -> bool {
    match expr {
        Expr::Paren(inner) => reads_tokens(inner, tokens),
        Expr::BinOp { left, right, .. } => {
            reads_tokens(left, tokens) || reads_tokens(right, tokens)
        }
        Expr::Literal { .. } | Expr::FsmEnumElement { .. } => false,
        Expr::Identifier {
            name,
            indices,
            call,
            ..
        } => {
            tokens.contains(name)
                || indices.iter().any(|e| reads_tokens(e, tokens))
                || call.iter().flatten().any(|e| reads_tokens(e, tokens))
        }
        Expr::PortPreview { .. } | Expr::PortSize { .. } | Expr::PortFree { .. } => true,
        Expr::Ternary { cond, then, els } => {
            reads_tokens(cond, tokens) || reads_tokens(then, tokens) || reads_tokens(els, tokens)
        }
        Expr::ListComprehension {
            expressions,
            generators,
        } => {
            expressions.iter().any(|e| reads_tokens(e, tokens))
                || generators
                    .iter()
                    .any(|g| reads_tokens(&g.start, tokens) || reads_tokens(&g.end, tokens))
        }
    }
}

fn emit_recvs(action: &Action, state: &HashSet<String>, locals: &HashSet<String>) -> String {
    let mut out = String::new();
    for pattern in &action.input_patterns {
        let port = port_ref(&pattern.port);
        match &pattern.repeat {
            Some(repeat) => {
                let n = format!("__n_{}", port_field(&pattern.port));
                let _ = writeln!(
                    out,
                    "            let {n} = ({}) as usize;",
                    emit_expr(repeat, state, locals)
                );
                for id in &pattern.ids {
                    let _ = writeln!(
                        out,
                        "            let mut {}: Vec<_> = Vec::with_capacity({n});",
                        ident(id)
                    );
                }
                let _ = write!(out, "            for _ in 0..{n} {{");
                for id in &pattern.ids {
                    let _ = write!(out, " {}.push({port}.recv());", ident(id));
                }
                out.push_str(" }\n");
            }
            None => {
                for id in &pattern.ids {
                    let _ = writeln!(out, "            let mut {} = {port}.recv();", ident(id));
                }
            }
        }
    }
    out
}

fn emit_peeks(action: &Action, state: &HashSet<String>, locals: &HashSet<String>) -> String {
    let mut out = String::new();
    for pattern in &action.input_patterns {
        let stride = pattern.ids.len();
        for (i, id) in pattern.ids.iter().enumerate() {
            if let Some(repeat) = &pattern.repeat {
                let _ = writeln!(
                    out,
                    "            let mut {}: Vec<_> = ({i}..({stride} * ({})) as usize).step_by({stride}).map(|__j| {}.peek(__j)).collect();",
                    ident(id),
                    emit_expr(repeat, state, locals),
                    port_ref(&pattern.port)
                );
            } else {
                let _ = writeln!(
                    out,
                    "            let mut {} = {}.peek({i});",
                    ident(id),
                    port_ref(&pattern.port)
                );
            }
        }
    }
    out
}

fn emit_action(
    action: &Action,
    state: &HashSet<String>,
    fsm_next: Option<&str>,
    commit: Commit,
) -> String {
    let mut locals = HashSet::new();
    let mut tokens = HashSet::new();
    for pattern in &action.input_patterns {
        for id in &pattern.ids {
            locals.insert(id.clone());
            tokens.insert(id.clone());
        }
    }
    for v in &action.vars {
        locals.insert(v.name.clone());
    }

    let consume = !action.guards.iter().any(|g| reads_tokens(g, &tokens));

    let body = emit_action_body(action, state, fsm_next, !consume);
    let tail = match commit {
        Commit::Fired => "            return true;\n",
        Commit::Fallthrough => "",
    };

    let guard = if action.guards.is_empty() {
        None
    } else {
        Some(
            action
                .guards
                .iter()
                .map(|g| emit_expr(g, state, &locals))
                .collect::<Vec<_>>()
                .join(" && "),
        )
    };

    let mut conds: Vec<String> = action
        .input_patterns
        .iter()
        .map(|p| {
            format!(
                "{}.avail({})",
                port_ref(&p.port),
                pattern_token_count(p, state, &locals)
            )
        })
        .collect();
    let mut produced = BTreeSet::new();
    for output in &action.output_expressions {
        if produced.insert(output.port.clone()) {
            conds.push(room_snapshot(&output.port));
        }
    }
    if consume && let Some(guard) = &guard {
        conds.push(guard.clone());
    }

    if conds.is_empty() {
        if action.vars.is_empty() {
            return format!("{body}{tail}");
        }
        return format!("        {{\n{body}{tail}        }}\n");
    }
    let avail = conds.join(" && ");

    if consume {
        let recvs = emit_recvs(action, state, &locals);
        return format!("        if {avail} {{\n{recvs}{body}{tail}        }}\n");
    }

    let peeks = emit_peeks(action, state, &locals);
    let guarded = match &guard {
        Some(guard) => format!("if {guard} {{\n{body}{tail}            }}"),
        None => format!("{{\n{body}{tail}            }}"),
    };
    format!("        if {avail} {{\n{peeks}            {guarded}\n        }}\n")
}

fn emit_action_body(
    action: &Action,
    state: &HashSet<String>,
    fsm_next: Option<&str>,
    pop_inputs: bool,
) -> String {
    let mut locals: HashSet<String> = action
        .input_patterns
        .iter()
        .flat_map(|p| p.ids.iter().cloned())
        .collect();
    let mut out = String::new();

    if pop_inputs {
        for pattern in &action.input_patterns {
            let _ = writeln!(
                out,
                "            for _ in 0..{} {{ {}.pop_front(); }}",
                pattern_token_count(pattern, state, &locals),
                port_ref(&pattern.port)
            );
        }
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
