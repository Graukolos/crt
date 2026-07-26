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

pub fn port_field(name: &str) -> String {
    format!("port_{}", ident(name))
}

pub fn port_ref(name: &str) -> String {
    format!("self.{}", port_field(name))
}

pub fn chan_var(id: &str, port: &str) -> String {
    format!("ch_{}_{}", ident(id), ident(port))
}

pub fn chan_tx(id: &str, port: &str) -> String {
    format!("tx_{}_{}", ident(id), ident(port))
}

pub fn chan_rx(id: &str, port: &str) -> String {
    format!("rx_{}_{}", ident(id), ident(port))
}

pub fn actor_mod(name: &str) -> String {
    format!("m_{}", ident(name))
}

pub fn out_port_ctor(targets: &[String]) -> String {
    match targets {
        [] => "OutPort::none()".to_string(),
        [target] => format!("OutPort::one({target})"),
        _ => format!("OutPort::many(vec![{}])", targets.join(", ")),
    }
}
