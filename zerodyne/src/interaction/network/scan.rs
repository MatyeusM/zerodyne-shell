//! The `wifi-scan` interaction handler: forced synchronous visibility refresh.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::json;

use super::common::{data, live};
use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::monitor::network::{scan_device, scan_visible};
use crate::runner::{Ctx, Runner};

pub struct ScanHandler {
    run: Arc<dyn Runner>,
}

impl ScanHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for ScanHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for ScanHandler {
    fn name(&self) -> &'static str {
        "wifi-scan"
    }

    fn usage(&self) -> &'static str {
        "wifi-scan [rescan]"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() > 1 || (args.len() == 1 && args[0] != "rescan") {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let dev = scan_device(ctx, self.run.as_ref());
        if dev.is_empty() {
            return Err(anyhow!("wifi-scan: no wifi device"));
        }
        let nets = scan_visible(ctx, self.run.as_ref(), &dev, args.len() == 1)
            .map_err(|e| anyhow!("wifi-scan: {}", e))?;
        Ok(ResultData::with_data(data(json!({
            "device": dev,
            "networks": nets,
        }))))
    }
}

#[cfg(test)]
mod tests {
    use super::super::common::stub;
    use super::*;

    #[test]
    fn scan_dedupes_to_strongest_ap() {
        let run = stub(vec![
            (
                "nmcli -e no -t -f DEVICE,TYPE,STATE device status",
                "wlan0:wifi:connected\n",
            ),
            (
                "nmcli -e no -t -f IN-USE",
                "*:80:WPA2:AA:BB:CC:DD:EE:FF:Home\n:70:WPA2:BB:CC:DD:EE:FF:00:Home\n:60::CC:DD:EE:FF:00:11:Open\n",
            ),
            ("nmcli -e no -t -f NAME connection show", "Home\n"),
        ]);
        let res = ScanHandler::with_runner(run)
            .exec(&Ctx::new(), &[])
            .unwrap();
        let nets = res.data.unwrap()["networks"].as_array().unwrap().clone();
        assert_eq!(nets.len(), 2);
        assert_eq!(nets[0]["ssid"], "Home");
        assert_eq!(nets[0]["in_use"], true);
        assert_eq!(nets[0]["known"], true);
        assert_eq!(nets[0]["signal"], 80);
        assert_eq!(nets[1]["ssid"], "Open");
        assert_eq!(nets[1]["security"], "open");
    }

    #[test]
    fn scan_skips_empty_ssid_and_bad_signal() {
        let run = stub(vec![
            (
                "nmcli -e no -t -f DEVICE,TYPE,STATE device status",
                "wlan0:wifi:disconnected\n",
            ),
            (
                "nmcli -e no -t -f IN-USE",
                ":50:WPA2:AA:\nCorp:xx:WPA2:BB:Corp\n",
            ),
        ]);
        let res = ScanHandler::with_runner(run)
            .exec(&Ctx::new(), &[])
            .unwrap();
        let nets = res.data.unwrap()["networks"].as_array().unwrap().clone();
        assert!(nets.is_empty(), "want no networks, got {:?}", nets);
    }

    #[test]
    fn scan_rejects_bad_args() {
        let err = ScanHandler::with_runner(stub(vec![]))
            .exec(&Ctx::new(), &["a".into(), "b".into()])
            .unwrap_err();
        assert!(err.to_string().contains("usage:"));
    }
}
