use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;

use crate::ast::{Action, Actor, Expr, InputPattern, ScheduleFsm, Stmt};

use super::{
    Priorities, emit_const_expr, emit_expr, emit_stmt, emit_vardefs, fsm_variant, fsm_wrapper,
    ident, port_field, port_ref, rust_type, var_init, var_rust_type,
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

    let ty = ident(&actor.name);
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
            emit_action(actor, init, &state, None, Commit::Fallthrough)
        );
    }

    let _ = write!(
        out,
        "    pub fn schedule(&mut self) -> usize {{\n{}    }}\n}}\n",
        emit_schedule(actor, &state, &ty)
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

fn walk_stmts(stmts: &[Stmt], visit: &mut impl FnMut(&Stmt)) {
    for stmt in stmts {
        visit(stmt);
        match stmt {
            Stmt::If { then, els, .. } => {
                walk_stmts(then, visit);
                walk_stmts(els, visit);
            }
            Stmt::Block { stmts, .. } | Stmt::While { stmts, .. } | Stmt::Foreach { stmts, .. } => {
                walk_stmts(stmts, visit);
            }
            _ => {}
        }
    }
}

fn stmts_return(stmts: &[Stmt]) -> bool {
    let mut found = false;
    walk_stmts(stmts, &mut |stmt| found |= matches!(stmt, Stmt::Return));
    found
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
        ident(&actor.name)
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
    let ty = ident(&actor.name);
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
            emit_action(actor, init, &state, None, Commit::Fallthrough)
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
                actor,
                reachable[i],
                &state,
                None,
                Commit::Move(nexts[i].as_str()),
            ));
        }
        let _ = write!(
            out,
            "impl {ty}<{here}> {{\n    fn step(mut self, {COUNTERS}: &mut {counters}) -> ({wrapper}, bool) {{\n{tries}        ({wrapper}::{here}(self), false)\n    }}\n}}\n\n",
            counters = counters_type(&actor.name)
        );
    }

    out.push_str(&emit_counters_struct(actor));
    out.push_str(&emit_fsm_wrapper(actor, &states));
    out
}

const COUNTERS: &str = "__c";

fn counters_type(name: &str) -> String {
    format!("{}Counters", ident(name))
}

fn emit_counters_struct(actor: &Actor) -> String {
    let fields = actor
        .inports
        .iter()
        .map(|p| avail_counter("", &p.name))
        .chain(actor.outports.iter().map(|p| room_snapshot("", &p.name)))
        .fold(String::new(), |mut acc, field| {
            let _ = writeln!(acc, "    {field}: usize,");
            acc
        });
    format!(
        "pub struct {} {{\n{fields}}}\n\n",
        counters_type(&actor.name)
    )
}

fn counters_init(actor: &Actor) -> String {
    let fields = actor
        .inports
        .iter()
        .map(|p| {
            format!(
                "            {}: self.{}_mut().len(),\n",
                avail_counter("", &p.name),
                port_field(&p.name)
            )
        })
        .chain(actor.outports.iter().map(|p| {
            format!(
                "            {}: self.{}_mut().room(),\n",
                room_snapshot("", &p.name),
                port_field(&p.name)
            )
        }))
        .collect::<String>();
    format!(
        "        let mut {COUNTERS} = {} {{\n{fields}        }};\n",
        counters_type(&actor.name)
    )
}

fn emit_fsm_wrapper(actor: &Actor, states: &BTreeSet<String>) -> String {
    let ty = ident(&actor.name);
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
        "    fn fire(&mut self, {COUNTERS}: &mut {}) -> bool {{\n        let (__next, __fired) = match core::mem::replace(self, Self::__Moving) {{\n{}            Self::__Moving => unreachable!(),\n        }};\n        *self = __next;\n        __fired\n    }}\n",
        counters_type(&actor.name),
        dispatch(&format!("__s.step({COUNTERS})"))
    );

    let _ = write!(
        out,
        "\n    pub fn schedule(&mut self) -> usize {{\n{}        let mut __fired = 0usize;\n        while __fired < FIRE_BUDGET && self.fire(&mut {COUNTERS}) {{\n            __fired += 1;\n        }}\n        __fired\n    }}\n",
        counters_init(actor)
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

fn room_snapshot(prefix: &str, port: &str) -> String {
    format!("{prefix}__room_{}", port_field(port))
}

fn avail_counter(prefix: &str, port: &str) -> String {
    format!("{prefix}__avail_{}", port_field(port))
}

fn stmt_ports(stmts: &[Stmt], touched: &mut BTreeSet<String>) {
    walk_stmts(stmts, &mut |stmt| {
        if let Stmt::OutputWrite { port, .. } | Stmt::InputRead { port, .. } = stmt {
            touched.insert(port.clone());
        }
    });
}

fn child_exprs(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::Paren(inner) => vec![inner],
        Expr::BinOp { left, right, .. } => vec![left, right],
        Expr::Ternary { cond, then, els } => vec![cond, then, els],
        Expr::Identifier { indices, call, .. } => {
            indices.iter().chain(call.iter().flatten()).collect()
        }
        Expr::ListComprehension {
            expressions,
            generators,
        } => expressions
            .iter()
            .chain(generators.iter().flat_map(|g| [&g.start, &g.end]))
            .collect(),
        Expr::PortPreview { index, .. } => index.iter().map(AsRef::as_ref).collect(),
        Expr::Literal { .. }
        | Expr::FsmEnumElement { .. }
        | Expr::PortSize { .. }
        | Expr::PortFree { .. } => Vec::new(),
    }
}

fn reads_ports_or(expr: &Expr, names: &HashSet<String>) -> bool {
    match expr {
        Expr::PortPreview { .. } | Expr::PortSize { .. } | Expr::PortFree { .. } => true,
        Expr::Identifier { name, .. } if names.contains(name) => true,
        _ => child_exprs(expr)
            .into_iter()
            .any(|child| reads_ports_or(child, names)),
    }
}

fn guard_time_evaluable(expr: &Expr, locals: &HashSet<String>) -> bool {
    !reads_ports_or(expr, locals)
}

pub fn output_burst(actor: &Actor, port: &str) -> Option<String> {
    let state = actor_state(actor);
    let mut counts: Vec<String> = Vec::new();
    for action in actor.actions.iter().chain(actor.init.iter()) {
        let mut touched = BTreeSet::new();
        stmt_ports(&action.stmts, &mut touched);
        if touched.contains(port) {
            return None;
        }
        let mut total: Vec<String> = Vec::new();
        for output in action.output_expressions.iter().filter(|o| o.port == port) {
            match &output.repeat {
                None => total.push(output.expressions.len().to_string()),
                Some(repeat) if guard_time_evaluable(repeat, &state) => total.push(format!(
                    "({} * ({})) as usize",
                    output.expressions.len(),
                    emit_const_expr(repeat)
                )),
                Some(_) => return None,
            }
        }
        if !total.is_empty() {
            counts.push(total.join(" + "));
        }
    }
    counts
        .into_iter()
        .reduce(|acc, count| format!("core::cmp::max({acc}, {count})"))
}

struct Rates {
    inputs: Vec<(String, String)>,
    outputs: Vec<(String, Option<String>)>,
    refresh_all: bool,
}

fn action_rates(action: &Action, state: &HashSet<String>, locals: &HashSet<String>) -> Rates {
    let mut touched = BTreeSet::new();
    stmt_ports(&action.stmts, &mut touched);

    let mut inputs: BTreeMap<String, String> = BTreeMap::new();
    for p in &action.input_patterns {
        let count = pattern_token_count(p, state, locals);
        inputs
            .entry(p.port.clone())
            .and_modify(|acc| *acc = format!("{acc} + {count}"))
            .or_insert(count);
    }

    let empty = HashSet::new();
    let mut outputs: BTreeMap<String, Option<Vec<String>>> = BTreeMap::new();
    for output in &action.output_expressions {
        let count = match &output.repeat {
            None => Some(output.expressions.len().to_string()),
            Some(repeat) if guard_time_evaluable(repeat, locals) => Some(format!(
                "({} * ({})) as usize",
                output.expressions.len(),
                emit_expr(repeat, state, &empty)
            )),
            Some(_) => None,
        };
        let slot = outputs
            .entry(output.port.clone())
            .or_insert_with(|| Some(Vec::new()));
        match (slot.as_mut(), count) {
            (Some(sum), Some(count)) => sum.push(count),
            _ => *slot = None,
        }
    }

    Rates {
        inputs: inputs.into_iter().collect(),
        outputs: outputs
            .into_iter()
            .map(|(port, sum)| (port, sum.map(|parts| parts.join(" + "))))
            .collect(),
        refresh_all: !touched.is_empty(),
    }
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
            room_snapshot("", &port),
            port_ref(&port)
        );
    }
    out
}

fn emit_schedule(actor: &Actor, state: &HashSet<String>, ty: &str) -> String {
    let lookup = |name: &str| actor.actions.iter().find(|a| a.name == name);
    let priorities = Priorities::new(actor);

    let mut out = String::new();
    for port in &actor.inports {
        let _ = writeln!(
            out,
            "        let mut {} = {}.len();",
            avail_counter("", &port.name),
            port_ref(&port.name)
        );
    }
    for port in &actor.outports {
        let _ = writeln!(
            out,
            "        let mut {} = {}.room();",
            room_snapshot("", &port.name),
            port_ref(&port.name)
        );
    }
    out.push_str("        let mut __fired = 0usize;\n");
    out.push_str("        '__sched: while __fired < FIRE_BUDGET {\n");

    let body = match &actor.fsm {
        None => {
            let candidates: Vec<&Action> = actor.actions.iter().collect();
            priorities
                .order(actor, &candidates)
                .into_iter()
                .map(|i| emit_action(actor, candidates[i], state, None, Commit::Sched))
                .collect()
        }
        Some(fsm) => {
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
                        actor,
                        reachable[i],
                        state,
                        Some(&nexts[i]),
                        Commit::Sched,
                    ));
                }
                let _ = write!(
                    arms,
                    "            {ty}State::{} => {{\n{tries}            }}\n",
                    fsm_variant(s)
                );
            }
            format!("        match self.state {{\n{arms}        }}\n")
        }
    };

    out.push_str(&body);
    out.push_str("            break;\n");
    out.push_str("        }\n");
    out.push_str("        __fired\n");
    out
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
    Fallthrough,
    Move(&'a str),
    Sched,
}

fn count_binding(port: &str) -> String {
    format!("__take_{}", port_field(port))
}

fn sched_tail(actor: &Actor, rates: &Rates) -> String {
    let mut out = counter_updates(actor, rates, "");
    out.push_str("            __fired += 1;\n            continue '__sched;\n");
    out
}

fn counter_updates(actor: &Actor, rates: &Rates, prefix: &str) -> String {
    let mut out = String::new();
    if rates.refresh_all {
        for port in &actor.inports {
            let _ = writeln!(
                out,
                "            {} = {}.len();",
                avail_counter(prefix, &port.name),
                port_ref(&port.name)
            );
        }
        for port in &actor.outports {
            let _ = writeln!(
                out,
                "            {} = {}.room();",
                room_snapshot(prefix, &port.name),
                port_ref(&port.name)
            );
        }
    } else {
        for (port, _) in &rates.inputs {
            let _ = writeln!(
                out,
                "            {} -= {};",
                avail_counter(prefix, port),
                count_binding(port)
            );
        }
        for (port, count) in &rates.outputs {
            if count.is_some() {
                let _ = writeln!(
                    out,
                    "            {} -= {};",
                    room_snapshot(prefix, port),
                    count_binding(port)
                );
            } else {
                let _ = writeln!(
                    out,
                    "            {} = {}.room();",
                    room_snapshot(prefix, port),
                    port_ref(port)
                );
            }
        }
    }
    out
}

fn count_bindings(rates: &Rates) -> String {
    if rates.refresh_all {
        return String::new();
    }
    let mut out = String::new();
    for (port, count) in &rates.inputs {
        let _ = writeln!(out, "            let {} = {count};", count_binding(port));
    }
    for (port, count) in &rates.outputs {
        if let Some(count) = count {
            let _ = writeln!(out, "            let {} = {count};", count_binding(port));
        }
    }
    out
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
    actor: &Actor,
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

    let consume = !action.guards.iter().any(|g| reads_ports_or(g, &tokens));

    let body = emit_action_body(action, state, fsm_next, !consume);
    let rates = (commit != Commit::Fallthrough).then(|| action_rates(action, state, &locals));
    let prefix = if matches!(commit, Commit::Move(_)) {
        format!("{COUNTERS}.")
    } else {
        String::new()
    };
    let tail = match (commit, &rates) {
        (Commit::Move(next), Some(rates)) => format!(
            "{}            return ({next}(self.into_state()), true);\n",
            counter_updates(actor, rates, &prefix)
        ),
        (Commit::Sched, Some(rates)) => sched_tail(actor, rates),
        _ => String::new(),
    };
    let bindings = rates.as_ref().map(count_bindings).unwrap_or_default();

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

    let mut conds: Vec<String> = Vec::new();
    if let Some(rates) = &rates {
        for (port, count) in &rates.inputs {
            conds.push(format!("{} >= {count}", avail_counter(&prefix, port)));
        }
        for (port, count) in &rates.outputs {
            match count {
                Some(count) => conds.push(format!("{} >= {count}", room_snapshot(&prefix, port))),
                None => conds.push(format!("{} >= 1", room_snapshot(&prefix, port))),
            }
        }
    } else {
        conds.extend(action.input_patterns.iter().map(|p| {
            format!(
                "{}.avail({})",
                port_ref(&p.port),
                pattern_token_count(p, state, &locals)
            )
        }));
        let mut produced = BTreeSet::new();
        for output in &action.output_expressions {
            if produced.insert(output.port.clone()) {
                conds.push(room_snapshot("", &output.port));
            }
        }
    }
    if consume && let Some(guard) = &guard {
        conds.push(guard.clone());
    }

    if conds.is_empty() {
        if action.vars.is_empty() && bindings.is_empty() {
            return format!("{body}{tail}");
        }
        return format!("        {{\n{bindings}{body}{tail}        }}\n");
    }
    let avail = conds.join(" && ");

    if consume {
        let recvs = emit_recvs(action, state, &locals);
        return format!("        if {avail} {{\n{bindings}{recvs}{body}{tail}        }}\n");
    }

    let peeks = emit_peeks(action, state, &locals);
    let guarded = match &guard {
        Some(guard) => format!("if {guard} {{\n{body}{tail}            }}"),
        None => format!("{{\n{body}{tail}            }}"),
    };
    format!("        if {avail} {{\n{bindings}{peeks}            {guarded}\n        }}\n")
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
