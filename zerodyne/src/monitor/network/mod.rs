//! Network monitor: status, band helpers, wifi scanning, and trackers.
//! Port of `internal/monitor/network/*`. The child modules keep each file
//! focused; all public API is re-exported here so callers keep using
//! `crate::monitor::network::...` unchanged.

mod band;
mod model;
mod monitor;
mod parse;
mod scan;
mod scanmon;
mod status;
mod trackers;

pub use band::{
    BAND_5, BAND_6, BAND_24, BAND_AUTO, band_for_freq, band_from_nm, nm_band_for,
    normalize_band_target, valid_band,
};
pub use model::Status;
pub use monitor::Monitor;
pub use scan::{ScanNetwork, scan_device, scan_visible};
pub use scanmon::{SCAN_INTERVAL, ScanMonitor};
pub use status::{Collector, INTERNET_PROBE};
pub use trackers::{
    PING_AVERAGE_WINDOW, PING_HISTORY_WINDOW, PingStats, PingTracker, ThroughputTracker,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::Monitor as _;
    use crate::runner::{Ctx, FnRunner};
    use std::sync::Arc;

    /// Answers canned outputs for the collector's command probes.
    fn stub_runner() -> FnRunner<impl Fn(&Ctx, &str, &[&str]) -> Result<String, String>> {
        FnRunner(|_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("ip -j route get") {
                Ok(r#"[{"dev":"wlan0","gateway":"192.168.1.1","prefsrc":"192.168.1.5"}]"#.into())
            } else if key.starts_with("ip -j addr show") {
                Ok(r#"[{"addr_info":[{"family":"inet","prefixlen":24}]}]"#.into())
            } else if key.starts_with("iw dev wlan0 link") {
                Ok("Connected to aa:bb:cc:dd:ee:ff\nSSID: Home Net\nfreq: 5745\nsignal: -55 dBm\ntx bitrate: 866.7 MBit/s".into())
            } else if key.starts_with("nmcli -t -f IN-USE,SIGNAL") {
                Ok("*:72\n :55".into())
            } else if key.starts_with("nmcli -t -f GENERAL.STATE") {
                Ok("GENERAL.STATE:100 (connected)\nGENERAL.CONNECTION:Home Net".into())
            } else if key.starts_with("ping") {
                Ok("64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=12.3 ms".into())
            } else {
                Err("no stub".into())
            }
        })
    }

    fn stub_monitor<'a>(
        outputs: &'a [(&'a str, &'a str)],
    ) -> FnRunner<impl Fn(&Ctx, &str, &[&str]) -> Result<String, String> + 'a> {
        FnRunner(move |_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            for (prefix, out) in outputs {
                if key.starts_with(prefix) {
                    return Ok(out.to_string());
                }
            }
            Ok(String::new())
        })
    }

    #[test]
    fn collector_verbose_wifi() {
        let mut c = Collector::new(Arc::new(stub_runner()));
        c.is_wireless = Arc::new(|dev| dev == "wlan0");
        c.has = Arc::new(|_| true);
        let st = c.collect(&Ctx::new()).unwrap();
        assert_eq!(st.iface, "wlan0");
        assert_eq!(st.r#type, "wifi");
        assert_eq!(st.ssid, "Home Net");
        assert_eq!(st.freq_mhz, 5745);
        assert_eq!(st.signal_dbm, Some(-55));
        assert_eq!(st.signal_pct, Some(72));
        assert_eq!(st.bitrate, "866.7 MBit/s");
        assert_eq!(st.prefix, 24);
        assert_eq!(st.gateway, "192.168.1.1");
        assert_eq!(st.ip, "192.168.1.5");
        assert_eq!(st.internet_ping_ms, Some(12.3));
    }

    #[test]
    fn collector_no_route() {
        let mut c = Collector::new(Arc::new(FnRunner(|_: &Ctx, _: &str, _: &[&str]| {
            Err("no stub".into())
        })));
        c.has = Arc::new(|_| true);
        assert!(c.collect(&Ctx::new()).is_err());
    }

    #[test]
    fn collector_missing_tools_degrades() {
        let mut c = Collector::new(Arc::new(FnRunner(|_: &Ctx, _: &str, _: &[&str]| {
            panic!("no command should run when tools are missing")
        })));
        c.has = Arc::new(|_| false);
        assert!(c.collect(&Ctx::new()).is_err());
    }

    #[test]
    fn collector_ssid_with_colon() {
        let run = FnRunner(|_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("ip -j route") {
                Ok(r#"[{"dev":"wlan0","gateway":"","prefsrc":"192.168.1.5"}]"#.into())
            } else if key.starts_with("ip -j addr") {
                Ok(r#"[{"addr_info":[{"family":"inet","prefixlen":24}]}]"#.into())
            } else if key.contains("GENERAL.STATE") {
                Ok("GENERAL.STATE:100 (connected)\nGENERAL.CONNECTION:--".into())
            } else if key.contains("IN-USE") {
                Err("no stub".into())
            } else if key.starts_with("iw ") {
                Ok("SSID: odd:name\nfreq: 2412".into())
            } else {
                Err("no stub".into())
            }
        });
        let mut c = Collector::new(Arc::new(run));
        c.is_wireless = Arc::new(|dev| dev == "wlan0");
        c.has = Arc::new(|_| true);
        let st = c.collect(&Ctx::new()).unwrap();
        assert_eq!(st.ssid, "odd:name");
        assert_eq!(st.freq_mhz, 2412);
    }

    #[test]
    fn monitor_snapshot_aggregates() {
        let mut collector = Collector::new(Arc::new(stub_runner()));
        collector.is_wireless = Arc::new(|dev| dev == "wlan0");
        collector.has = Arc::new(|_| true);
        let m = Monitor::new_with(collector, ThroughputTracker::new(), PingTracker::new());
        let v = m.snapshot(&Ctx::new()).unwrap();
        assert_eq!(m.name(), "network");
        assert_eq!(v["download_bps"], 0.0);
        assert_eq!(v["upload_bps"], 0.0);
        assert_eq!(v["ping_samples"], 1);
        assert_eq!(v["internet_ping_avg_ms"], 12.3);
        assert_eq!(v["internet_loss_pct"], 0);
    }

    #[test]
    fn scan_visible_keeps_colon_ssids() {
        let outputs: &[(&str, &str)] = &[
            ("nmcli -e no -t -f NAME connection show", "My:Home\n"),
            (
                "nmcli -e no -t -f IN-USE",
                "*:80:WPA2:AA:BB:CC:DD:EE:FF:My:Home\n:70:WPA2:BB:CC:DD:EE:FF:00:My:Home\n",
            ),
        ];
        let run = stub_monitor(outputs);
        let nets = scan_visible(&Ctx::new(), &run, "wlan0", false).unwrap();
        assert_eq!(nets.len(), 1);
        assert_eq!(nets[0].ssid, "My:Home");
        assert_eq!(nets[0].bssid, "AA:BB:CC:DD:EE:FF");
        assert!(nets[0].in_use);
        assert!(nets[0].known);
    }

    #[test]
    fn scan_monitor_serves_cache() {
        let outputs: &[(&str, &str)] = &[
            (
                "nmcli -e no -t -f DEVICE,TYPE,STATE device status",
                "wlan0:wifi:connected\n",
            ),
            ("nmcli -e no -t -f NAME connection show", "Home\n"),
            (
                "nmcli -e no -t -f IN-USE",
                "*:80:WPA2:AA:BB:CC:DD:EE:FF:Home\n",
            ),
        ];
        let run = stub_monitor(outputs);
        let m = ScanMonitor::new_with_runner(Arc::new(run));
        m.refresh(&Ctx::new());
        let v = m.snapshot(&Ctx::new()).unwrap();
        assert_eq!(m.name(), "wifi");
        assert_eq!(v["device"], "wlan0");
        assert_eq!(v["networks"].as_array().unwrap().len(), 1);
        assert_eq!(v["networks"][0]["ssid"], "Home");
    }

    #[test]
    fn status_json_shape_matches_go() {
        let mut c = Collector::new(Arc::new(stub_runner()));
        c.is_wireless = Arc::new(|dev| dev == "wlan0");
        c.has = Arc::new(|_| true);
        let v = serde_json::to_value(c.collect(&Ctx::new()).unwrap()).unwrap();
        assert_eq!(v["iface"], "wlan0");
        assert_eq!(v["type"], "wifi");
        assert_eq!(v["rx_bytes"], 0);
        assert!(v.get("router_ping_ms").unwrap().is_number());
        assert!(v.get("internet_ping_ms").unwrap().is_number());
        assert!(v.get("download_bps").unwrap().is_number());
        assert_eq!(v["ping_samples"], 0); // no omitempty: always present
    }
}
