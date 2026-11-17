//! Session power handler, ported from `internal/interaction/power/power.go`.
//!
//! All four operations are fire-and-forget: the session ends (or the screen
//! locks), so there is no state for QML to re-poll. Only usage errors and
//! command failures are reported.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};

use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::runner::{CombinedRunner, Ctx, Runner};

pub struct Handler {
    run: Arc<dyn Runner>,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            run: Arc::new(CombinedRunner::default()),
        }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for Handler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for Handler {
    fn name(&self) -> &'static str {
        "power"
    }

    fn usage(&self) -> &'static str {
        "power <logout|lock|restart|shutdown>"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 1 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let sctx = ctx.with_timeout(Duration::from_secs(10));
        let (cmd, args_for, msg): (&str, &[&str], &str) = match args[0].as_str() {
            "logout" => ("hyprctl", &["dispatch", "hl.dsp.exit()"], "logging out"),
            "lock" => ("hyprlock", &[], "locked"),
            "restart" => ("systemctl", &["reboot"], "restarting"),
            "shutdown" => ("systemctl", &["poweroff"], "shutting down"),
            _ => return Err(anyhow!("usage: {}", self.usage())),
        };
        self.run
            .run(&sctx, cmd, args_for)
            .map_err(|e| anyhow!("power: {}", e))?;
        Ok(ResultData::message(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq)]
    struct PowerCall {
        name: String,
        args: Vec<String>,
    }

    fn stub_power(calls: Option<Arc<Mutex<Vec<PowerCall>>>>) -> Arc<dyn Runner> {
        let calls = calls.unwrap_or_default();
        Arc::new(FnRunner(move |_ctx, name, args| {
            calls.lock().unwrap().push(PowerCall {
                name: name.into(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            Ok(String::new())
        }))
    }

    #[test]
    fn power_usage() {
        for args in [
            vec![],
            vec!["logout", "extra"],
            vec!["suspend"],
            vec!["bogus"],
        ] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(
                Handler::with_runner(stub_power(None))
                    .exec(&Ctx::new(), &a)
                    .is_err()
            );
        }
    }

    #[test]
    fn power_commands() {
        let cases = [
            ("logout", "hyprctl dispatch hl.dsp.exit()"),
            ("lock", "hyprlock"),
            ("restart", "systemctl reboot"),
            ("shutdown", "systemctl poweroff"),
        ];
        for (action, want) in cases {
            let calls = Arc::new(Mutex::new(Vec::<PowerCall>::new()));
            let res = Handler::with_runner(stub_power(Some(calls.clone())))
                .exec(&Ctx::new(), &[action.into()])
                .unwrap();
            assert!(!res.message.unwrap().is_empty());
            let c = calls.lock().unwrap();
            assert_eq!(c.len(), 1);
            let got = format!("{} {}", c[0].name, c[0].args.join(" "));
            assert_eq!(got.trim(), want);
        }
    }
}
