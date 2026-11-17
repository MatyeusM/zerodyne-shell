//! Shared helpers for the Wi-Fi interaction handlers (ported from wifi.go).

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::runner::{Ctx, Runner};

/// Small helper matching Go's `json!({...}).as_object().unwrap().clone()`.
pub(crate) fn data(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    v.as_object().expect("object literal").clone()
}

/// A context for a single sequencing step (Go's `step` helper).
pub(crate) fn step(ctx: &Ctx, secs: u64) -> Ctx {
    ctx.with_timeout(Duration::from_secs(secs))
}

/// Run a child with combined output, a 15s ceiling, and `LC_ALL=C` pinned for
/// nmcli (Go's `liveRun`). `run_process` cannot set per-command env, so this
/// mirrors it with `Command::env` instead.
fn live_process(ctx: &Ctx, name: &str, args: &[&str]) -> Result<String, String> {
    let timeout = ctx.remaining(Duration::from_secs(15));
    let mut cmd = Command::new(name);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if name == "nmcli" {
        // nmcli translates state words such as "connected"; pin the locale.
        cmd.env("LC_ALL", "C");
    }
    let mut child = cmd.spawn().map_err(|e| format!("{}: {}", name, e))?;
    let drain = |mut r: std::process::ChildStdout| -> thread::JoinHandle<String> {
        thread::spawn(move || {
            let mut s = String::new();
            let _ = r.read_to_string(&mut s);
            s
        })
    };
    let out_h = child.stdout.take().map(drain);
    let err_h = child.stderr.take().map(|mut r| {
        thread::spawn(move || {
            let mut s = String::new();
            let _ = r.read_to_string(&mut s);
            s
        })
    });

    let deadline = Instant::now() + timeout;
    let mut ok = false;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                ok = status.success();
                break;
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(2)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            Err(_) => break,
        }
    }

    let mut text = out_h.and_then(|h| h.join().ok()).unwrap_or_default();
    text.push_str(&err_h.and_then(|h| h.join().ok()).unwrap_or_default());
    // Go's helpers use strings.TrimRight(out, "\n").
    let text = text.trim_end_matches('\n').to_string();
    if ok { Ok(text) } else { Err(text) }
}

struct LiveRunner;
impl Runner for LiveRunner {
    fn run(&self, ctx: &Ctx, name: &str, args: &[&str]) -> Result<String, String> {
        live_process(ctx, name, args)
    }
}

/// The default runner for every network handler.
pub(crate) fn live() -> Arc<dyn Runner> {
    Arc::new(LiveRunner)
}

/// `nmcli -e no -t -f <fields> ...`: unescaped ':'/'\' so SSIDs match iw.
pub(crate) fn nm_list(
    ctx: &Ctx,
    run: &dyn Runner,
    fields: &str,
    args: &[&str],
) -> Result<String, String> {
    let mut full = vec!["-e", "no", "-t", "-f", fields];
    full.extend_from_slice(args);
    run.run(ctx, "nmcli", &full)
}

/// `nmcli -e no -g <field> ...` with the output trimmed like Go's nmGet.
pub(crate) fn nm_get(
    ctx: &Ctx,
    run: &dyn Runner,
    field: &str,
    args: &[&str],
) -> Result<String, String> {
    let mut full = vec!["-e", "no", "-g", field];
    full.extend_from_slice(args);
    let out = run.run(ctx, "nmcli", &full)?;
    Ok(out.trim().to_string())
}

/// Connected Wi-Fi device, falling back to the first Wi-Fi device in any state.
pub(crate) fn first_wifi_device(ctx: &Ctx, run: &dyn Runner) -> String {
    let Ok(out) = nm_list(ctx, run, "DEVICE,TYPE,STATE", &["device", "status"]) else {
        return String::new();
    };
    let mut fallback = String::new();
    for line in out.split('\n') {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() != 3 || parts[1] != "wifi" {
            continue;
        }
        if fallback.is_empty() {
            fallback = parts[0].to_string();
        }
        if parts[2].starts_with("connected") {
            return parts[0].to_string();
        }
    }
    fallback
}

/// Resolve the default-route device (`ip route get 1.1.1.1`).
pub(crate) fn route_device(ctx: &Ctx, run: &dyn Runner) -> String {
    let Ok(out) = run.run(ctx, "ip", &["route", "get", "1.1.1.1"]) else {
        return String::new();
    };
    let fields: Vec<&str> = out.split_whitespace().collect();
    for (i, f) in fields.iter().enumerate() {
        if *f == "dev" && i + 1 < fields.len() {
            return fields[i + 1].to_string();
        }
    }
    String::new()
}

pub(crate) fn is_wireless_dir(dev: &str) -> bool {
    std::fs::metadata(format!("/sys/class/net/{}/wireless", dev))
        .map(|m| m.is_dir())
        .unwrap_or(false)
}

pub(crate) fn first_line(s: &str) -> String {
    s.split('\n').next().unwrap_or("").trim().to_string()
}

pub(crate) fn iw_ssid(link: &str) -> String {
    for line in link.split('\n') {
        if let Some(i) = line.find("SSID:") {
            return line[i + "SSID:".len()..].trim().to_string();
        }
    }
    String::new()
}

pub(crate) fn iw_freq(link: &str) -> String {
    for line in link.split('\n') {
        if let Some(v) = line.trim().strip_prefix("freq:") {
            return v.trim().to_string();
        }
    }
    String::new()
}

fn band_order(b: &str) -> u8 {
    match b {
        "2.4" => 0,
        "5" => 1,
        "6" => 2,
        _ => 3,
    }
}

/// Every band the SSID is reachable on (low to high), always including the
/// current one, from NM's cache (`--rescan no`).
pub(crate) fn available_bands(
    ctx: &Ctx,
    run: &dyn Runner,
    dev: &str,
    ssid: &str,
    current: &str,
) -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    if !current.is_empty() {
        set.insert(current.to_string());
    }
    if let Ok(out) = nm_get(
        ctx,
        run,
        "FREQ,SSID",
        &["dev", "wifi", "list", "ifname", dev, "--rescan", "no"],
    ) {
        for line in out.split('\n') {
            let Some(idx) = line.find(':') else {
                continue;
            };
            // SSID is everything after the first ':' reassembled verbatim.
            let (freq, name) = (&line[..idx], &line[idx + 1..]);
            if name != ssid {
                continue;
            }
            if let Some(b) = crate::monitor::network::band_for_freq(freq.trim()) {
                set.insert(b);
            }
        }
    }
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort_by_key(|b| band_order(b));
    out
}

/// A stub runner shared by the wifiband and wifi-scan test modules.
#[cfg(test)]
pub(crate) fn stub(outputs: Vec<(&'static str, &'static str)>) -> Arc<dyn Runner> {
    Arc::new(crate::runner::FnRunner(move |_ctx, name, args| {
        let key = format!("{} {}", name, args.join(" "));
        for (prefix, out) in &outputs {
            if key.starts_with(prefix) {
                return Ok((*out).to_string());
            }
        }
        Err(String::new())
    }))
}
