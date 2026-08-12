use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;

use crate::ast::Actor;
use crate::codegen::{Options, Program};
use crate::network_ffi::ffi::Instance;

use super::{
    actor_mod, actor_type, chan_var, default_value, emit_const, emit_const_expr, emit_function,
    emit_natives, emit_procedure, inst_var, out_port_ctor, param_value, rust_type,
};

pub fn channel_types(program: &Program<'_>) -> BTreeSet<String> {
    program
        .network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .flat_map(|i| program.actors[&i.class_name].inports.iter())
        .map(|port| rust_type(&port.typ))
        .collect()
}

pub fn live_instances<'a>(program: &Program<'a>) -> Vec<&'a Instance> {
    program
        .network
        .instances
        .iter()
        .filter(|i| program.actors.contains_key(&i.class_name))
        .collect()
}

pub fn live_inports<'a>(
    program: &Program<'a>,
    instances: &[&'a Instance],
) -> HashSet<(&'a str, &'a str)> {
    instances
        .iter()
        .flat_map(|i| {
            program.actors[&i.class_name]
                .inports
                .iter()
                .map(move |port| (i.id.as_str(), port.name.as_str()))
        })
        .collect()
}

pub fn check_no_fanout(program: &Program<'_>) -> std::io::Result<()> {
    let instances = live_instances(program);
    let known = live_inports(program, &instances);

    let mut offenders = Vec::new();
    for inst in &instances {
        for port in &program.actors[&inst.class_name].outports {
            let targets: Vec<String> = program
                .network
                .edges
                .iter()
                .filter(|e| e.src_id == inst.id && e.src_port == port.name)
                .filter(|e| known.contains(&(e.dst_id.as_str(), e.dst_port.as_str())))
                .map(|e| format!("{}.{}", e.dst_id, e.dst_port))
                .collect();
            if targets.len() > 1 {
                offenders.push(format!(
                    "{}.{} -> {}",
                    inst.id,
                    port.name,
                    targets.join(", ")
                ));
            }
        }
    }
    if offenders.is_empty() {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "output port(s) feeding more than one destination, which the ports \
             cannot represent: {}",
            offenders.join("; ")
        ),
    ))
}

pub fn check_single_producer(program: &Program<'_>) -> std::io::Result<()> {
    let mut producers: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
    for edge in &program.network.edges {
        producers
            .entry((edge.dst_id.as_str(), edge.dst_port.as_str()))
            .or_default()
            .push(edge.src_id.as_str());
    }
    let offenders: Vec<String> = producers
        .iter()
        .filter(|(_, srcs)| srcs.len() > 1)
        .map(|((id, port), srcs)| format!("{id}.{port} <- {}", srcs.join(", ")))
        .collect();
    if offenders.is_empty() {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "input port(s) fed by more than one connection, which the lock-free \
             channels cannot represent: {}",
            offenders.join("; ")
        ),
    ))
}

pub fn emit_shared_decls(program: &Program<'_>, options: Options) -> String {
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
        out.push_str(&emit_natives(program, options.orcc));
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
                    Some(expr) => emit_const_expr(expr),
                    None => default_value(&p.typ),
                },
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn emit_main_prelude<'a>(
    program: &Program<'a>,
    options: Options,
    handle: &str,
) -> (Vec<&'a Instance>, String) {
    let instances = live_instances(program);
    let known = live_inports(program, &instances);

    let mut out = String::from("fn main() {\n");

    if program.has_natives() && options.orcc {
        out.push_str(crate::codegen::orcc::MAIN_SETUP);
    }

    out.push_str(&emit_channels(program, &instances, handle));

    for inst in &instances {
        let actor = &program.actors[&inst.class_name];
        let mut args = vec![instance_args(inst, actor)];
        args.retain(|a| !a.is_empty());
        args.push(port_args(program, &known, inst, actor));
        let _ = writeln!(
            out,
            "    let mut {} = {}::{}::new({});",
            inst_var(&inst.id),
            actor_mod(&actor.name),
            actor_type(actor, options.typestate),
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

fn emit_channels(program: &Program<'_>, instances: &[&Instance], handle: &str) -> String {
    let mut out = String::new();
    for inst in instances {
        let actor = &program.actors[&inst.class_name];
        for port in &actor.inports {
            let ty = rust_type(&port.typ);
            let _ = writeln!(
                out,
                "    let {} = {handle}::new(Chan::<{ty}>::new());",
                chan_var(&inst.id, &port.name),
            );
        }
    }
    out
}

fn port_args(
    program: &Program<'_>,
    known: &HashSet<(&str, &str)>,
    inst: &Instance,
    actor: &Actor,
) -> String {
    let mut args = Vec::new();
    for port in &actor.inports {
        args.push(format!(
            "InPort::new({}.clone())",
            chan_var(&inst.id, &port.name)
        ));
    }
    for port in &actor.outports {
        let targets: Vec<String> = program
            .network
            .edges
            .iter()
            .filter(|e| e.src_id == inst.id && e.src_port == port.name)
            .filter(|e| known.contains(&(e.dst_id.as_str(), e.dst_port.as_str())))
            .map(|e| format!("{}.clone()", chan_var(&e.dst_id, &e.dst_port)))
            .collect();
        args.push(out_port_ctor(&targets));
    }
    args.join(", ")
}
