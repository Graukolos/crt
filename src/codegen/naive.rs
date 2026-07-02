use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::ast::{Action, Actor, InputPattern};
use crate::codegen::common::*;
use crate::codegen::{CodeGenerator, Program};
use crate::network_ffi::ffi::Instance;

pub struct Naive;

impl CodeGenerator for Naive {
    fn name(&self) -> &'static str {
        "naive"
    }

    fn generate(&self, program: &Program<'_>, out_dir: &Path) -> io::Result<()> {
        let source = emit_program(program);
        let tokens = source.parse().map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("generated source failed to tokenize: {err}\n--- source ---\n{source}"),
            )
        })?;
        super::write_rust(&out_dir.join("src").join("main.rs"), tokens)?;
        super::write_cargo_toml(out_dir, &program.network.name, program.has_natives())?;
        if program.has_natives() {
            super::write_native_support(out_dir, program.native_sources)?;
        }
        Ok(())
    }
}

fn emit_program(program: &Program<'_>) -> String {
    let mut out = String::new();
    out.push_str("#![allow(warnings)]\n");
    out.push_str("use std::collections::VecDeque;\n\n");
    out.push_str(&emit_decls(program));
    out.push_str(&emit_main(program));
    out
}

pub fn emit_decls(program: &Program<'_>) -> String {
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

    let class_names: Vec<&String> = program.actors.keys().collect();
    for class in class_names {
        let actor = &program.actors[class];
        out.push_str(&emit_actor(actor));
        out.push('\n');
    }

    out
}

fn emit_actor(actor: &Actor) -> String {
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
    let _ = write!(out, "struct {ty} {{\n{}\n}}\n\n", fields.join("\n"));

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
        "impl {ty} {{\n    fn new({params}) -> Self {{\n{lets}        Self {{\n{}\n        }}\n    }}\n\n",
        inits.join("\n")
    );

    if let Some(init) = &actor.init {
        let body = emit_action_body(init, &state, None);
        let _ = write!(
            out,
            "    fn init(&mut self{}) {{\n{body}\n    }}\n\n",
            port_params(actor)
        );
    }

    let _ = write!(
        out,
        "    fn fire(&mut self{}) -> bool {{\n{}\n        false\n    }}\n}}\n",
        port_params(actor),
        emit_fire(actor, &state, &ty)
    );

    out
}

fn actor_state(actor: &Actor) -> HashSet<String> {
    actor
        .parameters
        .iter()
        .map(|p| p.name.clone())
        .chain(actor.vars.iter().map(|v| v.name.clone()))
        .collect()
}

fn port_params(actor: &Actor) -> String {
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

fn emit_main(program: &Program<'_>) -> String {
    let network = program.network;
    let instances: Vec<&Instance> = network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect();

    let mut out = String::from("fn main() {\n");

    if program.has_natives() {
        out.push_str(super::orcc::MAIN_SETUP);
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        for port in &actor.inports {
            let _ = writeln!(
                out,
                "    let mut {}: VecDeque<{}> = VecDeque::new();",
                fifo_in(&inst.id, &port.name),
                rust_type(&port.typ)
            );
        }
        for port in &actor.outports {
            let _ = writeln!(
                out,
                "    let mut {}: VecDeque<{}> = VecDeque::new();",
                fifo_out(&inst.id, &port.name),
                rust_type(&port.typ)
            );
        }
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let args = actor
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
            .join(", ");
        let _ = writeln!(
            out,
            "    let mut {} = {}::new({args});",
            inst_var(&inst.id),
            type_ident(&actor.name)
        );
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        if actor.init.is_some() {
            let _ = writeln!(
                out,
                "    {}.init({});",
                inst_var(&inst.id),
                fire_args(inst, actor)
            );
        }
    }

    out.push_str("    loop {\n        let mut progress = false;\n");
    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let _ = write!(
            out,
            "        if {}.fire({}) {{\n{}            progress = true;\n        }}\n",
            inst_var(&inst.id),
            fire_args(inst, actor),
            distribute(program, inst, actor)
        );
    }
    out.push_str("        if !progress {\n            break;\n        }\n    }\n}\n");
    out
}

pub fn fire_args(inst: &Instance, actor: &Actor) -> String {
    let mut parts = Vec::new();
    for port in &actor.inports {
        parts.push(format!("&mut {}", fifo_in(&inst.id, &port.name)));
    }
    for port in &actor.outports {
        parts.push(format!("&mut {}", fifo_out(&inst.id, &port.name)));
    }
    parts.join(", ")
}

fn distribute(program: &Program<'_>, inst: &Instance, actor: &Actor) -> String {
    let mut out = String::new();
    for port in &actor.outports {
        let targets: Vec<String> = program
            .network
            .edges
            .iter()
            .filter(|e| e.src_id == inst.id && e.src_port == port.name)
            .map(|e| fifo_in(&e.dst_id, &e.dst_port))
            .collect();
        let staging = fifo_out(&inst.id, &port.name);
        if targets.is_empty() {
            let _ = writeln!(out, "            {staging}.clear();");
        } else {
            let _ = writeln!(
                out,
                "            while let Some(token) = {staging}.pop_front() {{"
            );
            for target in &targets {
                let _ = writeln!(out, "                {target}.push_back(token);");
            }
            out.push_str("            }\n");
        }
    }
    out
}
