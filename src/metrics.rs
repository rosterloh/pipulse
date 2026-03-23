use std::time::Instant;
use sysinfo::{Components, Disks, Networks, System};

pub struct NetSampler {
    iface: String,
    prev_rx: u64,
    prev_tx: u64,
    last_sample: Instant,
}

impl NetSampler {
    pub fn new(iface: &str) -> Self {
        let (rx, tx) = Self::iface_bytes(iface);
        Self {
            iface: iface.to_string(),
            prev_rx: rx,
            prev_tx: tx,
            last_sample: Instant::now(),
        }
    }

    fn iface_bytes(iface: &str) -> (u64, u64) {
        let networks = Networks::new_with_refreshed_list();
        for (name, data) in &networks {
            if name == iface {
                return (data.total_received(), data.total_transmitted());
            }
        }
        (0, 0)
    }

    /// Returns `(rx_bytes_per_sec, tx_bytes_per_sec)` since last call.
    pub fn sample(&mut self) -> (u64, u64) {
        let elapsed = self.last_sample.elapsed().as_secs_f64().max(0.001);
        let (rx, tx) = Self::iface_bytes(&self.iface);
        let rx_bps = (rx.saturating_sub(self.prev_rx) as f64 / elapsed) as u64;
        let tx_bps = (tx.saturating_sub(self.prev_tx) as f64 / elapsed) as u64;
        self.prev_rx = rx;
        self.prev_tx = tx;
        self.last_sample = Instant::now();
        (rx_bps, tx_bps)
    }
}

pub struct CpuCollector {
    sys: System,
}

impl CpuCollector {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        Self { sys }
    }

    pub fn sample(&mut self) -> u8 {
        self.sys.refresh_cpu_usage();
        self.sys.global_cpu_usage().min(100.0) as u8
    }
}

/// Returns `(ip_address, interface_name)` for the first non-virtual IPv4 interface.
pub fn get_net_info() -> (String, String) {
    let networks = Networks::new_with_refreshed_list();
    for (iface_name, data) in &networks {
        if iface_name.starts_with("lo")
            || iface_name.starts_with("docker")
            || iface_name.starts_with("veth")
        {
            continue;
        }
        for ip_net in data.ip_networks() {
            if ip_net.addr.is_ipv4() && !ip_net.addr.is_loopback() {
                return (ip_net.addr.to_string(), iface_name.clone());
            }
        }
    }
    ("no ip".into(), String::new())
}

pub fn get_loadavg() -> String {
    let la = System::load_average();
    format!("{:.2} {:.2} {:.2}", la.one, la.five, la.fifteen)
}

pub fn get_memory_percent() -> u8 {
    let mut sys = System::new();
    sys.refresh_memory();
    let total = sys.total_memory();
    if total == 0 {
        return 0;
    }
    let used = sys.used_memory();
    ((used as f64 / total as f64) * 100.0).min(100.0) as u8
}

pub fn get_cpu_temp() -> Option<f32> {
    let components = Components::new_with_refreshed_list();
    if let Some(temp) = components.iter().find_map(|c| c.temperature()) {
        return Some(temp);
    }
    // Fallback for Raspberry Pi thermal zone
    std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .map(|m| m as f32 / 1000.0)
}

pub fn get_disk_percent() -> u8 {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .map(|d| {
            let total = d.total_space();
            if total == 0 {
                return 0;
            }
            let used = total.saturating_sub(d.available_space());
            ((used as f64 / total as f64) * 100.0).min(100.0) as u8
        })
        .unwrap_or(0)
}

pub fn get_uptime_str() -> String {
    let secs = System::uptime();
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
    System::host_name().unwrap_or_else(|| "?".into())
}
