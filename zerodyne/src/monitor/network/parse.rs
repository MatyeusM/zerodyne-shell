//! Raw `iw` output and sysfs parsing helpers from `network/status.go`.
//! All of these treat missing/unparseable input as "unknown".

use std::path::Path;

/// SSID is the rest of the line, so ':' in SSIDs survives verbatim.
pub(crate) fn iw_ssid(link: &str) -> String {
    for line in link.split('\n') {
        if let Some(i) = line.find("SSID:") {
            return line[i + "SSID:".len()..].trim().to_string();
        }
    }
    String::new()
}

/// First whitespace-delimited token after `key`, parsed as an integer.
pub(crate) fn iw_int(link: &str, key: &str) -> Option<i32> {
    for line in link.split('\n') {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let token = rest.split_whitespace().next()?;
        return token.parse::<i32>().ok();
    }
    None
}

/// First whitespace-delimited token after `key`, parsed as a float.
pub(crate) fn iw_float(link: &str, key: &str) -> Option<f64> {
    for line in link.split('\n') {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let token = rest.split_whitespace().next()?;
        return token.parse::<f64>().ok();
    }
    None
}

/// `tx bitrate: <rate> <unit>` → (rate, unit).
pub(crate) fn iw_bitrate(link: &str) -> (String, String) {
    for line in link.split('\n') {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("tx bitrate:") {
            let fields: Vec<&str> = rest.split_whitespace().collect();
            let rate = fields.first().unwrap_or(&"").to_string();
            let unit = fields.get(1).unwrap_or(&"").to_string();
            return (rate, unit);
        }
    }
    (String::new(), String::new())
}

pub(crate) fn is_wireless(dev: &str) -> bool {
    Path::new(&format!("/sys/class/net/{}/wireless", dev)).is_dir()
}

pub(crate) fn sysfs_net(dev: &str, leaf: &str) -> String {
    format!("/sys/class/net/{}/{}", dev, leaf)
}

pub(crate) fn read_uint_sysfs(path: &str) -> u64 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub(crate) fn read_int_sysfs(path: &str) -> Option<i32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iw_parsers_handle_colon_ssids() {
        let link = "Connected to aa:bb:cc:dd:ee:ff\nSSID: odd:name:here\nfreq: 5745\nsignal: -55 dBm\ntx bitrate: 866.7 MBit/s";
        assert_eq!(iw_ssid(link), "odd:name:here");
        assert_eq!(iw_int(link, "signal:"), Some(-55));
        assert_eq!(iw_float(link, "freq:"), Some(5745.0));
        assert_eq!(iw_bitrate(link), ("866.7".into(), "MBit/s".into()));
        assert_eq!(iw_ssid("no ssid here"), "");
        assert_eq!(iw_int(link, "missing:"), None);
    }
}
