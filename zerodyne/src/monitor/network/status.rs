//! Network status collection: the typed form of `network-status.sh --verbose`
//! plus daemon-side aggregation. Port of `network/status.go`.

use anyhow::Result;
use std::sync::Arc;

use super::model::{AddrIface, RouteInfo, RouteRow, Status};
use super::parse::{
    is_wireless, iw_bitrate, iw_float, iw_int, iw_ssid, read_int_sysfs, read_uint_sysfs, sysfs_net,
};
use crate::caps;
use crate::runner::{Ctx, Runner};

/// The address the default route is resolved against.
pub const INTERNET_PROBE: &str = "1.1.1.1";

/// Collector gathers network state through a Runner.
pub struct Collector {
    pub run: Arc<dyn Runner>,
    pub is_wireless: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    pub has: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl Collector {
    pub fn new(run: Arc<dyn Runner>) -> Self {
        Self {
            run,
            is_wireless: Arc::new(is_wireless),
            has: Arc::new(caps::available),
        }
    }

    /// The one network status snapshot.
    pub fn collect(&self, ctx: &Ctx) -> Result<Status> {
        let route = self.route_get(ctx);
        if route.dev.is_empty() {
            return Err(anyhow::anyhow!("network: no default route"));
        }
        let dev = route.dev.clone();
        let mut st = Status {
            iface: dev.clone(),
            ip: route.src,
            prefix: route.prefix,
            gateway: route.gw.clone(),
            r#type: String::new(),
            rx_bytes: read_uint_sysfs(&sysfs_net(&dev, "statistics/rx_bytes")),
            tx_bytes: read_uint_sysfs(&sysfs_net(&dev, "statistics/tx_bytes")),
            ssid: String::new(),
            signal_pct: None,
            signal_dbm: None,
            freq_mhz: 0,
            bitrate: String::new(),
            speed_mb: None,
            duplex: String::new(),
            router_ping_ms: None,
            internet_ping_ms: None,
            download_bps: 0.0,
            upload_bps: 0.0,
            router_ping_avg_ms: None,
            internet_ping_avg_ms: None,
            internet_loss_pct: 0,
            ping_samples: 0,
        };

        if (self.is_wireless)(&dev) {
            st.r#type = "wifi".to_string();
            let link = self.iw_link(ctx, &dev);
            st.ssid = iw_ssid(&link);
            st.signal_dbm = iw_int(&link, "signal:");
            if let Some(v) = iw_float(&link, "freq:") {
                st.freq_mhz = v as i32;
            }
            let (rate, unit) = iw_bitrate(&link);
            if !rate.is_empty() {
                st.bitrate = format!("{} {}", rate, unit).trim().to_string();
            }
            if st.ssid.is_empty() {
                let (_, conn) = self.nm_device(ctx, &dev);
                st.ssid = conn;
            }
            st.signal_pct = self.wifi_signal(ctx, &dev);
        } else {
            st.r#type = "ethernet".to_string();
            st.speed_mb = read_int_sysfs(&sysfs_net(&dev, "speed"));
            if let Ok(raw) = std::fs::read_to_string(sysfs_net(&dev, "duplex")) {
                let s = raw.trim();
                if !s.is_empty() {
                    st.duplex = s.to_string();
                }
            }
        }

        let (router, internet) = self.ping_samples(ctx, &route.gw);
        st.router_ping_ms = router;
        st.internet_ping_ms = internet;
        Ok(st)
    }

    /// Resolve the default route, falling back to text parsing when `ip -j`
    /// output is empty or unparseable.
    fn route_get(&self, ctx: &Ctx) -> RouteInfo {
        let mut ri = RouteInfo::default();
        if !(self.has)("ip") {
            return ri;
        }
        let out = match self
            .run
            .run(ctx, "ip", &["-j", "route", "get", INTERNET_PROBE])
        {
            Ok(out) => out,
            Err(_) => return self.route_get_text(ctx),
        };
        if out.trim().is_empty() {
            return self.route_get_text(ctx);
        }
        let rows: Vec<RouteRow> = match serde_json::from_str(&out) {
            Ok(rows) => rows,
            Err(_) => return self.route_get_text(ctx),
        };
        let Some(first) = rows.first() else {
            return self.route_get_text(ctx);
        };
        ri.dev = first.dev.clone();
        ri.gw = first.gateway.clone();
        ri.src = first.prefsrc.clone();
        if !ri.dev.is_empty() {
            ri.prefix = self.inet_prefix(ctx, &ri.dev);
        }
        ri
    }

    fn route_get_text(&self, ctx: &Ctx) -> RouteInfo {
        let mut ri = RouteInfo::default();
        let out = match self.run.run(ctx, "ip", &["route", "get", INTERNET_PROBE]) {
            Ok(out) => out,
            Err(_) => return ri,
        };
        let fields: Vec<&str> = out.split_whitespace().collect();
        for (i, f) in fields.iter().enumerate() {
            let value = fields.get(i + 1).copied().unwrap_or("");
            match *f {
                "dev" => ri.dev = value.to_string(),
                "via" => ri.gw = value.to_string(),
                "src" => ri.src = value.to_string(),
                _ => {}
            }
        }
        if !ri.dev.is_empty() {
            ri.prefix = self.inet_prefix(ctx, &ri.dev);
        }
        ri
    }

    fn inet_prefix(&self, ctx: &Ctx, dev: &str) -> i32 {
        let out = match self.run.run(ctx, "ip", &["-j", "addr", "show", dev]) {
            Ok(out) => out,
            Err(_) => return 0,
        };
        let ifaces: Vec<AddrIface> = match serde_json::from_str(&out) {
            Ok(v) => v,
            Err(_) => return 0,
        };
        for iface in ifaces {
            for a in iface.addr_info {
                if a.family == "inet" {
                    return a.prefixlen;
                }
            }
        }
        0
    }

    /// (GENERAL.STATE, GENERAL.CONNECTION) for a device.
    fn nm_device(&self, ctx: &Ctx, dev: &str) -> (String, String) {
        if !(self.has)("nmcli") {
            return (String::new(), String::new());
        }
        let out = match self.run.run(
            ctx,
            "nmcli",
            &[
                "-t",
                "-f",
                "GENERAL.STATE,GENERAL.CONNECTION",
                "dev",
                "show",
                dev,
            ],
        ) {
            Ok(out) => out,
            Err(_) => return (String::new(), String::new()),
        };
        let (mut state, mut conn) = (String::new(), String::new());
        for line in out.split('\n') {
            if let Some(v) = line.strip_prefix("GENERAL.STATE:") {
                state = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("GENERAL.CONNECTION:") {
                conn = v.trim().to_string();
            }
        }
        if conn == "--" {
            conn.clear();
        }
        (state, conn)
    }

    /// The in-use AP's SIGNAL (0-100), like the script's awk.
    fn wifi_signal(&self, ctx: &Ctx, dev: &str) -> Option<i32> {
        if !(self.has)("nmcli") {
            return None;
        }
        let out = self
            .run
            .run(
                ctx,
                "nmcli",
                &[
                    "-t",
                    "-f",
                    "IN-USE,SIGNAL",
                    "dev",
                    "wifi",
                    "list",
                    "ifname",
                    dev,
                    "--rescan",
                    "no",
                ],
            )
            .ok()?;
        for line in out.split('\n') {
            let Some((in_use, sig)) = line.split_once(':') else {
                continue;
            };
            if in_use.trim() != "*" {
                continue;
            }
            if let Ok(v) = sig.trim().parse::<i32>() {
                return Some(v);
            }
        }
        None
    }

    fn iw_link(&self, ctx: &Ctx, dev: &str) -> String {
        if !(self.has)("iw") {
            return String::new();
        }
        self.run
            .run(ctx, "iw", &["dev", dev, "link"])
            .unwrap_or_default()
    }

    /// Ping the gateway (if any) and the internet probe in parallel, like
    /// Go's two goroutines. Two unreachable hosts cost ~1s, not ~2s.
    fn ping_samples(&self, ctx: &Ctx, gateway: &str) -> (Option<f64>, Option<f64>) {
        if !(self.has)("ping") {
            return (None, None);
        }
        let run = self.run.as_ref();
        std::thread::scope(|scope| {
            let router =
                (!gateway.is_empty()).then(|| scope.spawn(|| ping_once(ctx, run, gateway)));
            let internet = scope.spawn(|| ping_once(ctx, run, INTERNET_PROBE));
            (
                router.and_then(|h| h.join().ok()).flatten(),
                internet.join().ok().flatten(),
            )
        })
    }
}

/// `ping -n -c1 -W1 host`, extracting the `time=` token.
fn ping_once(ctx: &Ctx, run: &dyn Runner, host: &str) -> Option<f64> {
    let out = run
        .run(ctx, "ping", &["-n", "-c", "1", "-W", "1", host])
        .ok()?;
    if out.is_empty() {
        return None;
    }
    for token in out.split_whitespace() {
        if let Some(raw) = token.strip_prefix("time=")
            && let Ok(v) = raw.parse::<f64>()
        {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn pings_run_in_parallel() {
        let runner = Arc::new(FnRunner(|_ctx, name, args| {
            if name == "ping" {
                thread::sleep(Duration::from_millis(150));
                return Ok(format!(
                    "64 bytes from {}: time=5.0 ms",
                    args.last().unwrap_or(&"")
                ));
            }
            Ok(String::new())
        }));
        let mut collector = Collector::new(runner);
        collector.has = Arc::new(|_| true);
        let started = Instant::now();
        let (router, internet) = collector.ping_samples(&Ctx::new(), "192.168.1.1");
        assert_eq!(router, Some(5.0));
        assert_eq!(internet, Some(5.0));
        assert!(
            started.elapsed() < Duration::from_millis(280),
            "pings took {:?} (expected ~150ms, not ~300ms)",
            started.elapsed()
        );
    }
}
