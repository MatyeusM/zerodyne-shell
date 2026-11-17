//! Wi-Fi interaction handlers, ported from the Go daemon's
//! `internal/interaction/network` package.
//!
//! The public surface is re-exported here so callers keep using
//! `crate::interaction::network::ConnectHandler` and friends unchanged.

mod common;
mod scan;
mod wifiband;
mod wificonn;
mod wifiqr;

pub use scan::ScanHandler;
pub use wifiband::Handler;
pub use wificonn::{ConnectHandler, DisconnectHandler, ForgetHandler, NEED_PASSWORD};
pub use wifiqr::{QrData, QrHandler, collapse_qr_matrix, escape_qr_value};
