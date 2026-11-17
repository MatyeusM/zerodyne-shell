//! The `audio-output-switch` handler: cycle available output sinks.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};

use super::common::{apply_output_default, collector, step};
use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::monitor::audio::{SinkEntry, resolve_volume_sink, sink_muted, sink_volume_pct};
use crate::runner::{Ctx, Runner};

pub struct OutputSwitchHandler {
    run: Arc<dyn Runner>,
}

impl OutputSwitchHandler {
    pub fn new() -> Self {
        Self {
            run: super::common::live(),
        }
    }

    pub fn with_runner(run: Arc<dyn Runner>) -> Self {
        Self { run }
    }
}

impl Default for OutputSwitchHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn data(v: Value) -> Map<String, Value> {
    v.as_object().expect("object literal").clone()
}

impl HandlerTrait for OutputSwitchHandler {
    fn name(&self) -> &'static str {
        "audio-output-switch"
    }

    fn usage(&self) -> &'static str {
        "audio-output-switch [next|previous]"
    }

    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        let mut dir = "next";
        if args.len() > 1 || (args.len() == 1 && args[0] != "next" && args[0] != "previous") {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        if args.len() == 1 {
            dir = &args[0];
        }
        let sctx = step(ctx, 10);
        let c = collector(self.run.clone());
        let (sinks, def_name) = c.list_sinks(&sctx)?;
        let avail: Vec<SinkEntry> = sinks.into_iter().filter(|s| s.available).collect();
        if avail.is_empty() {
            return Err(anyhow!("audio-output-switch: no audio devices found"));
        }
        let idx = avail
            .iter()
            .position(|s| s.info.name == def_name)
            .unwrap_or(0);
        let next = if dir == "next" {
            (idx + 1) % avail.len()
        } else {
            (idx + avail.len() - 1) % avail.len()
        };
        let target = &avail[next];
        if target.info.name != def_name {
            apply_output_default(
                &sctx,
                self.run.as_ref(),
                &target.index.to_string(),
                &target.info.name,
            );
        }
        // Read the level from whichever sink actually carries it (a tuning
        // sink sits at fixed 100% while loudness lives beneath).
        let eff = resolve_volume_sink(&sctx, self.run.as_ref(), &target.info.name);
        let vol = sink_volume_pct(&sctx, self.run.as_ref(), &eff).unwrap_or(0);
        let muted = sink_muted(&sctx, self.run.as_ref(), &eff).unwrap_or(false);
        Ok(ResultData {
            message: Some(target.info.description.clone()),
            data: Some(data(json!({
                "name": target.info.name,
                "description": target.info.description,
                "volume_pct": vol,
                "muted": muted,
            }))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::common::{AudioCall, audio_sequence};
    use super::*;
    use crate::runner::FnRunner;
    use std::sync::Mutex;

    const SWITCH_SINKS_JSON: &str = r#"[
      {"index":10,"name":"alsa_output.pci.x","description":"Built-in Audio","mute":false,
       "volume":{"front-left":{"value_percent":"50%"}},"ports":[],
       "properties":{}},
      {"index":20,"name":"bluez_output.y","description":"Headphones","mute":true,
       "volume":{"front-left":{"value_percent":"30%"}},"ports":[{"availability":"available"}],
       "properties":{"device.bus":"bluetooth"}},
      {"index":30,"name":"hdmi_output.z","description":"HDMI","mute":false,
       "volume":{"front-left":{"value_percent":"90%"}},"ports":[{"availability":"not available"}],
       "properties":{}}
    ]"#;

    fn stub_switch(def: &str, calls: Option<Arc<Mutex<Vec<AudioCall>>>>) -> Arc<dyn Runner> {
        let calls = calls.unwrap_or_default();
        let def = def.to_string();
        Arc::new(FnRunner(move |_ctx, name, args| {
            calls.lock().unwrap().push(AudioCall {
                name: name.into(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("pactl get-default-sink") {
                return Ok(def.clone());
            }
            if key.starts_with("pactl -f json list sinks") {
                return Ok(SWITCH_SINKS_JSON.into());
            }
            if key.starts_with("pactl list sink-inputs") {
                return Ok(
                    "Sink Input #1\n\tProperties:\n\t\tapplication.name = \"Firefox\"\n".into(),
                );
            }
            if key.starts_with("pactl get-sink-volume") {
                return Ok("Volume: front-left: 1 /  30% / -25 dB".into());
            }
            if key.starts_with("pactl get-sink-mute") {
                return Ok("Mute: yes".into());
            }
            Ok(String::new())
        }))
    }

    #[test]
    fn switch_next() {
        let calls = Arc::new(Mutex::new(Vec::<AudioCall>::new()));
        let res =
            OutputSwitchHandler::with_runner(stub_switch("alsa_output.pci.x", Some(calls.clone())))
                .exec(&Ctx::new(), &[])
                .unwrap();
        let d = res.data.unwrap();
        assert_eq!(d["name"], "bluez_output.y");
        assert_eq!(d["muted"], true);
        assert_eq!(d["volume_pct"], 30);
        let seq = audio_sequence(&calls.lock().unwrap());
        assert!(seq.contains("pactl set-default-sink bluez_output.y"));
        assert!(seq.contains("wpctl set-default 20"));
        assert!(seq.contains("pactl move-sink-input 1 bluez_output.y"));
    }

    #[test]
    fn switch_previous_wraps() {
        let res = OutputSwitchHandler::with_runner(stub_switch("alsa_output.pci.x", None))
            .exec(&Ctx::new(), &["previous".into()])
            .unwrap();
        assert_eq!(res.data.unwrap()["name"], "bluez_output.y");
    }

    #[test]
    fn switch_same_sink_no_set() {
        let calls = Arc::new(Mutex::new(Vec::<AudioCall>::new()));
        let base = stub_switch("alsa_output.pci.x", Some(calls.clone()));
        let one = r#"[{"index":10,"name":"alsa_output.pci.x","description":"Built-in","mute":false,
  "volume":{"front-left":{"value_percent":"50%"}},"ports":[],"properties":{}}]"#;
        let wrapped: Arc<dyn Runner> = Arc::new(FnRunner(move |ctx, name, args| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("pactl -f json list sinks") {
                return Ok(one.into());
            }
            base.run(ctx, name, args)
        }));
        let res = OutputSwitchHandler::with_runner(wrapped)
            .exec(&Ctx::new(), &[])
            .unwrap();
        assert_eq!(res.data.unwrap()["name"], "alsa_output.pci.x");
        assert!(!audio_sequence(&calls.lock().unwrap()).contains("set-default-sink"));
    }

    #[test]
    fn switch_usage() {
        for args in [vec!["next", "extra"], vec!["sideways"]] {
            let a: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(
                OutputSwitchHandler::with_runner(stub_switch("", None))
                    .exec(&Ctx::new(), &a)
                    .is_err()
            );
        }
    }
}
