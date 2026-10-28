use std::time::Instant;
use sysinfo::{Components, Disks, Networks, System};

/// One sample of every metric shown on the display.
pub struct Snapshot {
    pub ip: String,
    pub iface: String,
    pub hostname: String,
    pub cpu_pct: u8,
    pub mem_pct: u8,
    pub disk_pct: u8,
    pub temp: Option<f32>,
    pub uptime: String,
    pub load: String,
    /// Receive rate in bytes/sec since the previous sample.
    pub rx_bps: u64,
    /// Transmit rate in bytes/sec since the previous sample.
    pub tx_bps: u64,
}

/// Long-lived sysinfo handles, refreshed in place on every sample rather than
/// rebuilt from scratch.
pub struct Metrics {
    sys: System,
    networks: Networks,
    disks: Disks,
    components: Components,
    net_iface: String,
    prev_rx: u64,
    prev_tx: u64,
    last_net: Instant,
}

impl Metrics {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        let networks = Networks::new_with_refreshed_list();
        let (net_iface, prev_rx, prev_tx) = match primary_iface(&networks) {
            Some((name, _)) => {
                let (rx, tx) = iface_totals(&networks, name);
                (name.to_string(), rx, tx)
            }
            None => (String::new(), 0, 0),
        };
        Self {
            sys,
            networks,
            disks: Disks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            net_iface,
            prev_rx,
            prev_tx,
            last_net: Instant::now(),
        }
    }

    pub fn sample(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        // `true` also picks up interfaces that appeared since the last refresh
        self.networks.refresh(true);
        self.components.refresh(false);
        if let Some(root) = self.disks.list_mut().iter_mut().find(|d| is_root(d)) {
            root.refresh();
        }

        let (ip, iface) = match primary_iface(&self.networks) {
            Some((name, ip)) => (ip, name.to_string()),
            None => ("no ip".into(), String::new()),
        };
        let (rx_bps, tx_bps) = self.net_rates(&iface);

        Snapshot {
            ip,
            iface,
            hostname: System::host_name().unwrap_or_else(|| "?".into()),
            cpu_pct: self.sys.global_cpu_usage().min(100.0) as u8,
            mem_pct: percent(self.sys.used_memory(), self.sys.total_memory()),
            disk_pct: self.disks.iter().find(|d| is_root(d)).map_or(0, |d| {
                let total = d.total_space();
                percent(total.saturating_sub(d.available_space()), total)
            }),
            temp: self.cpu_temp(),
            uptime: uptime_str(System::uptime()),
            load: {
                let la = System::load_average();
                format!("{:.2} {:.2} {:.2}", la.one, la.five, la.fifteen)
            },
            rx_bps,
            tx_bps,
        }
    }

    /// Returns `(rx_bytes_per_sec, tx_bytes_per_sec)` for `iface` since the
    /// last call. If the primary interface changed, the baseline is reset and
    /// this sample reports zero.
    fn net_rates(&mut self, iface: &str) -> (u64, u64) {
        let elapsed = self.last_net.elapsed().as_secs_f64().max(0.001);
        let (rx, tx) = iface_totals(&self.networks, iface);
        let rates = if iface == self.net_iface {
            (
                (rx.saturating_sub(self.prev_rx) as f64 / elapsed) as u64,
                (tx.saturating_sub(self.prev_tx) as f64 / elapsed) as u64,
            )
        } else {
            self.net_iface = iface.to_string();
            (0, 0)
        };
        self.prev_rx = rx;
        self.prev_tx = tx;
        self.last_net = Instant::now();
        rates
    }

    fn cpu_temp(&self) -> Option<f32> {
        if let Some(temp) = self.components.iter().find_map(|c| c.temperature()) {
            return Some(temp);
        }
        // Fallback for Raspberry Pi thermal zone
        std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .map(|m| m as f32 / 1000.0)
    }
}

/// Returns `(interface_name, ipv4_address)` for the first non-virtual interface
/// with an IPv4 address. Interfaces are checked in name order so the choice is
/// stable between samples (e.g. `eth0` is preferred over `wlan0`).
fn primary_iface(networks: &Networks) -> Option<(&str, String)> {
    let mut names: Vec<&String> = networks
        .keys()
        .filter(|n| !(n.starts_with("lo") || n.starts_with("docker") || n.starts_with("veth")))
        .collect();
    names.sort();
    names.into_iter().find_map(|name| {
        networks[name]
            .ip_networks()
            .iter()
            .find(|ip| ip.addr.is_ipv4() && !ip.addr.is_loopback())
            .map(|ip| (name.as_str(), ip.addr.to_string()))
    })
}

fn iface_totals(networks: &Networks, iface: &str) -> (u64, u64) {
    networks
        .get(iface)
        .map_or((0, 0), |d| (d.total_received(), d.total_transmitted()))
}

fn is_root(disk: &sysinfo::Disk) -> bool {
    disk.mount_point() == std::path::Path::new("/")
}

fn percent(used: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((used as f64 / total as f64) * 100.0).min(100.0) as u8
}

fn uptime_str(secs: u64) -> String {
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
