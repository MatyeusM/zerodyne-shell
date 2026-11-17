//! CPU/memory/temperature snapshot read from /proc and sysfs.
//! Port of `internal/monitor/performance/monitor.go`.

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::monitor::Monitor as MonitorTrait;
use crate::runner::Ctx;

fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}
fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}
fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

/// The typed performance state.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub cpu_percent: f64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cpu_name: String,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub cpu_cores: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_temp_c: Option<f64>,
    pub mem_total_kb: u64,
    pub mem_avail_kb: u64,
    pub mem_percent: f64,
    #[serde(skip_serializing_if = "is_zero_u64")]
    pub swap_total_kb: u64,
    #[serde(skip_serializing_if = "is_zero_u64")]
    pub swap_free_kb: u64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub swap_percent: f64,
    pub sampled_at_unix: i64,
}

/// The registered "performance" monitor.
pub struct Monitor {
    mu: Mutex<Prev>,
}

#[derive(Default)]
struct Prev {
    idle: u64,
    total: u64,
    ready: bool,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            mu: Mutex::new(Prev::default()),
        }
    }
}

impl MonitorTrait for Monitor {
    fn name(&self) -> &'static str {
        "performance"
    }

    fn snapshot(&self, _ctx: &Ctx) -> Result<Value> {
        let (idle, total) = read_cpu()?;

        let cpu = {
            let mut prev = self.mu.lock().unwrap_or_else(|e| e.into_inner());
            let mut pct = 0.0;
            if prev.ready && total > prev.total {
                let idle_delta = idle.wrapping_sub(prev.idle) as f64;
                let total_delta = total.wrapping_sub(prev.total) as f64;
                if total_delta > 0.0 && idle_delta <= total_delta {
                    pct = (1.0 - idle_delta / total_delta) * 100.0;
                }
            }
            prev.idle = idle;
            prev.total = total;
            prev.ready = true;
            pct
        };

        let (total_kb, avail_kb, swap_tot_kb, swap_free_kb) = read_mem()?;
        let mem_pct = if total_kb > 0 {
            (total_kb - avail_kb) as f64 / total_kb as f64 * 100.0
        } else {
            0.0
        };
        let swap_pct = if swap_tot_kb > 0 {
            (swap_tot_kb - swap_free_kb) as f64 / swap_tot_kb as f64 * 100.0
        } else {
            0.0
        };
        let (name, cores) = read_cpu_info();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Ok(serde_json::to_value(Snapshot {
            cpu_percent: cpu,
            cpu_name: name,
            cpu_cores: cores,
            cpu_temp_c: read_cpu_temp(),
            mem_total_kb: total_kb,
            mem_avail_kb: avail_kb,
            mem_percent: mem_pct,
            swap_total_kb: swap_tot_kb,
            swap_free_kb,
            swap_percent: swap_pct,
            sampled_at_unix: now,
        })?)
    }
}

/// Parse the aggregate `cpu` line of /proc/stat; returns (idle, total).
pub fn read_cpu() -> Result<(u64, u64)> {
    let raw = std::fs::read_to_string("/proc/stat")
        .map_err(|e| anyhow!("performance: /proc/stat: {}", e))?;
    for line in raw.split('\n') {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 || fields[0] != "cpu" {
            continue;
        }
        let mut vals = Vec::with_capacity(fields.len() - 1);
        for s in &fields[1..] {
            let v: u64 = s
                .parse()
                .map_err(|_| anyhow!("performance: parse /proc/stat: {}", s))?;
            vals.push(v);
        }
        let total: u64 = vals.iter().sum();
        // idle + iowait (vals[3] + vals[4]).
        let idle = if vals.len() >= 5 {
            vals[3] + vals[4]
        } else {
            vals[3]
        };
        return Ok((idle, total));
    }
    Err(anyhow!("performance: no cpu line in /proc/stat"))
}

/// Parse MemTotal/MemAvailable/SwapTotal/SwapFree from /proc/meminfo (kB).
pub fn read_mem() -> Result<(u64, u64, u64, u64)> {
    let raw = std::fs::read_to_string("/proc/meminfo")
        .map_err(|e| anyhow!("performance: /proc/meminfo: {}", e))?;
    let (mut total, mut avail, mut swap_tot, mut swap_free) = (0u64, 0u64, 0u64, 0u64);
    for line in raw.split('\n') {
        let Some((name, val)) = line.split_once(':') else {
            continue;
        };
        let n: u64 = val
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        match name.trim() {
            "MemTotal" => total = n,
            "MemAvailable" => avail = n,
            "SwapTotal" => swap_tot = n,
            "SwapFree" => swap_free = n,
            _ => {}
        }
    }
    if total == 0 {
        return Err(anyhow!("performance: no MemTotal in /proc/meminfo"));
    }
    Ok((total, avail, swap_tot, swap_free))
}

/// First `model name` and the processor count from /proc/cpuinfo (best effort).
pub fn read_cpu_info() -> (String, i32) {
    let raw = match std::fs::read_to_string("/proc/cpuinfo") {
        Ok(v) => v,
        Err(_) => return (String::new(), 0),
    };
    let mut name = String::new();
    let mut cores = 0;
    for line in raw.split('\n') {
        if line.starts_with("processor") {
            cores += 1;
            continue;
        }
        if name.is_empty()
            && line.starts_with("model name")
            && let Some((_, v)) = line.split_once(':')
        {
            name = cpu_name_trim(v.trim());
        }
    }
    (name, cores)
}

/// Strip the redundant `<n>-Core Processor` suffix from a CPU model name.
/// Equivalent to the old `\s*\d+-Core\s+Processor\s*$` regex, without the
/// regex engine dependency.
pub fn cpu_name_trim(s: &str) -> String {
    let trimmed = s.trim_end();
    let Some(rest) = trimmed.strip_suffix("Processor") else {
        return s.to_string();
    };
    if !rest.ends_with(char::is_whitespace) {
        return s.to_string();
    }
    let Some(rest) = rest.trim_end().strip_suffix("Core") else {
        return s.to_string();
    };
    let Some(digits) = rest.strip_suffix('-') else {
        return s.to_string();
    };
    // Only the trailing run needs to be digits; anything before it is the
    // model name (and any whitespace before `\d+` is consumed by `\s*`).
    let digit_len = digits
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .count();
    if digit_len == 0 {
        return s.to_string();
    }
    digits[..digits.len() - digit_len].trim_end().to_string()
}

/// Hottest sysfs temperature in °C: max of thermal zones and hwmon inputs.
pub fn read_cpu_temp() -> Option<f64> {
    let mut best: Option<f64> = None;
    let mut consider = |raw: String| {
        if let Ok(v) = raw.trim().parse::<f64>()
            && v > 0.0
        {
            let c = v / 1000.0;
            if best.map(|b| c > b).unwrap_or(true) {
                best = Some(c);
            }
        }
    };

    for p in glob_dir_entries("/sys/class/thermal", "thermal_zone", "") {
        let path = format!("/sys/class/thermal/{}/temp", p);
        if let Ok(raw) = std::fs::read_to_string(path) {
            consider(raw);
        }
    }
    for p in glob_dir_entries("/sys/class/hwmon", "hwmon", "") {
        for f in glob_dir_entries(&format!("/sys/class/hwmon/{}", p), "temp", "_input") {
            let path = format!("/sys/class/hwmon/{}/{}", p, f);
            if let Ok(raw) = std::fs::read_to_string(path) {
                consider(raw);
            }
        }
    }
    best
}

/// Minimal stand-in for `filepath.Glob` on a single directory level:
/// entries starting with `prefix` and ending with `suffix`.
fn glob_dir_entries(dir: &str, prefix: &str, suffix: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let n = entry.file_name().to_string_lossy().into_owned();
            if n.starts_with(prefix)
                && n.ends_with(suffix)
                && n.len() >= prefix.len() + suffix.len()
            {
                out.push(n);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_name_trim_strips_redundant_suffix() {
        let cases = [
            ("AMD Ryzen 5 7600 6-Core Processor", "AMD Ryzen 5 7600"),
            ("AMD Ryzen 9 7950X 16-Core Processor", "AMD Ryzen 9 7950X"),
            ("Intel(R) Core(TM) i7-14700K", "Intel(R) Core(TM) i7-14700K"),
            ("Apple M1 Pro", "Apple M1 Pro"),
        ];
        for (input, want) in cases {
            assert_eq!(cpu_name_trim(input), want, "trim({:?})", input);
        }
    }

    #[test]
    fn name_is_performance() {
        assert_eq!(Monitor::new().name(), "performance");
    }

    #[test]
    fn reads_real_proc_files() {
        let (idle, total) = read_cpu().unwrap();
        assert!(total >= idle);
        let (total_kb, avail_kb, _, _) = read_mem().unwrap();
        assert!(total_kb > 0 && avail_kb <= total_kb);
    }

    #[test]
    fn snapshot_omits_empty_optional_fields() {
        let v = Monitor::new().snapshot(&Ctx::new()).unwrap();
        assert!(v.get("cpu_percent").is_some());
        assert!(v.get("mem_total_kb").is_some());
        assert!(v.get("sampled_at_unix").is_some());
    }
}
