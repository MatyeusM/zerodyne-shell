//! Visible-access-point collection shared by the background wifi monitor and
//! the forced-rescan interaction. Port of `network/scan.go`.

use anyhow::{Result, anyhow};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use crate::runner::{Ctx, Runner};

/// One visible access point.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ScanNetwork {
    pub ssid: String,
    pub bssid: String,
    pub signal: i32,
    pub security: String,
    pub in_use: bool,
    pub known: bool,
}

/// Split `AA:BB:CC:DD:EE:FF:<ssid>` into MAC and SSID. Equivalent to the old
/// `^((?:[0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}):(.*)$` regex, but the SSID keeps
/// any colons verbatim and no regex engine is needed.
fn split_bssid(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    if bytes.len() < 18 || bytes[17] != b':' {
        return None;
    }
    for (i, byte) in bytes.iter().enumerate().take(17) {
        let ok = match i % 3 {
            0 | 1 => byte.is_ascii_hexdigit(),
            _ => *byte == b':',
        };
        if !ok {
            return None;
        }
    }
    Some((&s[..17], &s[18..]))
}

/// The connected Wi-Fi device, falling back to the first Wi-Fi device seen.
pub fn scan_device(ctx: &Ctx, run: &dyn Runner) -> String {
    let out = match run.run(
        ctx,
        "nmcli",
        &[
            "-e",
            "no",
            "-t",
            "-f",
            "DEVICE,TYPE,STATE",
            "device",
            "status",
        ],
    ) {
        Ok(out) => out,
        Err(_) => return String::new(),
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

/// The set of saved connection names.
fn scan_known(ctx: &Ctx, run: &dyn Runner) -> HashSet<String> {
    let mut set = HashSet::new();
    let out = match run.run(
        ctx,
        "nmcli",
        &["-e", "no", "-t", "-f", "NAME", "connection", "show"],
    ) {
        Ok(out) => out,
        Err(_) => return set,
    };
    for line in out.split('\n') {
        let name = line.trim();
        if !name.is_empty() {
            set.insert(name.to_string());
        }
    }
    set
}

/// List visible APs. `rescan` forces a real NM scan (slow).
pub fn scan_visible(
    ctx: &Ctx,
    run: &dyn Runner,
    dev: &str,
    rescan: bool,
) -> Result<Vec<ScanNetwork>> {
    let mode = if rescan { "yes" } else { "no" };
    let out = run
        .run(
            ctx,
            "nmcli",
            &[
                "-e",
                "no",
                "-t",
                "-f",
                "IN-USE,SIGNAL,SECURITY,BSSID,SSID",
                "dev",
                "wifi",
                "list",
                "ifname",
                dev,
                "--rescan",
                mode,
            ],
        )
        .map_err(|e| {
            anyhow!(
                "wifi scan: {}",
                if e.is_empty() { "command failed" } else { &e }
            )
        })?;
    let known = scan_known(ctx, run);
    let mut best: HashMap<String, ScanNetwork> = HashMap::new();
    for line in out.split('\n') {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() != 4 {
            continue;
        }
        let Ok(signal) = parts[1].trim().parse::<i32>() else {
            continue;
        };
        let Some((bssid, ssid_raw)) = split_bssid(parts[3]) else {
            continue;
        };
        let ssid = ssid_raw.trim().to_string();
        if ssid.is_empty() {
            continue;
        }
        let row = ScanNetwork {
            ssid: ssid.clone(),
            bssid: bssid.to_string(),
            signal,
            security: wifi_sec_class(parts[2]),
            in_use: parts[0].trim() == "*",
            known: known.contains(&ssid),
        };
        // One row per SSID: keep the strongest AP, sticky in-use flag.
        match best.get(&ssid) {
            None => {
                best.insert(ssid, row);
            }
            Some(cur) if row.signal > cur.signal => {
                let mut r = row;
                r.in_use = r.in_use || cur.in_use;
                best.insert(ssid, r);
            }
            Some(cur) if row.in_use => {
                let mut c = cur.clone();
                c.in_use = true;
                best.insert(ssid, c);
            }
            Some(_) => {}
        }
    }
    let mut nets: Vec<ScanNetwork> = best.into_values().collect();
    nets.sort_by(|a, b| {
        b.in_use
            .cmp(&a.in_use)
            .then_with(|| b.signal.cmp(&a.signal))
            .then_with(|| a.ssid.cmp(&b.ssid))
            .then_with(|| a.bssid.cmp(&b.bssid))
    });
    Ok(nets)
}

/// Reduce NM's security string to a UI class.
fn wifi_sec_class(s: &str) -> String {
    let u = s.to_uppercase();
    if u.contains("802.1X") || u.contains("EAP") {
        "eap"
    } else if u.contains("WEP") {
        "wep"
    } else if u.contains("WPA") || u.contains("SAE") || u.contains("OWE") {
        "wpa"
    } else {
        "open"
    }
    .to_string()
}
