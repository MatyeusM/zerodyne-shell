//! `power-profile-set` handler, ported from
//! `internal/interaction/powerprofile/powerprofile.go`.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};

use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::monitor::powerprofile::{KNOWN_PROFILES, valid_profile};
use crate::runner::{CombinedRunner, Ctx, Runner};

pub struct SetHandler {
    run: Arc<dyn Runner>,
}

impl SetHandler {
    pub fn new() -> Self {
        Self {
            run: Arc::new(CombinedRunner::default()),
        }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for SetHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn first_line(s: &str) -> String {
    s.split('\n').next().unwrap_or("").trim().to_string()
}

impl HandlerTrait for SetHandler {
    fn name(&self) -> &'static str {
        "power-profile-set"
    }

    fn usage(&self) -> &'static str {
        "power-profile-set <profile>"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 1 || !valid_profile(&args[0]) {
            return Err(anyhow!(
                "usage: {} (known: {})",
                self.usage(),
                KNOWN_PROFILES.join(", ")
            ));
        }
        let sctx = ctx.with_timeout(Duration::from_secs(10));
        match self.run.run(&sctx, "powerprofilesctl", &["set", &args[0]]) {
            Ok(_) => Ok(ResultData::message(args[0].clone())),
            Err(out) => Err(anyhow!("power-profile-set: {}", first_line(&out))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    #[test]
    fn set_usage() {
        let run: Arc<dyn Runner> = Arc::new(FnRunner(|_ctx, _name, _args| Ok(String::new())));
        for args in [
            vec![],
            vec!["balanced", "extra"],
            vec!["turbo"],
            vec!["Performance"],
        ] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(
                SetHandler::with_runner(run.clone())
                    .exec(&Ctx::new(), &a)
                    .is_err()
            );
        }
    }

    #[test]
    fn set_command() {
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let c2 = calls.clone();
        let run: Arc<dyn Runner> = Arc::new(FnRunner(move |_ctx, name, args| {
            c2.lock()
                .unwrap()
                .push(format!("{} {}", name, args.join(" ")));
            Ok(String::new())
        }));
        let res = SetHandler::with_runner(run)
            .exec(&Ctx::new(), &["power-saver".into()])
            .unwrap();
        assert_eq!(res.message.as_deref(), Some("power-saver"));
        let c = calls.lock().unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0], "powerprofilesctl set power-saver");
    }
}
