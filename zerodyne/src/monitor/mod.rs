//! Monitor abstraction and registry. A monitor is read-only: it answers
//! "what is true right now" and never changes state. On-demand snapshots are
//! the norm; monitors that accumulate data (the background wifi scan cache)
//! opt into `start`/`stop` with the daemon's lifecycle.

pub mod audio;
pub mod gpu;
pub mod network;
pub mod performance;
pub mod powerprofile;
pub mod system;

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::runner::Ctx;

pub trait Monitor: Send + Sync {
    /// The `status <target>` selector, e.g. "network".
    fn name(&self) -> &'static str;
    /// Current state. Must be safe for concurrent use and respect the ctx.
    fn snapshot(&self, ctx: &Ctx) -> Result<Value>;
    /// Optional background collection; default is a no-op.
    fn start(&self, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    /// End background collection; default is a no-op.
    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

/// Maps monitor names to monitors. Registration happens once at startup.
pub struct Registry {
    monitors: RwLock<BTreeMap<String, Arc<dyn Monitor>>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            monitors: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn register(&self, m: Arc<dyn Monitor>) -> Result<()> {
        if m.name().is_empty() {
            return Err(anyhow!("monitor: unnamed monitor"));
        }
        let mut map = self.monitors.write().unwrap();
        if map.contains_key(m.name()) {
            return Err(anyhow!("monitor: duplicate {:?}", m.name()));
        }
        map.insert(m.name().to_string(), m);
        Ok(())
    }

    /// Registration is fatal at startup, so mirror Go's MustRegister.
    pub fn must_register(&self, m: Arc<dyn Monitor>) {
        self.register(m).expect("monitor registration failed");
    }

    pub fn snapshot(&self, ctx: &Ctx, name: &str) -> Result<Value> {
        let m = self
            .monitors
            .read()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "unknown status target {:?} (known: {})",
                    name,
                    crate::go_list(&self.names())
                )
            })?;
        m.snapshot(ctx)
    }

    pub fn start_all(&self, ctx: &Ctx) {
        for m in self.monitors.read().unwrap().values() {
            let _ = m.start(ctx);
        }
    }

    pub fn stop_all(&self) {
        for m in self.monitors.read().unwrap().values() {
            let _ = m.stop();
        }
    }

    /// Sorted registered names (for errors/help).
    pub fn names(&self) -> Vec<String> {
        self.monitors.read().unwrap().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub(&'static str, &'static str);

    impl Monitor for Stub {
        fn name(&self) -> &'static str {
            self.0
        }
        fn snapshot(&self, _: &Ctx) -> Result<Value> {
            Ok(Value::String(self.1.into()))
        }
    }

    #[test]
    fn dispatch_and_duplicates() {
        let r = Registry::new();
        r.must_register(Arc::new(Stub("b", "bee")));
        r.must_register(Arc::new(Stub("a", "aye")));
        assert_eq!(r.names(), vec!["a", "b"]);
        assert_eq!(r.snapshot(&Ctx::new(), "b").unwrap(), "bee");
        assert!(r.snapshot(&Ctx::new(), "nope").is_err());
        assert!(r.register(Arc::new(Stub("a", "dup"))).is_err());
        assert!(r.register(Arc::new(Stub("", "x"))).is_err());
    }
}
