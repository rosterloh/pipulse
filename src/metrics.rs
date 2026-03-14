use std::{fs, net::IpAddr};
use get_if_addrs::get_if_addrs;

pub struct CpuCollector {
    prev_idle: u64,
    prev_total: u64,
}

impl CpuCollector {
    pub fn new() -> Self {
        Self { prev_idle: 0, prev_total: 0 }
    }

    pub fn sample(&mut self) -> u8 {
        let Ok(s) = fs::read_to_string("/proc/stat") else { return 0 };
        let line = s.lines().next().unwrap_or("");
        let nums: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|x| x.parse().ok())
            .collect();
        if nums.len() < 4 {
            return 0;
        }
        let idle: u64 = nums[3];
        let total: u64 = nums.iter().sum();
        let delta_total = total.saturating_sub(self.prev_total);
        let delta_idle = idle.saturating_sub(self.prev_idle);
        self.prev_idle = idle;
        self.prev_total = total;
        if delta_total == 0 {
            return 0;
        }
        ((delta_total - delta_idle) as f64 / delta_total as f64 * 100.0).min(100.0) as u8
    }
}

/// Returns `(ip_address, interface_name)` for the first non-virtual IPv4 interface.
pub fn get_net_info() -> (String, String) {
    match get_if_addrs() {
        Ok(addrs) => {
            let result = addrs
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
                });
            match result {
                Some((iface, ip)) => (ip.to_string(), iface),
                None => ("no ip".into(), String::new()),
            }
        }
        Err(_) => ("no ip".into(), String::new()),
    }
}

pub fn get_ipv4() -> String {
    get_net_info().0
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

pub fn get_memory_percent() -> u8 {
    let Ok(s) = fs::read_to_string("/proc/meminfo") else { return 0 };
    let mut total = 0u64;
    let mut available = 0u64;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = rest.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = rest.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
        }
    }
    if total == 0 {
        return 0;
    }
    let used = total.saturating_sub(available);
    ((used as f64 / total as f64) * 100.0).min(100.0) as u8
}

pub fn get_cpu_temp() -> Option<f32> {
    let s = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").ok()?;
    let millideg: i32 = s.trim().parse().ok()?;
    Some(millideg as f32 / 1000.0)
}

pub fn get_disk_percent() -> u8 {
    use libc::statvfs;
    use std::{ffi::CString, mem};
    let path = CString::new("/").unwrap();
    let mut stat: libc::statvfs = unsafe { mem::zeroed() };
    let ret = unsafe { statvfs(path.as_ptr(), &mut stat) };
    if ret != 0 {
        return 0;
    }
    let total = (stat.f_blocks as u64).saturating_mul(stat.f_frsize as u64);
    let avail = (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64);
    if total == 0 {
        return 0;
    }
    let used = total - avail;
    ((used as f64 / total as f64) * 100.0).min(100.0) as u8
}

pub fn get_uptime_str() -> String {
    let Ok(s) = fs::read_to_string("/proc/uptime") else { return "?".into() };
    let secs = s
        .split_whitespace()
        .next()
        .and_then(|x| x.parse::<f64>().ok())
        .map(|x| x as u64)
        .unwrap_or(0);
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

pub fn get_hostname() -> String {
    fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "?".into())
}
