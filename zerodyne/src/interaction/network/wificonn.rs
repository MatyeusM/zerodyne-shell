//! The wifi connect / disconnect / forget interaction handlers.

use std::sync::Arc;

use anyhow::{Result, anyhow};

use super::common::{first_line, live, nm_list};
use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::runner::{Ctx, Runner};

/// The exact error QML matches when the AP demands credentials.
pub const NEED_PASSWORD: &str = "need-password";

pub struct ConnectHandler {
    run: Arc<dyn Runner>,
}

impl ConnectHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for ConnectHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for ConnectHandler {
    fn name(&self) -> &'static str {
        "wifi-connect"
    }

    fn usage(&self) -> &'static str {
        "wifi-connect <ssid> [password]"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.is_empty() || args.len() > 2 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let mut cmd = vec!["dev", "wifi", "connect", args[0].as_str()];
        if args.len() == 2 {
            cmd.push("password");
            cmd.push(args[1].as_str());
        }
        match self.run.run(ctx, "nmcli", &cmd) {
            Ok(_) => Ok(ResultData::message(format!("connected to {}", args[0]))),
            Err(out) => {
                if out.contains("Secrets were required") {
                    Err(anyhow!("{}", NEED_PASSWORD))
                } else {
                    Err(anyhow!("wifi-connect: {}", first_line(&out)))
                }
            }
        }
    }
}

pub struct DisconnectHandler {
    run: Arc<dyn Runner>,
}

impl DisconnectHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for DisconnectHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for DisconnectHandler {
    fn name(&self) -> &'static str {
        "wifi-disconnect"
    }

    fn usage(&self) -> &'static str {
        "wifi-disconnect"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if !args.is_empty() {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let out = nm_list(
            ctx,
            self.run.as_ref(),
            "DEVICE,TYPE,STATE",
            &["device", "status"],
        )
        .map_err(|e| anyhow!("wifi-disconnect: {}", e))?;
        let mut dev = String::new();
        for line in out.split('\n') {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() == 3 && parts[1] == "wifi" && parts[2].starts_with("connected") {
                dev = parts[0].to_string();
                break;
            }
        }
        if dev.is_empty() {
            return Err(anyhow!("wifi-disconnect: no active wifi connection"));
        }
        match self.run.run(ctx, "nmcli", &["dev", "disconnect", &dev]) {
            Ok(_) => Ok(ResultData::message("disconnected")),
            Err(out) => Err(anyhow!("wifi-disconnect: {}", first_line(&out))),
        }
    }
}

pub struct ForgetHandler {
    run: Arc<dyn Runner>,
}

impl ForgetHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for ForgetHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for ForgetHandler {
    fn name(&self) -> &'static str {
        "wifi-forget"
    }

    fn usage(&self) -> &'static str {
        "wifi-forget <ssid>"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 1 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let out = nm_list(
            ctx,
            self.run.as_ref(),
            "NAME,802-11-wireless.ssid",
            &["connection", "show"],
        )
        .map_err(|e| anyhow!("wifi-forget: {}", e))?;
        let mut profile = String::new();
        for line in out.split('\n') {
            let (name, ssid) = match line.split_once(':') {
                Some((n, s)) => (n.trim(), s.trim()),
                None => (line.trim(), ""),
            };
            if name.is_empty() {
                continue;
            }
            if ssid == args[0] || name == args[0] {
                profile = name.to_string();
                break;
            }
        }
        if profile.is_empty() {
            return Err(anyhow!("wifi-forget: no saved network {:?}", args[0]));
        }
        match self
            .run
            .run(ctx, "nmcli", &["connection", "delete", &profile])
        {
            Ok(_) => Ok(ResultData::message(format!("forgot {}", args[0]))),
            Err(out) => Err(anyhow!("wifi-forget: {}", first_line(&out))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    fn conn_stub<F>(f: F) -> Arc<dyn Runner>
    where
        F: Fn(&str, &[&str]) -> Result<String, String> + Send + Sync + 'static,
    {
        Arc::new(FnRunner(move |_ctx, name, args| f(name, args)))
    }

    #[test]
    fn connect_maps_need_password() {
        let run =
            conn_stub(|_name, _args| Err("Error: Secrets were required, but not provided".into()));
        let err = ConnectHandler::with_runner(run)
            .exec(&Ctx::new(), &["Cafe".into()])
            .unwrap_err();
        assert_eq!(err.to_string(), NEED_PASSWORD);
    }

    #[test]
    fn connect_passes_password() {
        let got = Arc::new(Mutex::new(Vec::<String>::new()));
        let g2 = got.clone();
        let run = conn_stub(move |_name, args| {
            *g2.lock().unwrap() = args.iter().map(|s| s.to_string()).collect();
            Ok("Connection successfully activated".into())
        });
        let res = ConnectHandler::with_runner(run)
            .exec(&Ctx::new(), &["Home".into(), "s3cret".into()])
            .unwrap();
        let joined = got.lock().unwrap().join(" ");
        assert!(joined.contains("password s3cret"), "got {:?}", joined);
        assert!(!res.message.unwrap().is_empty());
    }

    #[test]
    fn disconnect_finds_device() {
        let disconnected = Arc::new(Mutex::new(String::new()));
        let d2 = disconnected.clone();
        let run = conn_stub(move |_name, args| {
            if args.first().copied() == Some("-e") {
                return Ok("eth0:ethernet:connected\nwlan0:wifi:connected\n".into());
            }
            *d2.lock().unwrap() = args.last().unwrap().to_string();
            Ok(String::new())
        });
        DisconnectHandler::with_runner(run)
            .exec(&Ctx::new(), &[])
            .unwrap();
        assert_eq!(*disconnected.lock().unwrap(), "wlan0");
    }

    #[test]
    fn disconnect_without_wifi_errors() {
        let run = conn_stub(|_name, _args| Ok("eth0:ethernet:connected\n".into()));
        assert!(
            DisconnectHandler::with_runner(run)
                .exec(&Ctx::new(), &[])
                .is_err()
        );
    }

    #[test]
    fn forget_deletes_matching_profile() {
        let deleted = Arc::new(Mutex::new(String::new()));
        let d2 = deleted.clone();
        let run = conn_stub(move |_name, args| {
            if args.join(" ").contains("802-11-wireless.ssid") {
                return Ok("Home:Home\nCafe:Coffee Shop\n".into());
            }
            *d2.lock().unwrap() = args.last().unwrap().to_string();
            Ok(String::new())
        });
        ForgetHandler::with_runner(run)
            .exec(&Ctx::new(), &["Coffee Shop".into()])
            .unwrap();
        assert_eq!(*deleted.lock().unwrap(), "Cafe");
    }

    #[test]
    fn forget_unknown_errors() {
        let run = conn_stub(|_name, _args| Ok("Home:Home\n".into()));
        assert!(
            ForgetHandler::with_runner(run)
                .exec(&Ctx::new(), &["Nope".into()])
                .is_err()
        );
    }
}
