//! Shared sequencing helpers for the audio interaction handlers.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};

use crate::monitor::audio::{Collector, app_sink_input_ids};
use crate::runner::{CombinedRunner, Ctx, Runner};

pub(crate) fn live() -> Arc<dyn Runner> {
    Arc::new(CombinedRunner::default())
}

/// A context for a single sequencing step (Go's `step` helper).
pub(crate) fn step(ctx: &Ctx, secs: u64) -> Ctx {
    ctx.with_timeout(Duration::from_secs(secs))
}

/// Resolve the sink loudness lives on (through any DSP sink).
pub(crate) fn volume_sink(ctx: &Ctx, run: &dyn Runner) -> Result<String> {
    let sctx = step(ctx, 5);
    let def = crate::monitor::audio::default_sink(&sctx, run);
    if def.is_empty() {
        return Err(anyhow!("audio-volume: no audio sink"));
    }
    Ok(crate::monitor::audio::resolve_volume_sink(&sctx, run, &def))
}

pub(crate) fn clamp_pct(v: i32) -> i32 {
    v.clamp(0, 100)
}

/// Parse the `+N` / `-N` delta form (Go's `^([+-])(\d+)$`).
pub(crate) fn parse_delta(action: &str) -> Option<(char, i32)> {
    let sign = action.chars().next()?;
    if sign != '+' && sign != '-' {
        return None;
    }
    let digits = &action[1..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((sign, digits.parse().ok()?))
}

/// Port of the pure `wpctl get-volume` parser (`Volume: 0.80 [MUTED]`).
pub(crate) fn wpctl_volume_pct(out: &str) -> i32 {
    let fields: Vec<&str> = out.split_whitespace().collect();
    for (i, tok) in fields.iter().enumerate() {
        if *tok == "Volume:"
            && i + 1 < fields.len()
            && let Ok(v) = fields[i + 1].parse::<f64>()
        {
            return (v * 100.0 + 0.5) as i32;
        }
    }
    0
}

/// Shared set-default sequencing for output handlers. Stream moves are
/// best-effort: the default is already set, so a racing stream is cosmetic.
pub(crate) fn apply_output_default(ctx: &Ctx, run: &dyn Runner, node_id: &str, sink_name: &str) {
    let sctx = step(ctx, 10);
    let _ = run.run(&sctx, "wpctl", &["set-default", node_id]);
    let _ = run.run(&sctx, "pactl", &["set-default-sink", sink_name]);
    if let Ok(out) = run.run(&sctx, "pactl", &["list", "sink-inputs"]) {
        for id in app_sink_input_ids(&out) {
            let _ = run.run(&sctx, "pactl", &["move-sink-input", &id, sink_name]);
        }
    }
}

/// A monitor collector over a handler's runner, with all tools assumed present.
pub(crate) fn collector(run: Arc<dyn Runner>) -> Collector {
    let mut c = Collector::new(run);
    c.has = Arc::new(|_| true);
    c
}

#[cfg(test)]
pub(crate) struct AudioCall {
    pub name: String,
    pub args: Vec<String>,
}

#[cfg(test)]
pub(crate) fn audio_sequence(calls: &[AudioCall]) -> String {
    calls
        .iter()
        .map(|c| format!("{} {}", c.name, c.args.join(" ")))
        .collect::<Vec<_>>()
        .join("\n")
}
