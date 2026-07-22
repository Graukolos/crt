use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

use crate::ast::{Action, Actor, InputPattern};

use super::{
    emit_expr, emit_stmt, emit_vardefs, fsm_variant, ident, port_ref, rust_type, type_ident,
    var_init, var_rust_type,
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
