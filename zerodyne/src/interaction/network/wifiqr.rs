//! The `wifi-qr` interaction handler: share card payload + qrencode matrix.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::json;

use super::common::{data, first_wifi_device, is_wireless_dir, live, route_device, step};
use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::runner::{Ctx, Runner};

pub struct QrHandler {
    run: Arc<dyn Runner>,
}

impl QrHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }

    fn secrets(&self, ctx: &Ctx, uuid: &str) -> Result<QrFields> {
        let out = self
            .run
            .run(
                &step(ctx, 5),
                "nmcli",
                &[
                    "--show-secrets",
                    "--escape",
                    "no",
                    "--get-values",
                    "802-11-wireless.ssid,802-11-wireless-security.key-mgmt,802-11-wireless-security.psk,802-11-wireless.hidden,802-11-wireless-security.wep-key0",
                    "connection",
                    "show",
                    "uuid",
                    uuid,
                ],
            )
            .map_err(|_| anyhow!("wifi-qr: cannot read connection secrets"))?;
        let lines: Vec<&str> = out.split('\n').collect();
        let get = |i: usize| {
            lines
                .get(i)
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        Ok(QrFields {
            ssid: get(0),
            key_mgmt: get(1),
            password: get(2),
            hidden: get(3),
            wep_key: get(4),
        })
    }
}

impl Default for QrHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for QrHandler {
    fn name(&self) -> &'static str {
        "wifi-qr"
    }

    fn usage(&self) -> &'static str {
        "wifi-qr"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if !args.is_empty() {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        if !crate::caps::available("qrencode") {
            return Err(anyhow!("wifi-qr: qrencode is not installed"));
        }
        let mut dev = route_device(ctx, self.run.as_ref());
        if dev.is_empty() || !is_wireless_dir(&dev) {
            dev = first_wifi_device(ctx, self.run.as_ref());
        }
        if dev.is_empty() {
            return Err(anyhow!("wifi-qr: no active wifi connection"));
        }
        let uuid = self
            .run
            .run(
                ctx,
                "nmcli",
                &["--get-values", "GENERAL.CON-UUID", "device", "show", &dev],
            )
            .map(|out| out.trim().to_string())
            .unwrap_or_default();
        if uuid.is_empty() || uuid == "--" {
            return Err(anyhow!("wifi-qr: no active wifi connection"));
        }
        let f = self.secrets(ctx, &uuid)?;
        let qr = build_qr(ctx, self.run.as_ref(), &dev, &f)?;
        // Never log `f.password` / `qr.password`: the PSK is a secret.
        Ok(ResultData::with_data(data(json!({
            "iface": qr.iface,
            "ssid": qr.ssid,
            "security": qr.security,
            "password": qr.password,
            "size": qr.size,
            "rows": qr.rows,
        }))))
    }
}

/// The share card for the active Wi-Fi connection.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct QrData {
    pub iface: String,
    pub ssid: String,
    /// WPA | WEP | nopass
    pub security: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub password: String,
    pub size: usize,
    /// "0"/"1" per module, row-major.
    pub rows: Vec<String>,
}

struct QrFields {
    ssid: String,
    key_mgmt: String,
    password: String,
    hidden: String,
    wep_key: String,
}

/// Classify security, encode the WIFI: payload, and collapse qrencode's
/// two-characters-per-module ASCII into 0/1 rows.
fn build_qr(ctx: &Ctx, run: &dyn Runner, dev: &str, f: &QrFields) -> Result<QrData> {
    if f.ssid.is_empty() {
        return Err(anyhow!("wifi-qr: cannot read the wifi name"));
    }
    if f.key_mgmt.contains("eap") || f.key_mgmt.contains("ieee8021x") {
        return Err(anyhow!(
            "wifi-qr: enterprise wifi cannot be shared with a password qr code"
        ));
    }
    let mut qr = QrData {
        iface: dev.to_string(),
        ssid: f.ssid.clone(),
        security: String::new(),
        password: String::new(),
        size: 0,
        rows: Vec::new(),
    };
    if !f.key_mgmt.is_empty() && f.key_mgmt != "none" {
        if f.password.is_empty() {
            return Err(anyhow!("wifi-qr: cannot read the wifi password"));
        }
        qr.security = "WPA".to_string();
        qr.password = f.password.clone();
    } else if !f.wep_key.is_empty() {
        qr.security = "WEP".to_string();
        qr.password = f.wep_key.clone();
    } else {
        qr.security = "nopass".to_string();
    }
    let mut payload = format!(
        "WIFI:T:{};S:{};P:{};",
        qr.security,
        escape_qr_value(&f.ssid),
        escape_qr_value(&qr.password)
    );
    if f.hidden == "yes" {
        payload.push_str("H:true;");
    }
    payload.push(';');
    let ascii = run
        .run(
            &step(ctx, 15),
            "qrencode",
            &[
                "--type", "ASCII", "--margin", "4", "--output", "-", &payload,
            ],
        )
        .map_err(|_| anyhow!("wifi-qr: qrencode failed"))?;
    qr.rows = collapse_qr_matrix(&ascii);
    if qr.rows.is_empty() {
        return Err(anyhow!("wifi-qr: qrencode produced no matrix"));
    }
    qr.size = qr.rows.len();
    Ok(qr)
}

/// Escape the WIFI: value separators (order matters: backslash first).
pub fn escape_qr_value(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace(':', "\\:")
}

/// Collapse two-characters-per-module ASCII rows into 0/1 strings: a pair
/// counts as set when it contains '#'.
pub fn collapse_qr_matrix(ascii: &str) -> Vec<String> {
    let mut rows = Vec::new();
    for line in ascii.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        let bytes = line.as_bytes();
        let mut row = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let end = (i + 2).min(bytes.len());
            row.push(if bytes[i..end].contains(&b'#') {
                '1'
            } else {
                '0'
            });
            i += 2;
        }
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    #[test]
    fn escape_qr_value_test() {
        assert_eq!(escape_qr_value("a;b,c:d\\e"), "a\\;b\\,c\\:d\\\\e");
    }

    #[test]
    fn collapse_qr_matrix_test() {
        let rows = collapse_qr_matrix("##  \n  ##\n\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], "10");
        assert_eq!(rows[1], "01");
    }

    #[test]
    fn build_qr_classifies_and_encodes() {
        let payload = Arc::new(Mutex::new(String::new()));
        let p2 = payload.clone();
        let run: Arc<dyn Runner> = Arc::new(FnRunner(move |_ctx, _name, args| {
            *p2.lock().unwrap() = args.last().unwrap().to_string();
            Ok("##\n##\n".into())
        }));
        let fields = QrFields {
            ssid: "Ho;me".into(),
            key_mgmt: "wpa-psk".into(),
            password: "pw".into(),
            hidden: "no".into(),
            wep_key: String::new(),
        };
        let qr = build_qr(&Ctx::new(), run.as_ref(), "wlan0", &fields).unwrap();
        let p = payload.lock().unwrap();
        assert!(
            p.starts_with("WIFI:T:WPA;S:Ho\\;me;P:pw;"),
            "payload {:?}",
            p
        );
        assert!(p.ends_with(";;"), "payload {:?}", p);
        assert_eq!(qr.security, "WPA");
        assert_eq!(qr.password, "pw");
        assert_eq!(qr.size, 2);
    }

    #[test]
    fn build_qr_rejects_enterprise() {
        let run: Arc<dyn Runner> = Arc::new(FnRunner(|_ctx, _name, _args| Ok(String::new())));
        let fields = QrFields {
            ssid: "Corp".into(),
            key_mgmt: "wpa-eap".into(),
            password: String::new(),
            hidden: String::new(),
            wep_key: String::new(),
        };
        let err = build_qr(&Ctx::new(), run.as_ref(), "wlan0", &fields).unwrap_err();
        assert!(err.to_string().contains("enterprise"));
    }
}
