//! The registered "network" monitor: one snapshot plus derived aggregates.
//! Port of `network/monitor.go`.

use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use std::time::SystemTime;

use super::{Collector, PingTracker, ThroughputTracker};
use crate::monitor::Monitor as MonitorTrait;
use crate::runner::{Ctx, OutputRunner};

/// The registered "network" monitor.
pub struct Monitor {
    collector: Collector,
    tracker: ThroughputTracker,
    pings: PingTracker,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self::new_with(
            Collector::new(Arc::new(OutputRunner::default())),
            ThroughputTracker::new(),
            PingTracker::new(),
        )
    }

    /// Constructor used by tests to inject a stub collector and trackers.
    pub fn new_with(collector: Collector, tracker: ThroughputTracker, pings: PingTracker) -> Self {
        Self {
            collector,
            tracker,
            pings,
        }
    }
}

impl MonitorTrait for Monitor {
    fn name(&self) -> &'static str {
        "network"
    }

    fn snapshot(&self, ctx: &Ctx) -> Result<Value> {
        let ctx = ctx.with_timeout(std::time::Duration::from_secs(8));
        let mut st = self.collector.collect(&ctx)?;
        let now = SystemTime::now();
        let (down, up) = self
            .tracker
            .update(&st.iface, st.rx_bytes, st.tx_bytes, now);
        st.download_bps = down;
        st.upload_bps = up;
        let stats = self
            .pings
            .update(&st.iface, st.router_ping_ms, st.internet_ping_ms);
        st.router_ping_avg_ms = stats.router_avg;
        st.internet_ping_avg_ms = stats.internet_avg;
        st.internet_loss_pct = stats.internet_loss_pct;
        st.ping_samples = stats.samples as i32;
        Ok(serde_json::to_value(st)?)
    }
}
