//! The `wifi-band` interaction handler: ported from network-band.sh.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::json;

use super::common::{available_bands, data, iw_freq, iw_ssid, live, nm_get, step};
use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::monitor::network::{band_for_freq, band_from_nm, nm_band_for, normalize_band_target};
use crate::runner::{Ctx, Runner};

pub struct Handler {
    run: Arc<dyn Runner>,
}

impl Handler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }

    /// Query result: current band, reachable bands, pinned band.
    fn status(&self, ctx: &Ctx) -> Result<Status> {
        let dev = self.wifi_device(ctx)?;
        if dev.is_empty() {
            return Err(anyhow!("wifi-band: no connected Wi-Fi device"));
        }
        let link = match self.run.run(ctx, "iw", &["dev", &dev, "link"]) {
            Ok(l) => l,
            Err(_) => return Err(anyhow!("wifi-band: no active Wi-Fi link")),
        };
        if !link.contains("SSID:") {
            return Err(anyhow!("wifi-band: no active Wi-Fi link"));
        }
        let ssid = iw_ssid(&link);
        let freq = iw_freq(&link);
        let band = band_for_freq(&freq).unwrap_or_default();
        let available = available_bands(ctx, self.run.as_ref(), &dev, &ssid, &band);
        let profile = self.profile(ctx, &dev).unwrap_or_default();
        let mut selected = "auto".to_string();
        if !profile.is_empty()
            && let Ok(v) = nm_get(
                ctx,
                self.run.as_ref(),
                "802-11-wireless.band",
                &["connection", "show", &profile],
            )
        {
            selected = band_from_nm(&v);
        }
        Ok(Status {
            band,
            available,
            selected,
        })
    }

    fn set_band(&self, ctx: &Ctx, target: &str) -> Result<()> {
        let dev = self.wifi_device(ctx)?;
        if dev.is_empty() {
            return Err(anyhow!("wifi-band: no connected Wi-Fi device"));
        }
        let profile = self.profile(ctx, &dev)?;
        if profile.is_empty() {
            return Err(anyhow!("wifi-band: no active Wi-Fi connection profile"));
        }
        let mut desired = String::new();
        if target != "auto" {
            let link = self
                .run
                .run(&step(ctx, 5), "iw", &["dev", &dev, "link"])
                .unwrap_or_default();
            let ssid = iw_ssid(&link);
            let cur = band_for_freq(&iw_freq(&link)).unwrap_or_default();
            if !available_bands(ctx, self.run.as_ref(), &dev, &ssid, &cur)
                .iter()
                .any(|b| b == target)
            {
                return Err(anyhow!(
                    "wifi-band: {}GHz is not available on this network",
                    target
                ));
            }
            match nm_band_for(target) {
                Some(nm) => desired = nm,
                None => return Err(anyhow!("wifi-band: invalid band {:?}", target)),
            }
        }
        let previous = nm_get(
            ctx,
            self.run.as_ref(),
            "802-11-wireless.band",
            &["connection", "show", &profile],
        )
        .unwrap_or_default();
        if previous == desired {
            return Ok(());
        }
        if let Err(e) = self.run.run(
            ctx,
            "nmcli",
            &[
                "connection",
                "modify",
                &profile,
                "802-11-wireless.band",
                &desired,
            ],
        ) {
            return Err(anyhow!("wifi-band: modify profile: {}", e));
        }
        // A band change only takes effect on reassociation. Revert rather
        // than strand the machine offline.
        if self
            .run
            .run(ctx, "nmcli", &["connection", "up", &profile])
            .is_err()
        {
            let _ = self.run.run(
                ctx,
                "nmcli",
                &[
                    "connection",
                    "modify",
                    &profile,
                    "802-11-wireless.band",
                    &previous,
                ],
            );
            let _ = self.run.run(ctx, "nmcli", &["connection", "up", &profile]);
            return Err(anyhow!(
                "wifi-band: could not connect on {}; reverted to previous band",
                target
            ));
        }
        Ok(())
    }

    fn wifi_device(&self, ctx: &Ctx) -> Result<String> {
        let out = nm_get(
            ctx,
            self.run.as_ref(),
            "DEVICE,TYPE,STATE",
            &["device", "status"],
        )
        .map_err(|e| anyhow!("{}", e))?;
        for line in out.split('\n') {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() == 3 && parts[1] == "wifi" && parts[2].starts_with("connected") {
                return Ok(parts[0].to_string());
            }
        }
        Ok(String::new())
    }

    fn profile(&self, ctx: &Ctx, dev: &str) -> Result<String> {
        nm_get(
            ctx,
            self.run.as_ref(),
            "GENERAL.CONNECTION",
            &["device", "show", dev],
        )
        .map_err(|e| anyhow!("{}", e))
    }
}

struct Status {
    band: String,
    available: Vec<String>,
    selected: String,
}

impl Default for Handler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for Handler {
    fn name(&self) -> &'static str {
        "wifi-band"
    }

    fn usage(&self) -> &'static str {
        "wifi-band [auto|2.4|5|6|<freq-mhz>]"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.is_empty() {
            let st = self.status(ctx)?;
            return Ok(ResultData::with_data(data(json!({
                "band": st.band,
                "available": st.available,
                "selected": st.selected,
            }))));
        }
        if args.len() > 1 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let Some(band) = normalize_band_target(&args[0]) else {
            return Err(anyhow!(
                "invalid band {:?}: usage: {}",
                args[0],
                self.usage()
            ));
        };
        self.set_band(ctx, &band)?;
        Ok(ResultData::message(format!("band set to {}", band)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::common::stub;
    use super::*;
    use std::sync::Mutex;

    fn wifi_up() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "nmcli -e no -g DEVICE,TYPE,STATE",
                "wlan0:wifi:connected\neth0:ethernet:disconnected",
            ),
            (
                "nmcli -e no -g GENERAL.CONNECTION device show wlan0",
                "Home",
            ),
            (
                "iw dev wlan0 link",
                "Connected to aa:bb\nSSID: Home\nfreq: 5745\nsignal: -55 dBm",
            ),
            (
                "nmcli -e no -g FREQ,SSID",
                "2412:Other\n5745:Home\n6135:Home",
            ),
            (
                "nmcli -e no -g 802-11-wireless.band connection show Home",
                "a",
            ),
        ]
    }

    #[test]
    fn query_status() {
        let h = Handler::with_runner(stub(wifi_up()));
        let res = h.exec(&Ctx::new(), &[]).unwrap();
        let d = res.data.unwrap();
        assert_eq!(d["band"], "5");
        assert_eq!(d["selected"], "5");
        let avail = d["available"].as_array().unwrap();
        assert_eq!(avail.len(), 2);
        assert_eq!(avail[0], "5");
        assert_eq!(avail[1], "6");
    }

    #[test]
    fn query_no_device() {
        let h = Handler::with_runner(stub(vec![(
            "nmcli -e no -g DEVICE,TYPE,STATE",
            "eth0:ethernet:connected",
        )]));
        assert!(h.exec(&Ctx::new(), &[]).is_err());
    }

    #[test]
    fn set_band_validation() {
        let h = Handler::with_runner(stub(wifi_up()));
        assert!(h.exec(&Ctx::new(), &["bogus".into()]).is_err());
        assert!(h.exec(&Ctx::new(), &["5".into(), "extra".into()]).is_err());
        assert!(h.exec(&Ctx::new(), &["2.4".into()]).is_err());
    }

    #[test]
    fn set_band_reverts_on_failure() {
        let modified = Arc::new(Mutex::new(Vec::<String>::new()));
        let m2 = modified.clone();
        let base = wifi_up();
        let run: Arc<dyn Runner> = Arc::new(crate::runner::FnRunner(move |_ctx, name, args| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("nmcli connection up") {
                return Err("association failed".into());
            }
            if key.starts_with("nmcli connection modify") {
                m2.lock().unwrap().push(key);
                return Ok(String::new());
            }
            for (prefix, out) in &base {
                if key.starts_with(prefix) {
                    return Ok((*out).to_string());
                }
            }
            Err(String::new())
        }));
        let h = Handler::with_runner(run);
        assert!(h.exec(&Ctx::new(), &["6".into()]).is_err());
        let m = modified.lock().unwrap();
        assert_eq!(m.len(), 2, "expected set+revert modifies: {:?}", m);
        assert!(
            m[1].ends_with(" a"),
            "revert should restore previous: {:?}",
            m
        );
    }

    #[test]
    fn band_name_usage() {
        let h = Handler::new();
        assert_eq!(h.name(), "wifi-band");
        assert!(!h.usage().is_empty());
    }
}
