//! Active power-profiles-daemon profile plus the available set, parsed from
//! `powerprofilesctl list` and `powerprofilesctl get`.
//! Port of `internal/monitor/powerprofile/{powerprofile,status,monitor}.go`.

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

use crate::caps;
use crate::monitor::Monitor as MonitorTrait;
use crate::runner::{Ctx, OutputRunner, Runner};

/// The daemon's fixed profile set.
pub const KNOWN_PROFILES: &[&str] = &["performance", "balanced", "power-saver"];

/// The power-profile state.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Status {
    pub active: String,
    pub profiles: Vec<String>,
}

/// Collector gathers power-profile state through a Runner.
pub struct Collector {
    pub run: Arc<dyn Runner>,
    pub has: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl Collector {
    pub fn new(run: Arc<dyn Runner>) -> Self {
        Self {
            run,
            has: Arc::new(caps::available),
        }
    }

    pub fn collect(&self, ctx: &Ctx) -> Result<Status> {
        if !(self.has)("powerprofilesctl") {
            return Err(anyhow!("power-profile: powerprofilesctl not available"));
        }
        let out = self
            .run
            .run(ctx, "powerprofilesctl", &["list"])
            .map_err(|e| anyhow!("power-profile: list: {}", run_err(&e)))?;
        let (mut active, profiles) = parse_list(&out);
        if let Ok(out) = self.run.run(ctx, "powerprofilesctl", &["get"]) {
            let got = out.trim();
            if !got.is_empty() {
                active = got.to_string();
            }
        }
        Ok(Status { active, profiles })
    }
}

fn run_err(text: &str) -> String {
    if text.is_empty() {
        "command failed".to_string()
    } else {
        text.to_string()
    }
}

/// Decode `powerprofilesctl list`: top-level `<name>:` lines in list order,
/// the active one prefixed with `* `. Detail lines (indent 4+) are ignored.
pub fn parse_list(out: &str) -> (String, Vec<String>) {
    let mut active = String::new();
    let mut profiles = Vec::new();
    for line in out.split('\n') {
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        if indent > 2 {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut name = trimmed.strip_suffix(':').unwrap_or(trimmed);
        let is_active = if let Some(rest) = name.strip_prefix("* ") {
            name = rest;
            true
        } else {
            false
        };
        let name = name.trim();
        if !valid_profile(name) {
            continue;
        }
        profiles.push(name.to_string());
        if is_active {
            active = name.to_string();
        }
    }
    (active, profiles)
}

/// Whether `name` is a settable profile.
pub fn valid_profile(name: &str) -> bool {
    KNOWN_PROFILES.contains(&name)
}

/// The registered "power-profile" monitor.
pub struct Monitor {
    collector: Collector,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            collector: Collector::new(Arc::new(OutputRunner::default())),
        }
    }

    pub fn new_with_collector(collector: Collector) -> Self {
        Self { collector }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorTrait for Monitor {
    fn name(&self) -> &'static str {
        "power-profile"
    }

    fn snapshot(&self, ctx: &Ctx) -> Result<Value> {
        let ctx = ctx.with_timeout(std::time::Duration::from_secs(8));
        Ok(serde_json::to_value(self.collector.collect(&ctx)?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;

    const STUB_LIST: &str = "  performance:\n     CpuDriver:\tamd_pstate\n     Degraded:   no\n\n * balanced:\n     CpuDriver:\tamd_pstate\n     PlatformDriver:\tplaceholder\n\n  power-saver:\n     CpuDriver:\tamd_pstate\n     PlatformDriver:\tplaceholder\n";
    // The Go fixture uses `* balanced:` with a single leading space.
    const STUB_LIST_GO: &str = "  performance:\n     CpuDriver:\tamd_pstate\n     Degraded:   no\n\n* balanced:\n     CpuDriver:\tamd_pstate\n     PlatformDriver:\tplaceholder\n\n  power-saver:\n     CpuDriver:\tamd_pstate\n     PlatformDriver:\tplaceholder\n";

    #[test]
    fn parse_list_finds_active_and_profiles() {
        let (active, profiles) = parse_list(STUB_LIST_GO);
        assert_eq!(active, "balanced");
        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0], "performance");
        assert_eq!(profiles[2], "power-saver");
    }

    #[test]
    fn parse_list_skips_details() {
        let (active, profiles) = parse_list("CpuDriver:\n    performance:\n* bogus:\n");
        assert_eq!(active, "");
        assert!(profiles.is_empty());
    }

    #[test]
    fn parse_list_handles_indented_active_marker() {
        let (active, profiles) = parse_list(STUB_LIST);
        assert_eq!(active, "balanced");
        assert_eq!(profiles, vec!["performance", "balanced", "power-saver"]);
    }

    #[test]
    fn valid_profile_set() {
        for p in KNOWN_PROFILES {
            assert!(valid_profile(p));
        }
        assert!(!valid_profile("turbo"));
    }

    #[test]
    fn collect_uses_get_to_override_active() {
        let run = FnRunner(|_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            match key.as_str() {
                "powerprofilesctl list" => Ok(STUB_LIST_GO.to_string()),
                "powerprofilesctl get" => Ok("performance\n".to_string()),
                _ => Err("no stub".into()),
            }
        });
        let mut c = Collector::new(Arc::new(run));
        c.has = Arc::new(|_| true);
        let st = c.collect(&Ctx::new()).unwrap();
        assert_eq!(st.active, "performance");
        assert_eq!(st.profiles.len(), 3);
    }

    #[test]
    fn collect_requires_tool() {
        let c = Collector::new(Arc::new(FnRunner(|_: &Ctx, _: &str, _: &[&str]| {
            Err("no stub".into())
        })));
        assert!(c.collect(&Ctx::new()).is_err());
        assert_eq!(Monitor::new().name(), "power-profile");
    }
}
