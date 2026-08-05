use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

use crate::ast::{Action, Actor, Expr, InputPattern, ScheduleFsm, Stmt};

use super::{
    Priorities, emit_expr, emit_stmt, emit_vardefs, fsm_variant, fsm_wrapper, ident, port_field,
    port_ref, rust_type, type_ident, var_init, var_rust_type,
};

pub fn emit_actor(actor: &Actor, typestate: bool) -> String {
    if typestate && actor.fsm.is_some() && !typestate_actor(actor, typestate) {
        eprintln!(
            "warning: actor {}: 'return' inside an action is unsupported by --typestate; emitting the value-based FSM for this actor",
            actor.name
        );
    }
    if typestate_actor(actor, typestate) {
        return emit_actor_typestate(actor);
    }

    let ty = type_ident(&actor.name);
    let state = actor_state(actor);
    let mut out = String::new();

    if let Some(fsm) = &actor.fsm {
        let states = fsm_states(fsm);
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

    let params = ctor_params(actor);
    let lets = ctor_var_lets(actor);
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

fn fsm_states(fsm: &ScheduleFsm) -> BTreeSet<String> {
    let mut states = BTreeSet::new();
    states.insert(fsm.initial_state.clone());
    for t in &fsm.transitions {
        states.insert(t.state.clone());
        states.insert(t.next.clone());
    }
    states
}

fn stmts_return(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        Stmt::Return => true,
        Stmt::If { then, els, .. } => stmts_return(then) || stmts_return(els),
        Stmt::Block { stmts, .. } | Stmt::While { stmts, .. } | Stmt::Foreach { stmts, .. } => {
            stmts_return(stmts)
        }
        _ => false,
    })
}

pub fn typestate_actor(actor: &Actor, typestate: bool) -> bool {
    typestate
        && actor.fsm.is_some()
        && !actor
            .actions
            .iter()
            .chain(actor.init.iter())
            .any(|action| stmts_return(&action.stmts))
}

pub fn actor_type(actor: &Actor, typestate: bool) -> String {
    if typestate_actor(actor, typestate) {
        fsm_wrapper(&actor.name)
    } else {
        type_ident(&actor.name)
    }
}

pub fn actor_port(actor: &Actor, typestate: bool, owner: &str, port: &str) -> String {
    if typestate_actor(actor, typestate) {
        format!("{owner}.{}_mut()", port_field(port))
    } else {
        format!("{owner}.{}", port_field(port))
    }
}

fn ctor_params(actor: &Actor) -> String {
    let mut params: Vec<String> = actor
        .parameters
        .iter()
        .map(|p| format!("{}: {}", ident(&p.name), rust_type(&p.typ)))
        .collect();
    for (name, ty) in port_types(actor) {
        params.push(format!("{}: {ty}", port_field(&name)));
    }
    params.join(", ")
}

fn ctor_args(actor: &Actor) -> String {
    let mut args: Vec<String> = actor.parameters.iter().map(|p| ident(&p.name)).collect();
    for (name, _) in port_types(actor) {
        args.push(port_field(&name));
    }
    args.join(", ")
}

fn ctor_var_lets(actor: &Actor) -> String {
    let mut lets = String::new();
    for v in &actor.vars {
        let _ = writeln!(lets, "        let {} = {};", ident(&v.name), var_init(v));
    }
    lets
}

fn field_names(actor: &Actor) -> Vec<String> {
    let mut names: Vec<String> = actor.parameters.iter().map(|p| ident(&p.name)).collect();
    names.extend(actor.vars.iter().map(|v| ident(&v.name)));
    names.extend(port_types(actor).into_iter().map(|(n, _)| port_field(&n)));
    names
}

fn emit_actor_typestate(actor: &Actor) -> String {
    let ty = type_ident(&actor.name);
    let wrapper = fsm_wrapper(&actor.name);
    let fsm = actor.fsm.as_ref().expect("typestate requires an fsm");
    let state = actor_state(actor);
    let states = fsm_states(fsm);
    let names = field_names(actor);
    let mut out = String::new();

    for s in &states {
        let _ = writeln!(out, "pub struct {};", fsm_variant(s));
    }
    out.push('\n');

    let mut fields = Vec::new();
    for p in &actor.parameters {
        fields.push(format!("    {}: {},", ident(&p.name), rust_type(&p.typ)));
    }
    for v in &actor.vars {
        fields.push(format!("    {}: {},", ident(&v.name), var_rust_type(v)));
    }
    for (name, port_ty) in port_types(actor) {
        fields.push(format!("    pub {}: {port_ty},", port_field(&name)));
    }
    fields.push("    __state: core::marker::PhantomData<S>,".to_string());
    let _ = write!(out, "pub struct {ty}<S> {{\n{}\n}}\n\n", fields.join("\n"));

    let inits = names.iter().fold(String::new(), |mut acc, n| {
        let _ = writeln!(acc, "            {n},");
        acc
    });
    let _ = write!(
        out,
        "impl<S> {ty}<S> {{\n    pub fn new({}) -> Self {{\n{}        Self {{\n{inits}            __state: core::marker::PhantomData,\n        }}\n    }}\n\n",
        ctor_params(actor),
        ctor_var_lets(actor)
    );

    let moves = names.iter().fold(String::new(), |mut acc, n| {
        let _ = writeln!(acc, "            {n}: self.{n},");
        acc
    });
    let _ = write!(
        out,
        "    fn into_state<__T>(self) -> {ty}<__T> {{\n        {ty} {{\n{moves}            __state: core::marker::PhantomData,\n        }}\n    }}\n"
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
            "\n    pub fn init(&mut self) {{\n{}{}    }}\n",
            room_snapshots(std::iter::once(init)),
            emit_action(init, &state, None, Commit::Fallthrough)
        );
    }
    out.push_str("}\n\n");

    let lookup = |name: &str| actor.actions.iter().find(|a| a.name == name);
    let priorities = Priorities::new(actor);
    for s in &states {
        let here = fsm_variant(s);
        let (reachable, nexts) = state_candidates(fsm, s, lookup, |next| {
            format!("{wrapper}::{}", fsm_variant(next))
        });
        let mut tries = String::new();
        for i in priorities.order(actor, &reachable) {
            tries.push_str(&emit_action(
                reachable[i],
                &state,
                None,
                Commit::Move(nexts[i].as_str()),
            ));
        }
        let _ = write!(
            out,
            "impl {ty}<{here}> {{\n    fn step(mut self) -> ({wrapper}, bool) {{\n{}{tries}        ({wrapper}::{here}(self), false)\n    }}\n}}\n\n",
            room_snapshots(reachable.into_iter())
        );
    }

    out.push_str(&emit_fsm_wrapper(actor, &states));
    out
}

fn emit_fsm_wrapper(actor: &Actor, states: &BTreeSet<String>) -> String {
    let ty = type_ident(&actor.name);
    let wrapper = fsm_wrapper(&actor.name);
    let fsm = actor.fsm.as_ref().expect("typestate requires an fsm");
    let mut out = String::new();

    let variants = states.iter().fold(String::new(), |mut acc, s| {
        let v = fsm_variant(s);
        let _ = writeln!(acc, "    {v}({ty}<{v}>),");
        acc
    });
    let _ = write!(
        out,
        "pub enum {wrapper} {{\n{variants}    __Moving,\n}}\n\n"
    );

    let _ = write!(
        out,
        "impl {wrapper} {{\n    pub fn new({}) -> Self {{\n        Self::{}({ty}::new({}))\n    }}\n\n",
        ctor_params(actor),
        fsm_variant(&fsm.initial_state),
        ctor_args(actor)
    );

    let dispatch = |body: &str| -> String {
        states.iter().fold(String::new(), |mut acc, s| {
            let _ = writeln!(acc, "            Self::{}(__s) => {body},", fsm_variant(s));
            acc
        })
    };

    if actor.init.is_some() {
        let _ = write!(
            out,
            "    pub fn init(&mut self) {{\n        match self {{\n{}            Self::__Moving => {{}}\n        }}\n    }}\n\n",
            dispatch("__s.init()")
        );
    }

    let _ = write!(
        out,
        "    pub fn fire(&mut self) -> bool {{\n        let (__next, __fired) = match core::mem::replace(self, Self::__Moving) {{\n{}            Self::__Moving => unreachable!(),\n        }};\n        *self = __next;\n        __fired\n    }}\n",
        dispatch("__s.step()")
    );

    for (name, port_ty) in port_types(actor) {
        let field = port_field(&name);
        let _ = write!(
            out,
            "\n    pub fn {field}_mut(&mut self) -> &mut {port_ty} {{\n        match self {{\n{}            Self::__Moving => unreachable!(),\n        }}\n    }}\n",
            dispatch(&format!("&mut __s.{field}"))
        );
    }
    out.push_str("}\n");

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
    let priorities = Priorities::new(actor);

    let Some(fsm) = &actor.fsm else {
        let candidates: Vec<&Action> = actor.actions.iter().collect();
        let body: String = priorities
            .order(actor, &candidates)
            .into_iter()
            .map(|i| emit_action(candidates[i], state, None, Commit::Fired))
            .collect();
        return format!("{}{body}", room_snapshots(actor.actions.iter()));
    };

    let mut states = BTreeSet::new();
    for t in &fsm.transitions {
        states.insert(t.state.clone());
    }
    let mut arms = String::new();
    for s in &states {
        let (reachable, nexts) = state_candidates(fsm, s, lookup, |next| {
            format!("self.state = {ty}State::{};", fsm_variant(next))
        });
        let mut tries = String::new();
        for i in priorities.order(actor, &reachable) {
            tries.push_str(&emit_action(
                reachable[i],
                state,
                Some(&nexts[i]),
                Commit::Fired,
            ));
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

fn state_candidates<'a>(
    fsm: &ScheduleFsm,
    state: &str,
    lookup: impl Fn(&str) -> Option<&'a Action>,
    next_code: impl Fn(&str) -> String,
) -> (Vec<&'a Action>, Vec<String>) {
    let mut actions = Vec::new();
    let mut nexts = Vec::new();
    for t in fsm.transitions.iter().filter(|t| t.state == state) {
        for action_name in &t.actions {
            if let Some(action) = lookup(action_name) {
                actions.push(action);
                nexts.push(next_code(&t.next));
            }
        }
    }
    (actions, nexts)
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
enum Commit<'a> {
    Fired,
    Fallthrough,
    Move(&'a str),
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
    commit: Commit<'_>,
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
        Commit::Fired => "            return true;\n".to_string(),
        Commit::Fallthrough => String::new(),
        Commit::Move(next) => format!("            return ({next}(self.into_state()), true);\n"),
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
