use std::collections::HashSet;
use std::fmt::Write as _;

use crate::ast::Actor;
use crate::codegen::Program;
use crate::network_ffi::ffi::Instance;

use super::{
    actor_mod, actor_type, chan_rx, chan_tx, chan_var, default_value, emit_const, emit_expr,
    emit_function, emit_natives, emit_procedure, inst_var, out_port_ctor, param_value, rust_type,
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Channels {
    Local,
    Crossbeam,
}

pub fn emit_shared_decls(program: &Program<'_>, orcc: bool) -> String {
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
        out.push_str(&emit_natives(program, orcc));
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

pub fn emit_main_prelude<'a>(
    program: &Program<'a>,
    orcc: bool,
    channels: Channels,
    typestate: bool,
) -> (Vec<&'a Instance>, String) {
    let network = program.network;
    let instances: Vec<&Instance> = network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect();

    let mut out = String::from("fn main() {\n");

    if program.has_natives() && orcc {
        out.push_str(crate::codegen::orcc::MAIN_SETUP);
    }

    out.push_str(&emit_channels(program, &instances, channels));

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut args = vec![instance_args(inst, actor)];
        args.retain(|a| !a.is_empty());
        args.push(port_args(program, &instances, inst, actor, channels));
        let _ = writeln!(
            out,
            "    let mut {} = {}::{}::new({});",
            inst_var(&inst.id),
            actor_mod(&actor.name),
            actor_type(actor, typestate),
            args.join(", ")
        );
    }

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        if actor.init.is_some() {
            let _ = writeln!(out, "    {}.init();", inst_var(&inst.id));
        }
    }

    (instances, out)
}

fn emit_channels(program: &Program<'_>, instances: &[&Instance], channels: Channels) -> String {
    let mut out = String::new();
    for inst in instances {
        let actor = &program.actors[&inst.class_name];
        for port in &actor.inports {
            let ty = rust_type(&port.typ);
            match channels {
                Channels::Local => {
                    let _ = writeln!(
                        out,
                        "    let {} = Rc::new(RefCell::new(VecDeque::<{ty}>::new()));",
                        chan_var(&inst.id, &port.name)
                    );
                }
                Channels::Crossbeam => {
                    let _ = writeln!(
                        out,
                        "    let ({}, {}) = if CAP == 0 {{ crossbeam_channel::unbounded::<{ty}>() }} else {{ crossbeam_channel::bounded::<{ty}>(CAP) }};",
                        chan_tx(&inst.id, &port.name),
                        chan_rx(&inst.id, &port.name)
                    );
                }
            }
        }
    }
    out
}

fn port_args(
    program: &Program<'_>,
    instances: &[&Instance],
    inst: &Instance,
    actor: &Actor,
    channels: Channels,
) -> String {
    let known: HashSet<(&str, &str)> = instances
        .iter()
        .flat_map(|i| {
            program.actors[&i.class_name]
                .inports
                .iter()
                .map(move |port| (i.id.as_str(), port.name.as_str()))
        })
        .collect();

    let mut args = Vec::new();
    for port in &actor.inports {
        let source = match channels {
            Channels::Local => format!("{}.clone()", chan_var(&inst.id, &port.name)),
            Channels::Crossbeam => chan_rx(&inst.id, &port.name),
        };
        args.push(format!("InPort::new({source})"));
    }
    for port in &actor.outports {
        let targets: Vec<String> = program
            .network
            .edges
            .iter()
            .filter(|e| e.src_id == inst.id && e.src_port == port.name)
            .filter(|e| known.contains(&(e.dst_id.as_str(), e.dst_port.as_str())))
            .map(|e| match channels {
                Channels::Local => format!("{}.clone()", chan_var(&e.dst_id, &e.dst_port)),
                Channels::Crossbeam => format!("{}.clone()", chan_tx(&e.dst_id, &e.dst_port)),
            })
            .collect();
        args.push(out_port_ctor(&targets));
    }
    args.join(", ")
}
