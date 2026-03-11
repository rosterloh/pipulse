use std::{fs, net::IpAddr};
use get_if_addrs::get_if_addrs;

pub fn get_ipv4() -> String {
    match get_if_addrs() {
        Ok(addrs) => addrs
            .into_iter()
            .filter(|i| !i.is_loopback())
            .filter_map(|i| match i.ip() {
                IpAddr::V4(v4) => Some((i.name, v4)),
                _ => None,
            })
            .find(|(name, _)| {
                !name.starts_with("docker")
                    && !name.starts_with("veth")
                    && !name.starts_with("lo")
            })
            .map(|(_, ip)| ip.to_string())
            .unwrap_or_else(|| "no ip".into()),
        Err(_) => "no ip".into(),
    }
}

pub fn get_loadavg() -> String {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| {
            let mut p = s.split_whitespace();
            let a = p.next()?.to_owned();
            let b = p.next()?.to_owned();
            let c = p.next()?.to_owned();
            Some(format!("{a} {b} {c}"))
        })
        .unwrap_or_else(|| "? ? ?".into())
}
