//! Host-level facts that fit neither network nor performance (hostname,
//! uptime). Port of `internal/monitor/system/monitor.go`.

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::Value;

use crate::monitor::Monitor as MonitorTrait;
use crate::runner::Ctx;

/// The typed system state.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub hostname: String,
    pub uptime_secs: i64,
}

/// The registered "system" monitor.
pub struct Monitor;

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self
    }
}

impl MonitorTrait for Monitor {
    fn name(&self) -> &'static str {
        "system"
    }

    fn snapshot(&self, _ctx: &Ctx) -> Result<Value> {
        let host = hostname();
        let up = read_uptime()?;
        Ok(serde_json::to_value(Snapshot {
            hostname: host,
            uptime_secs: up,
        })?)
    }
}

/// `os.Hostname()` in Go: the kernel's nodename, empty on failure.
fn hostname() -> String {
    let mut buf = [0 as libc::c_char; 256];
    if unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len()) } == 0 {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let mut bytes = Vec::with_capacity(end);
        for c in buf[..end].iter() {
            bytes.push(*c as u8);
        }
        return String::from_utf8_lossy(&bytes).into_owned();
    }
    // Fallback for containers where the syscall is blocked.
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn read_uptime() -> Result<i64> {
    let raw = std::fs::read_to_string("/proc/uptime")
        .map_err(|e| anyhow!("system: /proc/uptime: {}", e))?;
    let first = raw.split_whitespace().next().unwrap_or("");
    let f: f64 = first
        .parse()
        .map_err(|_| anyhow!("system: parse /proc/uptime: {}", first))?;
    Ok(f as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_system() {
        assert_eq!(Monitor::new().name(), "system");
    }

    #[test]
    fn snapshot_reports_host_and_uptime() {
        let v = Monitor::new().snapshot(&Ctx::new()).unwrap();
        assert!(v["hostname"].is_string());
        assert!(v["uptime_secs"].is_i64());
        assert!(v["uptime_secs"].as_i64().unwrap() >= 0);
    }

    #[test]
    fn uptime_is_nonnegative() {
        assert!(read_uptime().unwrap() >= 0);
    }
}
