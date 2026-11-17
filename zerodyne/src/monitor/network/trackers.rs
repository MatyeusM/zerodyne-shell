//! Accumulated network state: byte rates and ping history aggregates.
//! Port of `network/status.go` (ThroughputTracker / PingTracker sections).

use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PING_HISTORY_WINDOW: usize = 24;
pub const PING_AVERAGE_WINDOW: usize = 5;

fn clamp_non_neg(v: f64) -> f64 {
    if v < 0.0 { 0.0 } else { v }
}

/// Derives B/s rates from consecutive counter samples.
pub struct ThroughputTracker {
    mu: Mutex<ThruState>,
}

#[derive(Default)]
struct ThruState {
    iface: String,
    rx: u64,
    tx: u64,
    at: i64,
    down: f64,
    up: f64,
}

impl Default for ThroughputTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroughputTracker {
    pub fn new() -> Self {
        Self {
            mu: Mutex::new(ThruState::default()),
        }
    }

    pub fn update(&self, iface: &str, rx: u64, tx: u64, now: SystemTime) -> (f64, f64) {
        let nanos = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;
        self.update_nanos(iface, rx, tx, nanos)
    }

    pub fn update_nanos(&self, iface: &str, rx: u64, tx: u64, now_nanos: i64) -> (f64, f64) {
        let mut s = lock(&self.mu);
        if iface != s.iface || s.at == 0 {
            s.iface = iface.to_string();
            s.rx = rx;
            s.tx = tx;
            s.at = now_nanos;
            s.down = 0.0;
            s.up = 0.0;
            return (0.0, 0.0);
        }
        let dt = (now_nanos - s.at) as f64 / 1e9;
        if dt > 0.0 {
            s.down = clamp_non_neg(rx as f64 - s.rx as f64) / dt;
            s.up = clamp_non_neg(tx as f64 - s.tx as f64) / dt;
            s.rx = rx;
            s.tx = tx;
            s.at = now_nanos;
        }
        (s.down, s.up)
    }
}

/// Aggregates one interface's ping history.
#[derive(Debug, Clone, PartialEq)]
pub struct PingStats {
    pub router_avg: Option<f64>,
    pub internet_avg: Option<f64>,
    pub internet_loss_pct: i32,
    pub samples: usize,
}

/// Per-interface ping histories; resets when the interface changes.
pub struct PingTracker {
    mu: Mutex<PingState>,
}

#[derive(Default)]
struct PingState {
    iface: String,
    router: Vec<Option<f64>>,
    internet: Vec<Option<f64>>,
}

impl Default for PingTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PingTracker {
    pub fn new() -> Self {
        Self {
            mu: Mutex::new(PingState::default()),
        }
    }

    pub fn update(&self, iface: &str, router: Option<f64>, internet: Option<f64>) -> PingStats {
        let mut s = lock(&self.mu);
        if iface != s.iface {
            s.iface = iface.to_string();
            s.router.clear();
            s.internet.clear();
        }
        append_sample(&mut s.router, router);
        append_sample(&mut s.internet, internet);
        PingStats {
            router_avg: average_ping(&s.router),
            internet_avg: average_ping(&s.internet),
            internet_loss_pct: loss_pct(&s.internet),
            samples: s.internet.len(),
        }
    }
}

fn lock<T>(mu: &Mutex<T>) -> MutexGuard<'_, T> {
    mu.lock().unwrap_or_else(|e| e.into_inner())
}

fn append_sample(samples: &mut Vec<Option<f64>>, v: Option<f64>) {
    samples.push(v);
    if samples.len() > PING_HISTORY_WINDOW {
        let start = samples.len() - PING_HISTORY_WINDOW;
        samples.drain(..start);
    }
}

/// Average the last 5 samples, skipping timeouts; nil when none succeeded.
fn average_ping(samples: &[Option<f64>]) -> Option<f64> {
    let start = samples.len().saturating_sub(PING_AVERAGE_WINDOW);
    let mut total = 0.0;
    let mut n = 0;
    for v in samples[start..].iter().flatten() {
        total += *v;
        n += 1;
    }
    if n == 0 { None } else { Some(total / n as f64) }
}

/// Share of timed-out samples, rounded like QML.
fn loss_pct(samples: &[Option<f64>]) -> i32 {
    if samples.is_empty() {
        return 0;
    }
    let lost = samples.iter().filter(|v| v.is_none()).count();
    (lost as f64 / samples.len() as f64 * 100.0 + 0.5) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fptr(v: f64) -> Option<f64> {
        Some(v)
    }

    #[test]
    fn throughput_tracker_resets_and_clamps() {
        let tr = ThroughputTracker::new();
        let (d, u) = tr.update_nanos("wlan0", 1000, 500, 1_000_000_000);
        assert_eq!((d, u), (0.0, 0.0));
        let (d, u) = tr.update_nanos("wlan0", 2000, 1000, 2_000_000_000);
        assert_eq!((d, u), (1000.0, 500.0));
        let (d, _) = tr.update_nanos("wlan0", 100, 100, 3_000_000_000);
        assert_eq!(d, 0.0);
        let (d, u) = tr.update_nanos("eth0", 9999, 9999, 4_000_000_000);
        assert_eq!((d, u), (0.0, 0.0));
    }

    #[test]
    fn ping_tracker_average_and_loss() {
        let tr = PingTracker::new();
        for v in [10.0, 20.0, 30.0, 40.0] {
            tr.update("wlan0", fptr(v), fptr(v));
        }
        let mut stats = tr.update("wlan0", None, None);
        assert_eq!(stats.samples, 5);
        assert_eq!(stats.internet_avg, Some(25.0));
        assert_eq!(stats.internet_loss_pct, 20);

        for _ in 0..PING_HISTORY_WINDOW {
            stats = tr.update("wlan0", None, None);
        }
        assert_eq!(stats.samples, PING_HISTORY_WINDOW);
        assert_eq!(stats.internet_avg, None);
        assert_eq!(stats.internet_loss_pct, 100);

        stats = tr.update("eth0", fptr(5.0), fptr(5.0));
        assert_eq!(stats.samples, 1);
        assert_eq!(stats.internet_loss_pct, 0);
    }
}
