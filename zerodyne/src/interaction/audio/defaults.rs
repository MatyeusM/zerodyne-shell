//! `audio-output-default` / `audio-input-default` handlers.

use std::sync::Arc;

use anyhow::{Result, anyhow};

use super::common::{apply_output_default, live, step};
use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::runner::{Ctx, Runner};

pub struct OutputDefaultHandler {
    run: Arc<dyn Runner>,
}

impl OutputDefaultHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for OutputDefaultHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for OutputDefaultHandler {
    fn name(&self) -> &'static str {
        "audio-output-default"
    }

    fn usage(&self) -> &'static str {
        "audio-output-default <node-id> <sink-name>"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 2 || args[0].is_empty() || args[1].is_empty() {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        apply_output_default(ctx, self.run.as_ref(), &args[0], &args[1]);
        Ok(ResultData::message(format!("output {}", args[1])))
    }
}

pub struct InputDefaultHandler {
    run: Arc<dyn Runner>,
}

impl InputDefaultHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for InputDefaultHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for InputDefaultHandler {
    fn name(&self) -> &'static str {
        "audio-input-default"
    }

    fn usage(&self) -> &'static str {
        "audio-input-default <node-id> <source-name>"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 2 || args[0].is_empty() || args[1].is_empty() {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let sctx = step(ctx, 10);
        let _ = self.run.run(&sctx, "wpctl", &["set-default", &args[0]]);
        let _ = self
            .run
            .run(&sctx, "pactl", &["set-default-source", &args[1]]);
        if let Ok(out) = self
            .run
            .run(&sctx, "pactl", &["list", "short", "source-outputs"])
        {
            for line in out.split('\n') {
                if let Some(id) = line.split_whitespace().next() {
                    let _ = self
                        .run
                        .run(&sctx, "pactl", &["move-source-output", id, &args[1]]);
                }
            }
        }
        Ok(ResultData::message(format!("input {}", args[1])))
    }
}

#[cfg(test)]
mod tests {
    use super::super::common::{AudioCall, audio_sequence};
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    #[test]
    fn output_default_moves_apps() {
        let calls = Arc::new(Mutex::new(Vec::<AudioCall>::new()));
        let c2 = calls.clone();
        let run: Arc<dyn Runner> = Arc::new(FnRunner(move |_ctx, name, args| {
            c2.lock().unwrap().push(AudioCall {
                name: name.into(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            if name == "pactl" && args.join(" ") == "list sink-inputs" {
                return Ok("Sink Input #1\n\tProperties:\n\t\tapplication.name = \"Firefox\"\nSink Input #2\n\tProperties:\n\t\tnode.name = \"chain\"\nSink Input #3\n\tProperties:\n\t\tapplication.name = \"EasyEffects\"\n".into());
            }
            Ok(String::new())
        }));
        let res = OutputDefaultHandler::with_runner(run)
            .exec(&Ctx::new(), &["21".into(), "bluez_output.y".into()])
            .unwrap();
        assert!(!res.message.unwrap().is_empty());
        let seq = audio_sequence(&calls.lock().unwrap());
        for want in [
            "wpctl set-default 21",
            "pactl set-default-sink bluez_output.y",
            "pactl move-sink-input 1 bluez_output.y",
        ] {
            assert!(seq.contains(want), "missing {:?}:\n{}", want, seq);
        }
        assert!(!seq.contains("move-sink-input 2"));
        assert!(!seq.contains("move-sink-input 3"));
    }

    #[test]
    fn output_default_usage() {
        for args in [vec![], vec!["only"], vec!["a", "b", "c"], vec!["", "x"]] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            let empty: Arc<dyn Runner> = Arc::new(FnRunner(|_ctx, _name, _args| Ok(String::new())));
            assert!(
                OutputDefaultHandler::with_runner(empty)
                    .exec(&Ctx::new(), &a)
                    .is_err()
            );
        }
    }

    #[test]
    fn input_default_moves_all() {
        let calls = Arc::new(Mutex::new(Vec::<AudioCall>::new()));
        let c2 = calls.clone();
        let run: Arc<dyn Runner> = Arc::new(FnRunner(move |_ctx, name, args| {
            c2.lock().unwrap().push(AudioCall {
                name: name.into(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            if name == "pactl" && args.join(" ") == "list short source-outputs" {
                return Ok("5\t0\talsa_input.x\tPipeWire\n6\t0\talsa_input.x\tPipeWire\n".into());
            }
            Ok(String::new())
        }));
        InputDefaultHandler::with_runner(run)
            .exec(&Ctx::new(), &["50".into(), "alsa_input.y".into()])
            .unwrap();
        let seq = audio_sequence(&calls.lock().unwrap());
        for want in [
            "wpctl set-default 50",
            "pactl set-default-source alsa_input.y",
            "pactl move-source-output 5 alsa_input.y",
            "pactl move-source-output 6 alsa_input.y",
        ] {
            assert!(seq.contains(want), "missing {:?}:\n{}", want, seq);
        }
    }
}
