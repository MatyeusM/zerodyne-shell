//! GPU monitor: utilization, temperature, power and VRAM from
//! `amd-smi metric/static --json` (never rocm-smi). Port of
//! `internal/monitor/gpu/{gpu,status,monitor}.go`.

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

use crate::caps;
use crate::monitor::Monitor as MonitorTrait;
use crate::runner::{Ctx, OutputRunner, Runner};

/// The first GPU's state.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Status {
    pub name: String,
    pub util_pct: f64,
    pub temp_c: f64,
    pub power_w: f64,
    pub vram_used_mb: f64,
    pub vram_total_mb: f64,
    pub vram_pct: f64,
}

/// Collector gathers GPU state through a Runner.
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
        if !(self.has)("amd-smi") {
            return Err(anyhow!("gpu: amd-smi not available"));
        }
        let out = self
            .run
            .run(ctx, "amd-smi", &["metric", "-g", "0", "--json"])
            .map_err(|e| anyhow!("gpu: metric: {}", run_err(&e)))?;
        let mut st = parse_metric(&out)?;
        st.name = self.gpu_name(ctx);
        Ok(st)
    }

    /// Marketing name via `amd-smi static`, best effort (empty on failure).
    fn gpu_name(&self, ctx: &Ctx) -> String {
        let Ok(out) = self
            .run
            .run(ctx, "amd-smi", &["static", "-g", "0", "--json"])
        else {
            return String::new();
        };
        let Ok(doc) = serde_json::from_str::<Value>(&out) else {
            return String::new();
        };
        doc.get("gpu_data")
            .and_then(|d| d.get(0))
            .and_then(|g| g.get("asic"))
            .and_then(|a| a.get("market_name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string()
    }
}

fn run_err(text: &str) -> String {
    if text.is_empty() {
        "command failed".to_string()
    } else {
        text.to_string()
    }
}

/// Parse `amd-smi metric --json`. Measure values may be numbers or "N/A"
/// (and anything unparseable), which decode to zero like Go's `measure`.
pub fn parse_metric(out: &str) -> Result<Status> {
    let doc: Value = serde_json::from_str(out).map_err(|e| anyhow!("gpu: metric: {}", e))?;
    let Some(data) = doc.get("gpu_data") else {
        return Err(anyhow!("gpu: no gpu_data in metric output"));
    };
    let Some(gpus) = data.as_array() else {
        return Err(anyhow!("gpu: metric: gpu_data is not an array"));
    };
    let Some(g) = gpus.first() else {
        return Err(anyhow!("gpu: no gpu_data in metric output"));
    };
    let measure = |keys: &[&str]| -> f64 {
        keys.iter()
            .try_fold(g, |node, key| node.get(*key))
            .and_then(|m| m.get("value"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    };
    let util_pct = measure(&["usage", "gfx_activity"]);
    let temp_c = measure(&["temperature", "edge"]);
    let power_w = measure(&["power", "socket_power"]);
    let vram_used_mb = measure(&["mem_usage", "used_vram"]);
    let vram_total_mb = measure(&["mem_usage", "total_vram"]);
    let vram_pct = if vram_total_mb > 0.0 {
        vram_used_mb / vram_total_mb * 100.0
    } else {
        0.0
    };
    Ok(Status {
        name: String::new(),
        util_pct,
        temp_c,
        power_w,
        vram_used_mb,
        vram_total_mb,
        vram_pct,
    })
}

/// The registered "gpu" monitor.
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
        "gpu"
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
    use std::sync::Mutex;

    const STUB_METRIC: &str = r#"{"gpu_data": [{
      "gpu": 0,
      "usage": {"gfx_activity": {"value": 62, "unit": "%"}, "umc_activity": {"value": "N/A", "unit": "%"}},
      "power": {"socket_power": {"value": 120, "unit": "W"}},
      "temperature": {"edge": {"value": 54, "unit": "C"}, "hotspot": {"value": "N/A", "unit": "C"}},
      "mem_usage": {"used_vram": {"value": 2242, "unit": "MB"}, "total_vram": {"value": 16368, "unit": "MB"}}
    }]}"#;

    const STUB_STATIC: &str = r#"{"gpu_data": [{
      "gpu": 0,
      "asic": {"market_name": "AMD Radeon RX 7800 XT"}
    }]}"#;

    fn stub_gpu(
        static_ok: bool,
        calls: Option<Arc<Mutex<Vec<String>>>>,
    ) -> FnRunner<impl Fn(&Ctx, &str, &[&str]) -> Result<String, String>> {
        FnRunner(move |_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            if let Some(ref calls) = calls {
                calls.lock().unwrap().push(key.clone());
            }
            if key.contains("metric") {
                Ok(STUB_METRIC.to_string())
            } else if key.contains("static") {
                if static_ok {
                    Ok(STUB_STATIC.to_string())
                } else {
                    Err("no stub".into())
                }
            } else {
                Err("no stub".into())
            }
        })
    }

    #[test]
    fn collect_metric() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut c = Collector::new(Arc::new(stub_gpu(true, Some(calls.clone()))));
        c.has = Arc::new(|_| true);
        let st = c.collect(&Ctx::new()).unwrap();
        assert_eq!(st.name, "AMD Radeon RX 7800 XT");
        assert_eq!(st.util_pct, 62.0);
        assert_eq!(st.temp_c, 54.0);
        assert_eq!(st.power_w, 120.0);
        assert_eq!(st.vram_used_mb, 2242.0);
        assert_eq!(st.vram_total_mb, 16368.0);
        assert_eq!(st.vram_pct, st.vram_used_mb / st.vram_total_mb * 100.0);
        for k in calls.lock().unwrap().iter() {
            assert!(k.contains("-g 0"), "must select -g 0: {}", k);
            assert!(!k.contains("rocm-smi"), "never rocm-smi: {}", k);
        }
    }

    #[test]
    fn collect_static_best_effort() {
        let mut c = Collector::new(Arc::new(stub_gpu(false, None)));
        c.has = Arc::new(|_| true);
        let st = c.collect(&Ctx::new()).unwrap();
        assert_eq!(st.name, "");
        assert_eq!(st.util_pct, 62.0);
    }

    #[test]
    fn collect_requires_tool() {
        let mut c = Collector::new(Arc::new(stub_gpu(true, None)));
        c.has = Arc::new(|_| false);
        assert!(c.collect(&Ctx::new()).is_err());
    }

    #[test]
    fn collect_bad_json() {
        let mut c = Collector::new(Arc::new(FnRunner(|_: &Ctx, _: &str, _: &[&str]| {
            Ok("not json".to_string())
        })));
        c.has = Arc::new(|_| true);
        assert!(c.collect(&Ctx::new()).is_err());
    }

    #[test]
    fn parse_metric_na_is_zero() {
        let out = r#"{"gpu_data":[{"usage":{"gfx_activity":{"value":"N/A"}},"power":{"socket_power":{"value":0}},"temperature":{"edge":{"value":0}},"mem_usage":{"used_vram":{"value":0},"total_vram":{"value":0}}}]}"#;
        let st = parse_metric(out).unwrap();
        assert_eq!(st.util_pct, 0.0);
        assert_eq!(st.vram_pct, 0.0);
    }

    #[test]
    fn monitor_name_and_snapshot() {
        let mut c = Collector::new(Arc::new(stub_gpu(true, None)));
        c.has = Arc::new(|_| true);
        let m = Monitor::new_with_collector(c);
        assert_eq!(m.name(), "gpu");
        let v = m.snapshot(&Ctx::new()).unwrap();
        assert_eq!(v["name"], "AMD Radeon RX 7800 XT");
    }
}
