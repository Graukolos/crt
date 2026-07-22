use std::collections::HashSet;
use std::fmt::Write as _;

use crate::ast::Actor;
use crate::codegen::Program;
use crate::network_ffi::ffi::Instance;

use super::{
    actor_mod, default_value, emit_const, emit_expr, emit_function, emit_natives, emit_procedure,
    fifo_in, fifo_out, inst_var, param_value, rust_type, type_ident,
};

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

pub fn emit_main_prelude<'a>(program: &Program<'a>) -> (Vec<&'a Instance>, String) {
    let network = program.network;
    let instances: Vec<&Instance> = network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect();

    let mut out = String::from("fn main() {\n");

    if program.has_natives() {
        out.push_str(crate::codegen::orcc::MAIN_SETUP);
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
        let args = instance_args(inst, actor);
        let _ = writeln!(
            out,
            "    let mut {} = {}::{}::new({args});",
            inst_var(&inst.id),
            actor_mod(&actor.name),
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

    (instances, out)
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

pub fn distribute(program: &Program<'_>, inst: &Instance, actor: &Actor) -> String {
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
