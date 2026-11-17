//! `audio-volume` and `audio-input-mute` handlers (volume + mute writes).

use std::sync::Arc;

use anyhow::{Result, anyhow};

use super::common::{clamp_pct, live, parse_delta, step, volume_sink, wpctl_volume_pct};
use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::monitor::audio::sink_volume_pct;
use crate::runner::{Ctx, Runner};

const DEFAULT_SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";

pub struct VolumeHandler {
    run: Arc<dyn Runner>,
}

impl VolumeHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for VolumeHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for VolumeHandler {
    fn name(&self) -> &'static str {
        "audio-volume"
    }

    fn usage(&self) -> &'static str {
        "audio-volume <raise|lower|mute-toggle|+N|-N>"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 1 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let mut action = args[0].clone();
        if action == "raise" {
            action = "+5".to_string();
        } else if action == "lower" {
            action = "-5".to_string();
        }
        let sink = volume_sink(ctx, self.run.as_ref())?;
        if action == "mute-toggle" {
            let sctx = step(ctx, 5);
            self.run
                .run(&sctx, "pactl", &["set-sink-mute", &sink, "toggle"])
                .map_err(|e| anyhow!("audio-volume: {}", e))?;
            return Ok(ResultData::message("mute toggled"));
        }
        let Some((sign, n)) = parse_delta(&action) else {
            return Err(anyhow!("usage: {}", self.usage()));
        };
        let sctx = step(ctx, 5);
        let current = sink_volume_pct(&sctx, self.run.as_ref(), &sink)?;
        let next = clamp_pct(if sign == '-' {
            current - n
        } else {
            current + n
        });
        // Unmute-via-raise: the level moves on the real sink either way.
        self.run
            .run(&sctx, "pactl", &["set-sink-mute", &sink, "0"])
            .map_err(|e| anyhow!("audio-volume: {}", e))?;
        self.run
            .run(
                &sctx,
                "pactl",
                &["set-sink-volume", &sink, &format!("{}%", next)],
            )
            .map_err(|e| anyhow!("audio-volume: {}", e))?;
        Ok(ResultData::message(format!("{}%", next)))
    }
}

pub struct InputMuteHandler {
    run: Arc<dyn Runner>,
}

impl InputMuteHandler {
    pub fn new() -> Self {
        Self { run: live() }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for InputMuteHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerTrait for InputMuteHandler {
    fn name(&self) -> &'static str {
        "audio-input-mute"
    }

    fn usage(&self) -> &'static str {
        "audio-input-mute [toggle|on|off|+N|-N]"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() > 1 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let action = args.first().map(|s| s.as_str()).unwrap_or("toggle");
        let sctx = step(ctx, 5);
        let mute = |value: &str| -> Result<ResultData> {
            self.run
                .run(&sctx, "wpctl", &["set-mute", DEFAULT_SOURCE, value])
                .map_err(|e| anyhow!("audio-input-mute: {}", e))?;
            let msg = match value {
                "toggle" => "microphone toggled",
                "1" => "microphone muted",
                _ => "microphone on",
            };
            Ok(ResultData::message(msg))
        };
        match action {
            "toggle" => return mute("toggle"),
            "on" => return mute("1"),
            "off" => return mute("0"),
            _ => {}
        }
        let Some((sign, n)) = parse_delta(action) else {
            return Err(anyhow!("usage: {}", self.usage()));
        };
        let out = self
            .run
            .run(&sctx, "wpctl", &["get-volume", DEFAULT_SOURCE])
            .map_err(|e| anyhow!("audio-input-mute: {}", e))?;
        let current = wpctl_volume_pct(&out);
        let next = clamp_pct(if sign == '-' {
            current - n
        } else {
            current + n
        });
        self.run
            .run(
                &sctx,
                "wpctl",
                &["set-volume", DEFAULT_SOURCE, &format!("{}%", next)],
            )
            .map_err(|e| anyhow!("audio-input-mute: {}", e))?;
        Ok(ResultData::message(format!("{}%", next)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::common::{AudioCall, audio_sequence};
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    type Calls = Arc<Mutex<Vec<AudioCall>>>;

    /// Answers a 40% unmuted alsa sink by default (Go's stubAudioVolume).
    fn stub_audio_volume(vol: i32, muted: bool, calls: Option<Calls>) -> Arc<dyn Runner> {
        let calls = calls.unwrap_or_default();
        Arc::new(FnRunner(move |_ctx, name, args| {
            calls.lock().unwrap().push(AudioCall {
                name: name.into(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("pactl get-default-sink") {
                return Ok("alsa_output.pci.x".into());
            }
            if key.starts_with("pactl get-sink-volume") {
                return Ok(format!("Volume: front-left: 1 /  {}% / -25 dB", vol));
            }
            if key.starts_with("pactl get-sink-mute") {
                return Ok(if muted { "Mute: yes" } else { "Mute: no" }.into());
            }
            Ok(String::new())
        }))
    }

    #[test]
    fn volume_usage() {
        for args in [
            vec![],
            vec!["raise", "extra"],
            vec!["bogus"],
            vec!["+"],
            vec!["+x"],
        ] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(
                VolumeHandler::with_runner(stub_audio_volume(40, false, None))
                    .exec(&Ctx::new(), &a)
                    .is_err(),
                "want usage error for {:?}",
                a
            );
        }
    }

    #[test]
    fn volume_raise() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let res = VolumeHandler::with_runner(stub_audio_volume(40, false, Some(calls.clone())))
            .exec(&Ctx::new(), &["raise".into()])
            .unwrap();
        assert_eq!(res.message.as_deref(), Some("45%"));
        let seq = audio_sequence(&calls.lock().unwrap());
        let mute = seq.find("pactl set-sink-mute alsa_output.pci.x 0").unwrap();
        let set = seq
            .find("pactl set-sink-volume alsa_output.pci.x 45%")
            .unwrap();
        assert!(mute < set, "want unmute then 45%:\n{}", seq);
    }

    #[test]
    fn volume_lower_clamps() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let res = VolumeHandler::with_runner(stub_audio_volume(3, false, Some(calls.clone())))
            .exec(&Ctx::new(), &["-5".into()])
            .unwrap();
        assert_eq!(res.message.as_deref(), Some("0%"));
        assert!(
            audio_sequence(&calls.lock().unwrap()).contains("set-sink-volume alsa_output.pci.x 0%")
        );
    }

    #[test]
    fn volume_raise_clamps() {
        let res = VolumeHandler::with_runner(stub_audio_volume(99, false, None))
            .exec(&Ctx::new(), &["+10".into()])
            .unwrap();
        assert_eq!(res.message.as_deref(), Some("100%"));
    }

    #[test]
    fn volume_mute_toggle() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        VolumeHandler::with_runner(stub_audio_volume(40, false, Some(calls.clone())))
            .exec(&Ctx::new(), &["mute-toggle".into()])
            .unwrap();
        assert!(
            audio_sequence(&calls.lock().unwrap())
                .contains("pactl set-sink-mute alsa_output.pci.x toggle")
        );
    }

    #[test]
    fn volume_follows_dsp() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let inner = stub_audio_volume(40, false, Some(calls.clone()));
        let wrapped: Arc<dyn Runner> = Arc::new(FnRunner(move |ctx, name, args| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("pactl get-default-sink") {
                return Ok("tuning_sink".into());
            }
            if key.starts_with("pactl list sink-inputs") {
                return Ok("Sink Input #7\n\tSink: 43\n\tProperties:\n\t\tnode.name = \"tuning_sink_out\"\n".into());
            }
            if key.starts_with("pactl list sinks short") {
                return Ok("43\talsa_output.pci.x\tPipeWire\n".into());
            }
            inner.run(ctx, name, args)
        }));
        VolumeHandler::with_runner(wrapped)
            .exec(&Ctx::new(), &["raise".into()])
            .unwrap();
        assert!(
            audio_sequence(&calls.lock().unwrap())
                .contains("set-sink-volume alsa_output.pci.x 45%")
        );
    }

    #[test]
    fn input_mute_usage() {
        for args in [vec!["toggle", "extra"], vec!["bogus"]] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(
                InputMuteHandler::with_runner(stub_audio_volume(0, false, None))
                    .exec(&Ctx::new(), &a)
                    .is_err()
            );
        }
    }

    #[test]
    fn input_mute_forms() {
        let cases = [
            ("toggle", "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"),
            ("on", "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ 1"),
            ("off", "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ 0"),
        ];
        for (action, want) in cases {
            let calls: Calls = Arc::new(Mutex::new(Vec::new()));
            let args: Vec<String> = if action == "toggle" {
                vec![]
            } else {
                vec![action.into()]
            };
            InputMuteHandler::with_runner(stub_audio_volume(0, false, Some(calls.clone())))
                .exec(&Ctx::new(), &args)
                .unwrap();
            assert!(audio_sequence(&calls.lock().unwrap()).contains(want));
        }
    }

    #[test]
    fn input_volume_delta() {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let c2 = calls.clone();
        let run: Arc<dyn Runner> = Arc::new(FnRunner(move |_ctx, name, args| {
            c2.lock().unwrap().push(AudioCall {
                name: name.into(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            if name == "wpctl" && args.first().copied() == Some("get-volume") {
                return Ok("Volume: 0.80".into());
            }
            Ok(String::new())
        }));
        let res = InputMuteHandler::with_runner(run)
            .exec(&Ctx::new(), &["+5".into()])
            .unwrap();
        assert_eq!(res.message.as_deref(), Some("85%"));
        assert!(
            audio_sequence(&calls.lock().unwrap())
                .contains("wpctl set-volume @DEFAULT_AUDIO_SOURCE@ 85%")
        );
    }
}
