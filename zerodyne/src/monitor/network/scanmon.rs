//! The "wifi" monitor: an instant cache of visible APs, refreshed lazily in
//! the background. Port of `network/scanmon.go`.

use anyhow::Result;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::scan::{ScanNetwork, scan_device, scan_visible};
use crate::monitor::Monitor as MonitorTrait;
use crate::runner::{CancelToken, Ctx, OutputRunner, Runner};

/// Background rescan period. NM scans take seconds; they never touch a client
/// request path.
pub const SCAN_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Default)]
struct ScanCache {
    dev: String,
    nets: Vec<ScanNetwork>,
}

/// The registered "wifi" monitor.
pub struct ScanMonitor {
    run: Arc<dyn Runner>,
    state: Arc<Mutex<ScanCache>>,
    cancel: Mutex<Option<CancelToken>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Default for ScanMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanMonitor {
    pub fn new() -> Self {
        // Real NM scans take several seconds; the short default would kill
        // every background refresh.
        Self::with_runner(Arc::new(OutputRunner {
            timeout: Duration::from_secs(25),
        }))
    }

    pub fn new_with_runner(run: Arc<dyn Runner>) -> Self {
        Self::with_runner(run)
    }

    fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self {
            run,
            state: Arc::new(Mutex::new(ScanCache::default())),
            cancel: Mutex::new(None),
            worker: Mutex::new(None),
        }
    }

    /// One real scan into the cache. Slow path only.
    pub fn refresh(&self, ctx: &Ctx) {
        refresh_into(&self.run, &self.state, ctx);
    }
}

fn lock<T>(mu: &Mutex<T>) -> MutexGuard<'_, T> {
    mu.lock().unwrap_or_else(|e| e.into_inner())
}

fn refresh_into(run: &Arc<dyn Runner>, state: &Arc<Mutex<ScanCache>>, ctx: &Ctx) {
    let dev = scan_device(ctx, run.as_ref());
    if dev.is_empty() {
        return;
    }
    let Ok(nets) = scan_visible(ctx, run.as_ref(), &dev, true) else {
        return;
    };
    let mut s = lock(state);
    s.dev = dev;
    s.nets = nets;
}

impl MonitorTrait for ScanMonitor {
    fn name(&self) -> &'static str {
        "wifi"
    }

    /// Instant: serves the cache, never scans.
    fn snapshot(&self, _ctx: &Ctx) -> Result<Value> {
        let s = lock(&self.state);
        Ok(json!({ "device": s.dev, "networks": s.nets }))
    }

    /// Warms the cache, then rescans until stopped.
    fn start(&self, ctx: &Ctx) -> Result<()> {
        let token = CancelToken::new();
        let run = self.run.clone();
        let state = self.state.clone();
        // The token reaches every command the worker launches: run_process
        // kills a child immediately when cancellation fires.
        let worker_ctx = ctx.clone().with_cancel(token.clone());
        let handle = thread::spawn(move || {
            loop {
                refresh_into(&run, &state, &worker_ctx);
                // Refresh-then-wait: ticks coalesce behind a slow scan.
                let mut waited = Duration::ZERO;
                let step = Duration::from_millis(50);
                while waited < SCAN_INTERVAL && !worker_ctx.is_cancelled() {
                    thread::sleep(step);
                    waited += step;
                }
                if worker_ctx.is_cancelled() {
                    break;
                }
            }
        });
        *lock(&self.cancel) = Some(token);
        *lock(&self.worker) = Some(handle);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        // Cancel first, then join: an in-flight nmcli is killed rather than
        // holding shutdown for the full 25s scan timeout.
        if let Some(token) = lock(&self.cancel).take() {
            token.cancel();
        }
        if let Some(handle) = lock(&self.worker).take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    #[test]
    fn stop_cancels_in_flight_scan() {
        let entered = Arc::new(AtomicBool::new(false));
        let seen = entered.clone();
        let run = Arc::new(FnRunner(move |ctx, name, _args| {
            if name == "nmcli" {
                seen.store(true, Ordering::SeqCst);
                // Stands in for a child nmcli that run_process would kill.
                while !ctx.is_cancelled() {
                    thread::sleep(Duration::from_millis(2));
                }
                return Err("cancelled".into());
            }
            Ok(String::new())
        }));
        let monitor = ScanMonitor::new_with_runner(run);
        monitor.start(&Ctx::new()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !entered.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline, "worker never started");
            thread::sleep(Duration::from_millis(1));
        }
        let started = Instant::now();
        monitor.stop().unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "stop took {:?}",
            started.elapsed()
        );
    }
}
