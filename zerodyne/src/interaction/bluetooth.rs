//! Bluetooth interaction handlers, ported from `internal/interaction/bluetooth`.
//!
//! Device operations are best-effort (the UI polls BlueZ state afterwards),
//! so only usage errors fail. Power operations still report real failures.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::runner::{CombinedRunner, Ctx, Runner};

fn live() -> Arc<dyn Runner> {
    Arc::new(CombinedRunner {
        timeout: Duration::from_secs(60),
    })
}

/// Matches the old `^([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}$` pattern.
fn is_mac_address(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 17
        && bytes.iter().enumerate().all(|(i, b)| match i % 3 {
            0 | 1 => b.is_ascii_hexdigit(),
            _ => *b == b':',
        })
}

fn step(ctx: &Ctx, secs: u64) -> Ctx {
    ctx.with_timeout(Duration::from_secs(secs))
}

/// `bluetooth-power [on|off|toggle|is-on]`.
pub struct PowerHandler {
    run: Arc<dyn Runner>,
}

impl PowerHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for PowerHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Every bluetooth controller's address (field 1 of `bluetoothctl list`).
fn controllers(ctx: &Ctx, run: &dyn Runner) -> Vec<String> {
    let sctx = step(ctx, 5);
    let Ok(out) = run.run(&sctx, "bluetoothctl", &["list"]) else {
        return Vec::new();
    };
    out.split('\n')
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect()
}

/// Whether any controller is up. Never fails: no controllers reads as off.
fn powered(ctx: &Ctx, run: &dyn Runner) -> Result<bool> {
    for c in controllers(ctx, run) {
        let sctx = step(ctx, 5);
        if let Ok(out) = run.run(&sctx, "bluetoothctl", &["show", &c])
            && out.contains("Powered: yes")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn wait_powered(ctx: &Ctx, run: &dyn Runner) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if powered(ctx, run).unwrap_or(false) {
            return true;
        }
        if Instant::now() >= deadline || ctx.is_expired() {
            return false;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn power_on(ctx: &Ctx, run: &dyn Runner) -> Result<()> {
    if let Err(e) = run.run(ctx, "rfkill", &["unblock", "bluetooth"]) {
        return Err(anyhow!("bluetooth-power: {}", e));
    }
    if wait_powered(ctx, run) {
        return Ok(());
    }
    let sctx = step(ctx, 5);
    let _ = run.run(&sctx, "bluetoothctl", &["power", "on"]);
    if wait_powered(ctx, run) {
        return Ok(());
    }
    Err(anyhow!("bluetooth-power: adapter did not come up"))
}

/// Power the default adapter up when needed, for device operations.
fn ensure_powered(ctx: &Ctx, run: &dyn Runner) {
    let sctx = step(ctx, 5);
    if let Ok(out) = run.run(&sctx, "bluetoothctl", &["show"])
        && out.contains("Powered: yes")
    {
        return;
    }
    let _ = power_on(ctx, run);
}

impl HandlerTrait for PowerHandler {
    fn name(&self) -> &'static str {
        "bluetooth-power"
    }

    fn usage(&self) -> &'static str {
        "bluetooth-power [on|off|toggle|is-on]"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 1 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        match args[0].as_str() {
            "on" => {
                power_on(ctx, self.run.as_ref())?;
                Ok(ResultData::message("bluetooth on"))
            }
            "off" => {
                self.run
                    .run(ctx, "rfkill", &["block", "bluetooth"])
                    .map_err(|e| anyhow!("bluetooth-power: {}", e))?;
                Ok(ResultData::message("bluetooth off"))
            }
            "toggle" => {
                if powered(ctx, self.run.as_ref())? {
                    self.run
                        .run(ctx, "rfkill", &["block", "bluetooth"])
                        .map_err(|e| anyhow!("bluetooth-power: {}", e))?;
                    Ok(ResultData::message("bluetooth off"))
                } else {
                    power_on(ctx, self.run.as_ref())?;
                    Ok(ResultData::message("bluetooth on"))
                }
            }
            "is-on" => {
                if !powered(ctx, self.run.as_ref())? {
                    return Err(anyhow!("bluetooth-power: off"));
                }
                Ok(ResultData::message("on"))
            }
            _ => Err(anyhow!("usage: {}", self.usage())),
        }
    }
}

/// `bluetooth-device [pair|connect|disconnect|forget] <address>`.
pub struct DeviceHandler {
    run: Arc<dyn Runner>,
}

impl DeviceHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for DeviceHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn best_effort(ctx: &Ctx, run: &dyn Runner, secs: u64, args: &[&str]) {
    let sctx = step(ctx, secs);
    let _ = run.run(&sctx, "bluetoothctl", args);
}

impl HandlerTrait for DeviceHandler {
    fn name(&self) -> &'static str {
        "bluetooth-device"
    }

    fn usage(&self) -> &'static str {
        "bluetooth-device [pair|connect|disconnect|forget] <address>"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 2 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let (action, address) = (args[0].as_str(), args[1].as_str());
        if !matches!(action, "pair" | "connect" | "disconnect" | "forget") {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        if !is_mac_address(address) {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let run = self.run.as_ref();
        match action {
            "pair" => {
                ensure_powered(ctx, run);
                best_effort(ctx, run, 20, &["pair", address]);
                best_effort(ctx, run, 5, &["trust", address]);
                best_effort(ctx, run, 20, &["connect", address]);
            }
            "connect" => {
                ensure_powered(ctx, run);
                best_effort(ctx, run, 5, &["trust", address]);
                best_effort(ctx, run, 20, &["connect", address]);
            }
            "disconnect" => {
                best_effort(ctx, run, 10, &["disconnect", address]);
            }
            "forget" => {
                ensure_powered(ctx, run);
                best_effort(ctx, run, 10, &["disconnect", address]);
                best_effort(ctx, run, 10, &["remove", address]);
            }
            _ => unreachable!(),
        }
        Ok(ResultData::message(format!("{} {}", action, address)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq)]
    struct BtCall {
        name: String,
        args: Vec<String>,
    }

    type Calls = Arc<Mutex<Vec<BtCall>>>;

    fn bt_sequence(calls: &[BtCall]) -> String {
        calls
            .iter()
            .map(|c| format!("{} {}", c.name, c.args.join(" ")))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn stub_bt(is_powered: bool, calls: Option<Calls>) -> Arc<dyn Runner> {
        let calls = calls.unwrap_or_default();
        Arc::new(FnRunner(move |_ctx, name, args| {
            calls.lock().unwrap().push(BtCall {
                name: name.into(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            if name == "rfkill" {
                return Ok(String::new());
            }
            match args.first().copied() {
                Some("list") => {
                    Ok("Controller AA:BB:CC:DD:EE:FF MATTHIASDESKTOP [default]\n".into())
                }
                Some("show") => {
                    if is_powered {
                        Ok("Controller AA:BB:CC:DD:EE:FF\n\tPowered: yes\n".into())
                    } else {
                        Ok("Controller AA:BB:CC:DD:EE:FF\n\tPowered: no\n".into())
                    }
                }
                _ => Ok(String::new()),
            }
        }))
    }

    const TEST_ADDR: &str = "44:73:D6:A4:73:3F";

    #[test]
    fn power_usage() {
        for args in [vec![], vec!["on", "extra"], vec!["bogus"]] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(
                PowerHandler::with_runner(stub_bt(true, None))
                    .exec(&Ctx::new(), &a)
                    .is_err()
            );
        }
    }

    #[test]
    fn power_on_unblocks() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let res = PowerHandler::with_runner(stub_bt(true, Some(calls.clone())))
            .exec(&Ctx::new(), &["on".into()])
            .unwrap();
        assert!(!res.message.unwrap().is_empty());
        let c = calls.lock().unwrap();
        assert!(!c.is_empty());
        assert_eq!(c[0].name, "rfkill");
        assert_eq!(c[0].args.join(" "), "unblock bluetooth");
    }

    #[test]
    fn power_off_blocks() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        PowerHandler::with_runner(stub_bt(true, Some(calls.clone())))
            .exec(&Ctx::new(), &["off".into()])
            .unwrap();
        let seq = bt_sequence(&calls.lock().unwrap());
        assert!(seq.contains("rfkill block bluetooth"), "got:\n{}", seq);
    }

    #[test]
    fn power_toggle() {
        let blocked = Arc::new(Mutex::new(false));
        let b2 = blocked.clone();
        let run: Arc<dyn Runner> = Arc::new(FnRunner(move |_ctx, name, args| {
            if name == "rfkill" {
                *b2.lock().unwrap() = args.join(" ") == "block bluetooth";
                return Ok(String::new());
            }
            if name == "bluetoothctl" && args.first().copied() == Some("list") {
                return Ok("Controller AA:BB:CC:DD:EE:FF MATTHIASDESKTOP [default]\n".into());
            }
            if name == "bluetoothctl" && args.first().copied() == Some("show") {
                return Ok(if *b2.lock().unwrap() {
                    "Powered: no\n"
                } else {
                    "Powered: yes\n"
                }
                .into());
            }
            Ok(String::new())
        }));
        PowerHandler::with_runner(run.clone())
            .exec(&Ctx::new(), &["toggle".into()])
            .unwrap();
        assert!(*blocked.lock().unwrap(), "toggle when on should block");
        PowerHandler::with_runner(run)
            .exec(&Ctx::new(), &["toggle".into()])
            .unwrap();
        assert!(!*blocked.lock().unwrap(), "toggle when off should unblock");
    }

    #[test]
    fn power_is_on() {
        let res = PowerHandler::with_runner(stub_bt(true, None))
            .exec(&Ctx::new(), &["is-on".into()])
            .unwrap();
        assert_eq!(res.message.as_deref(), Some("on"));
        assert!(
            PowerHandler::with_runner(stub_bt(false, None))
                .exec(&Ctx::new(), &["is-on".into()])
                .is_err()
        );
    }

    #[test]
    fn device_usage() {
        let run = stub_bt(true, None);
        for args in [
            vec![],
            vec!["connect"],
            vec!["connect", TEST_ADDR, "extra"],
            vec!["explode", TEST_ADDR],
            vec!["connect", "not-a-mac"],
            vec!["connect", "44:73:D6:A4:73:3Z"],
        ] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            let err = DeviceHandler::with_runner(run.clone())
                .exec(&Ctx::new(), &a)
                .unwrap_err();
            assert!(err.to_string().contains("usage:"), "for {:?}: {}", a, err);
        }
    }

    #[test]
    fn device_pair_sequence() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let res = DeviceHandler::with_runner(stub_bt(true, Some(calls.clone())))
            .exec(&Ctx::new(), &["pair".into(), TEST_ADDR.into()])
            .unwrap();
        assert!(!res.message.unwrap().is_empty());
        let seq = bt_sequence(&calls.lock().unwrap());
        for want in [
            "bluetoothctl show".to_string(),
            "bluetoothctl pair ".to_string() + TEST_ADDR,
            "bluetoothctl trust ".to_string() + TEST_ADDR,
            "bluetoothctl connect ".to_string() + TEST_ADDR,
        ] {
            assert!(seq.contains(&want), "missing {:?}:\n{}", want, seq);
        }
        assert!(seq.find("bluetoothctl pair").unwrap() < seq.find("bluetoothctl connect").unwrap());
    }

    #[test]
    fn device_connect_skips_pair() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        DeviceHandler::with_runner(stub_bt(true, Some(calls.clone())))
            .exec(&Ctx::new(), &["connect".into(), TEST_ADDR.into()])
            .unwrap();
        let seq = bt_sequence(&calls.lock().unwrap());
        assert!(!seq.contains("bluetoothctl pair"));
        assert!(seq.contains(&format!("bluetoothctl trust {}", TEST_ADDR)));
        assert!(seq.contains(&format!("bluetoothctl connect {}", TEST_ADDR)));
    }

    #[test]
    fn device_disconnect_alone() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        DeviceHandler::with_runner(stub_bt(false, Some(calls.clone())))
            .exec(&Ctx::new(), &["disconnect".into(), TEST_ADDR.into()])
            .unwrap();
        let c = calls.lock().unwrap();
        assert_eq!(c.len(), 1, "got:\n{}", bt_sequence(&c));
        assert_eq!(
            bt_sequence(&c),
            format!("bluetoothctl disconnect {}", TEST_ADDR)
        );
    }

    #[test]
    fn device_forget_disconnects_then_removes() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        DeviceHandler::with_runner(stub_bt(true, Some(calls.clone())))
            .exec(&Ctx::new(), &["forget".into(), TEST_ADDR.into()])
            .unwrap();
        let seq = bt_sequence(&calls.lock().unwrap());
        let disc = seq
            .find(&format!("bluetoothctl disconnect {}", TEST_ADDR))
            .unwrap();
        let rem = seq
            .find(&format!("bluetoothctl remove {}", TEST_ADDR))
            .unwrap();
        assert!(disc < rem, "got:\n{}", seq);
    }
}
