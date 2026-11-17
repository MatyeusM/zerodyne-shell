//! Wire types for the network snapshot and the intermediate route/addr rows.

use serde::{Deserialize, Serialize};

fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}

/// The single network status shape. Fields without `omitempty` in Go stay
/// present in JSON (as `null` when unknown).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Status {
    pub iface: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ip: String,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub prefix: i32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub gateway: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ssid: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_pct: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_dbm: Option<i32>,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub freq_mhz: i32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bitrate: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mb: Option<i32>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub duplex: String,

    pub router_ping_ms: Option<f64>,
    pub internet_ping_ms: Option<f64>,

    pub download_bps: f64,
    pub upload_bps: f64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub router_ping_avg_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internet_ping_avg_ms: Option<f64>,
    pub internet_loss_pct: i32,
    pub ping_samples: i32,
}

#[derive(Default)]
pub(super) struct RouteInfo {
    pub(super) dev: String,
    pub(super) gw: String,
    pub(super) src: String,
    pub(super) prefix: i32,
}

#[derive(Deserialize, Default)]
pub(super) struct RouteRow {
    #[serde(default)]
    pub(super) dev: String,
    #[serde(default)]
    pub(super) gateway: String,
    #[serde(default)]
    pub(super) prefsrc: String,
}

#[derive(Deserialize)]
pub(super) struct AddrIface {
    #[serde(default, rename = "addr_info")]
    pub(super) addr_info: Vec<AddrInfo>,
}

#[derive(Deserialize)]
pub(super) struct AddrInfo {
    #[serde(default)]
    pub(super) family: String,
    #[serde(default)]
    pub(super) prefixlen: i32,
}
