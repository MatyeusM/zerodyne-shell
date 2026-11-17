//! Pure audio parsers and classifiers, shared by the monitor and interaction
//! layers. Port of `internal/monitor/audio/parse.go` plus the parsing helpers
//! from `status.go`.

use crate::runner::{Ctx, Runner};
use std::collections::HashMap;

pub const CLASS_HEADPHONES: &str = "headphones";
pub const CLASS_HEADSET: &str = "headset";
pub const CLASS_PHONE: &str = "phone";
pub const CLASS_SPEAKER: &str = "speaker";
pub const CLASS_HDMI: &str = "hdmi";
pub const CLASS_CAR: &str = "car";
pub const CLASS_CAMERA: &str = "camera";
pub const CLASS_MICROPHONE: &str = "microphone";
pub const CLASS_OTHER: &str = "other";

/// First token ending in `%` **on line 1** (not the line-wide maximum).
pub fn parse_sink_volume(out: &str) -> Option<i32> {
    let line = out.split('\n').next().unwrap_or("");
    for tok in line.split_whitespace() {
        if let Some(raw) = tok.strip_suffix('%')
            && let Ok(v) = raw.parse::<i32>()
        {
            return Some(v);
        }
    }
    None
}

/// `wpctl get-volume`: find `Volume:` then the next token; muted anywhere.
pub fn parse_wpctl_volume(out: &str) -> (i32, bool) {
    let muted = out.contains("MUTED");
    let fields: Vec<&str> = out.split_whitespace().collect();
    for (i, tok) in fields.iter().enumerate() {
        if *tok == "Volume:"
            && i + 1 < fields.len()
            && let Ok(v) = fields[i + 1].parse::<f64>()
        {
            return ((v * 100.0 + 0.5) as i32, muted);
        }
    }
    (0, muted)
}

/// Follow a DSP sink down to the physical sink that owns loudness.
pub fn resolve_volume_sink(ctx: &Ctx, run: &dyn Runner, sink: &str) -> String {
    let sink = sink.trim();
    if sink.is_empty() || sink.starts_with("alsa_") {
        return sink.to_string();
    }
    let inputs = match run.run(ctx, "pactl", &["list", "sink-inputs"]) {
        Ok(out) => out,
        Err(_) => return sink.to_string(),
    };
    let downstream = follow_downstream(&inputs, sink);
    if downstream.is_empty() {
        return sink.to_string();
    }
    let name = sink_name_for_id(ctx, run, &downstream);
    if name.is_empty() {
        sink.to_string()
    } else {
        name
    }
}

/// Scan `pactl list sink-inputs` for the stream a virtual sink feeds through.
pub fn follow_downstream(inputs: &str, virt: &str) -> String {
    let mut target = String::new();
    for line in inputs.split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("Sink Input #") {
            target.clear();
            continue;
        }
        if trimmed.starts_with("Sink:") {
            target = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            continue;
        }
        if target.is_empty() {
            continue;
        }
        if line.contains("node.name = ")
            && let Some(i) = line.find("node.name = \"")
        {
            let name = line[i + "node.name = \"".len()..]
                .strip_suffix('"')
                .unwrap_or(&line[i + "node.name = \"".len()..]);
            if name.starts_with(virt) {
                return target;
            }
        }
        if virt == "easyeffects_sink" && line.contains("application.name = \"EasyEffects\"") {
            return target;
        }
    }
    String::new()
}

/// Map a numeric sink id to its name via `pactl list sinks short`.
pub fn sink_name_for_id(ctx: &Ctx, run: &dyn Runner, id: &str) -> String {
    let out = match run.run(ctx, "pactl", &["list", "sinks", "short"]) {
        Ok(out) => out,
        Err(_) => return String::new(),
    };
    for line in out.split('\n') {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 2 && fields[0] == id {
            return fields[1].to_string();
        }
    }
    String::new()
}

/// Substring ladder: headphone before phone, bluetooth never overrides kind.
pub fn classify_sink(name: &str, desc: &str, props: &HashMap<String, String>) -> (String, bool) {
    let blob = class_blob(name, desc, props);
    let class = if has_any(&blob, &["headphone", "earbud", "earphone", "airpod"]) {
        CLASS_HEADPHONES
    } else if has_any(&blob, &["headset", "hands-free", "handsfree"]) {
        CLASS_HEADSET
    } else if has_any(&blob, &["hdmi", "display"]) {
        CLASS_HDMI
    } else if has_any(&blob, &["car"]) {
        CLASS_CAR
    } else if blob.contains("phone") {
        CLASS_PHONE
    } else {
        CLASS_SPEAKER
    };
    let bluetooth = has_any(&blob, &["bluetooth", "bluez", "a2dp"]);
    (class.to_string(), bluetooth)
}

pub fn classify_source(name: &str, desc: &str, props: &HashMap<String, String>) -> String {
    let blob = class_blob(name, desc, props);
    let class = if has_any(
        &blob,
        &[
            "headset",
            "headphone",
            "earbud",
            "earphone",
            "airpod",
            "hands-free",
            "handsfree",
        ],
    ) {
        CLASS_HEADSET
    } else if has_any(&blob, &["webcam", "camera"]) {
        CLASS_CAMERA
    } else if has_any(&blob, &["phone"]) {
        CLASS_PHONE
    } else {
        CLASS_MICROPHONE
    };
    class.to_string()
}

/// Lowercased identifying strings, in Go's key order (including the duplicate
/// `device.product.name` entry).
fn class_blob(name: &str, desc: &str, props: &HashMap<String, String>) -> String {
    let mut parts = vec![name.to_string(), desc.to_string()];
    for k in [
        "device.icon-name",
        "device.icon_name",
        "device.product.name",
        "device.product.name",
        "node.description",
        "node.nick",
        "device.bus",
        "device.form_factor",
        "api.bluez5.icon",
    ] {
        if let Some(v) = props.get(k)
            && !v.is_empty()
        {
            parts.push(v.clone());
        }
    }
    parts.join(" ").to_lowercase()
}

fn has_any(blob: &str, subs: &[&str]) -> bool {
    subs.iter().any(|s| blob.contains(s))
}

/// IDs of real application sink-inputs; DSP chains and EasyEffects are skipped.
pub fn app_sink_input_ids(inputs: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut id = String::new();
    let mut app = String::new();
    let flush = |ids: &mut Vec<String>, id: &mut String, app: &mut String| {
        if !id.is_empty() && !app.is_empty() && app != "EasyEffects" {
            ids.push(id.clone());
        }
        id.clear();
        app.clear();
    };
    for line in inputs.split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("Sink Input #") {
            flush(&mut ids, &mut id, &mut app);
            id = trimmed
                .strip_prefix("Sink Input #")
                .unwrap_or("")
                .to_string();
            continue;
        }
        if let Some(i) = line.find("application.name = \"") {
            let rest = &line[i + "application.name = \"".len()..];
            app = rest.strip_suffix('"').unwrap_or(rest).to_string();
        }
    }
    flush(&mut ids, &mut id, &mut app);
    ids
}
