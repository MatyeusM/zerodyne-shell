//! Audio monitor: the typed form of the omarchy audio scripts, served as
//! JSON. Port of `internal/monitor/audio/{audio,status,monitor}.go`.
//!
//! Pure parsers/classifiers live in [`parse`]; the pactl JSON shapes live in
//! [`pactl`]. Both are re-exported so callers keep using
//! `crate::monitor::audio::*` unchanged.

mod pactl;
mod parse;

pub use parse::*;

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

use crate::caps;
use crate::monitor::Monitor as MonitorTrait;
use crate::runner::{Ctx, OutputRunner, Runner};

/// Display description of one sink.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SinkInfo {
    pub name: String,
    pub description: String,
    #[serde(rename = "class")]
    pub class: String,
    pub bluetooth: bool,
}

/// One row of the output device list (JSON is flat, like Go's embedding).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SinkEntry {
    #[serde(flatten)]
    pub info: SinkInfo,
    pub index: i32,
    pub available: bool,
    pub is_default: bool,
    pub volume_pct: i32,
    pub muted: bool,
}

/// Display description of one source.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SourceInfo {
    pub name: String,
    pub description: String,
    #[serde(rename = "class")]
    pub class: String,
}

/// One row of the input device list.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SourceEntry {
    #[serde(flatten)]
    pub info: SourceInfo,
    pub index: i32,
    pub is_default: bool,
}

/// The single audio status shape.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Status {
    pub volume_pct: i32,
    pub muted: bool,
    pub volume_sink: String,
    pub sink: SinkInfo,
    pub sinks: Vec<SinkEntry>,
    pub source: SourceInfo,
    pub sources: Vec<SourceEntry>,
    pub input_volume_pct: i32,
    pub input_muted: bool,
}

/// Collector gathers audio state through a Runner.
pub struct Collector {
    pub run: Arc<dyn Runner>,
    pub has: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl Collector {
    pub fn new(run: Arc<dyn Runner>) -> Self {
        Self {
            run,
            has: Arc::new(caps::available),
        }
    }

    pub fn collect(&self, ctx: &Ctx) -> Result<Status> {
        if !(self.has)("pactl") {
            return Err(anyhow!("audio: pactl not available"));
        }
        let def_name = default_sink(ctx, self.run.as_ref());
        if def_name.is_empty() {
            return Err(anyhow!("audio: no default sink"));
        }
        let vol_sink = resolve_volume_sink(ctx, self.run.as_ref(), &def_name);
        let vol_pct = sink_volume_pct(ctx, self.run.as_ref(), &vol_sink)?;
        let muted = sink_muted(ctx, self.run.as_ref(), &vol_sink)?;
        let sinks = self.list_sinks_inner(ctx, &def_name)?;
        let mut st = Status {
            volume_pct: vol_pct,
            muted,
            volume_sink: vol_sink.clone(),
            sink: describe_default(&sinks, &def_name, &vol_sink),
            sinks,
            source: SourceInfo {
                name: String::new(),
                description: String::new(),
                class: String::new(),
            },
            sources: Vec::new(),
            input_volume_pct: 0,
            input_muted: false,
        };
        self.collect_input(ctx, &mut st);
        Ok(st)
    }

    /// Full sink list plus the default sink name, shared with the switcher.
    pub fn list_sinks(&self, ctx: &Ctx) -> Result<(Vec<SinkEntry>, String)> {
        let def_name = default_sink(ctx, self.run.as_ref());
        if def_name.is_empty() {
            return Err(anyhow!("audio: no default sink"));
        }
        let sinks = self.list_sinks_inner(ctx, &def_name)?;
        Ok((sinks, def_name))
    }

    fn list_sinks_inner(&self, ctx: &Ctx, def_name: &str) -> Result<Vec<SinkEntry>> {
        let out = self
            .run
            .run(ctx, "pactl", &["-f", "json", "list", "sinks"])
            .map_err(|e| anyhow!("audio: list sinks: {}", run_err(&e)))?;
        let raw: Vec<pactl::PactlSink> =
            serde_json::from_str(&out).map_err(|e| anyhow!("audio: list sinks: {}", e))?;
        let mut sinks = Vec::with_capacity(raw.len());
        for s in raw {
            let props = pactl::props(&s.properties);
            let desc = pactl::sink_description(&s, &props);
            let (class, bluetooth) = classify_sink(&s.name, &desc, &props);
            sinks.push(SinkEntry {
                info: SinkInfo {
                    name: s.name.clone(),
                    description: desc,
                    class,
                    bluetooth,
                },
                index: s.index,
                available: pactl::sink_available(&s.ports),
                is_default: s.name == def_name,
                volume_pct: pactl::sink_json_volume(&s.volume),
                muted: s.mute,
            });
        }
        Ok(sinks)
    }

    /// Source side: never fails the snapshot; a box with no input still has output.
    fn collect_input(&self, ctx: &Ctx, st: &mut Status) {
        st.sources = Vec::new();
        let def_name = default_source(ctx, self.run.as_ref());
        if let Ok(out) = self
            .run
            .run(ctx, "pactl", &["-f", "json", "list", "sources"])
            && let Ok(raw) = serde_json::from_str::<Vec<pactl::PactlSource>>(&out)
        {
            for s in raw {
                let props = pactl::props(&s.properties);
                let desc = if !s.description.is_empty() {
                    s.description.clone()
                } else {
                    props
                        .get("device.description")
                        .filter(|d| !d.is_empty())
                        .cloned()
                        .unwrap_or_else(|| s.name.clone())
                };
                let is_default = !def_name.is_empty() && s.name == def_name;
                st.sources.push(SourceEntry {
                    info: SourceInfo {
                        name: s.name.clone(),
                        description: desc.clone(),
                        class: classify_source(&s.name, &desc, &props),
                    },
                    index: s.index,
                    is_default,
                });
            }
        }
        if let Some(s) = st.sources.iter().find(|s| s.is_default) {
            st.source = s.info.clone();
        } else if let Some(s) = st.sources.first() {
            st.source = s.info.clone();
        }
        if (self.has)("wpctl")
            && let Ok(out) = self
                .run
                .run(ctx, "wpctl", &["get-volume", "@DEFAULT_AUDIO_SOURCE@"])
        {
            let (pct, muted) = parse_wpctl_volume(&out);
            st.input_volume_pct = pct;
            st.input_muted = muted;
        }
    }
}

fn run_err(text: &str) -> String {
    if text.is_empty() {
        "command failed".to_string()
    } else {
        text.to_string()
    }
}

/// The sink the indicator names: default, else resolved physical, else first.
fn describe_default(sinks: &[SinkEntry], def_name: &str, vol_sink: &str) -> SinkInfo {
    for s in sinks {
        if s.info.name == def_name {
            return s.info.clone();
        }
    }
    for s in sinks {
        if s.info.name == vol_sink {
            return s.info.clone();
        }
    }
    if let Some(s) = sinks.first() {
        return s.info.clone();
    }
    SinkInfo {
        name: def_name.to_string(),
        description: def_name.to_string(),
        class: CLASS_OTHER.to_string(),
        bluetooth: false,
    }
}

pub fn default_sink(ctx: &Ctx, run: &dyn Runner) -> String {
    run.run(ctx, "pactl", &["get-default-sink"])
        .map(|out| out.trim().to_string())
        .unwrap_or_default()
}

pub fn default_source(ctx: &Ctx, run: &dyn Runner) -> String {
    run.run(ctx, "pactl", &["get-default-source"])
        .map(|out| out.trim().to_string())
        .unwrap_or_default()
}

pub fn sink_volume_pct(ctx: &Ctx, run: &dyn Runner, sink: &str) -> Result<i32> {
    let out = run
        .run(ctx, "pactl", &["get-sink-volume", sink])
        .map_err(|e| anyhow!("audio: volume for {}: {}", sink, run_err(&e)))?;
    parse_sink_volume(&out).ok_or_else(|| anyhow!("audio: could not read volume for {}", sink))
}

pub fn sink_muted(ctx: &Ctx, run: &dyn Runner, sink: &str) -> Result<bool> {
    let out = run
        .run(ctx, "pactl", &["get-sink-mute", sink])
        .map_err(|e| anyhow!("audio: mute for {}: {}", sink, run_err(&e)))?;
    Ok(out.trim().ends_with("yes"))
}

/// The registered "audio" monitor.
pub struct Monitor {
    collector: Collector,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            collector: Collector::new(Arc::new(OutputRunner::default())),
        }
    }

    pub fn new_with_collector(collector: Collector) -> Self {
        Self { collector }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorTrait for Monitor {
    fn name(&self) -> &'static str {
        "audio"
    }

    fn snapshot(&self, ctx: &Ctx) -> Result<Value> {
        let ctx = ctx.with_timeout(std::time::Duration::from_secs(8));
        Ok(serde_json::to_value(self.collector.collect(&ctx)?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::FnRunner;
    use pactl::Port;
    use std::collections::HashMap;

    fn stub_audio() -> FnRunner<impl Fn(&Ctx, &str, &[&str]) -> Result<String, String>> {
        FnRunner(|_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("pactl get-default-sink") {
                Ok("alsa_output.pci-0000_00_1f.3.analog-stereo".to_string())
            } else if key.starts_with("pactl get-sink-volume") {
                Ok("Volume: front-left: 52428 /  80% / 0.00 dB".to_string())
            } else if key.starts_with("pactl get-sink-mute") {
                Ok("Mute: no".to_string())
            } else if key.starts_with("pactl -f json list sinks") {
                Ok(STUB_SINKS.to_string())
            } else if key.starts_with("pactl get-default-source") {
                Ok("alsa_input.usb-tonor".to_string())
            } else if key.starts_with("pactl -f json list sources") {
                Ok(STUB_SOURCES.to_string())
            } else if key.starts_with("wpctl get-volume") {
                Ok("Volume: 0.80".to_string())
            } else {
                Err("no stub".into())
            }
        })
    }

    const STUB_SINKS: &str = r#"[
      {"index":115,"name":"bluez_output.44_73_D6_A4_73_3F.1","description":"Zone Vibe 100","mute":false,
       "volume":{"front-left":{"value_percent":"37%"}},"ports":[{"availability":"available"}],
       "properties":{"device.icon_name":"audio-headset-bluetooth","device.bus":"bluetooth","device.form_factor":"headset"}},
      {"index":43,"name":"alsa_output.pci-0000_00_1f.3.analog-stereo","description":"Built-in Audio","mute":false,
       "volume":{"front-left":{"value_percent":"80%"}},"ports":[{"availability":"not available"}],
       "properties":{}}
    ]"#;

    const STUB_SOURCES: &str = r#"[
      {"index":58,"name":"alsa_input.usb-tonor","description":"TONOR TC30","properties":{}}
    ]"#;

    #[test]
    fn parse_sink_volume_first_percent_token() {
        assert_eq!(
            parse_sink_volume(
                "Volume: front-left: 24253 /  37% / -25.90 dB,   front-right: 24253 /  37% / -25.90 dB\n        balance 0.00"
            ),
            Some(37)
        );
        assert_eq!(
            parse_sink_volume("Volume: front-left: 100% / 125% / ..."),
            Some(100)
        );
        assert_eq!(parse_sink_volume("Mute: no"), None);
    }

    #[test]
    fn parse_wpctl_volume_cases() {
        assert_eq!(parse_wpctl_volume("Volume: 1.00"), (100, false));
        assert_eq!(parse_wpctl_volume("Volume: 0.80 [MUTED]"), (80, true));
    }

    #[test]
    fn classify_ladder() {
        type Case<'a> = (&'a str, &'a str, Vec<(&'a str, &'a str)>, &'a str, bool);
        let cases: Vec<Case<'_>> = vec![
            (
                "bluez_output.1",
                "WH-1000XM4",
                vec![
                    ("device.icon_name", "audio-headphones-bluetooth"),
                    ("device.bus", "bluetooth"),
                ],
                CLASS_HEADPHONES,
                true,
            ),
            (
                "alsa_output.usb-headset",
                "USB Headset",
                vec![],
                CLASS_HEADSET,
                false,
            ),
            (
                "alsa_output.phone",
                "Phone Audio",
                vec![],
                CLASS_PHONE,
                false,
            ),
            (
                "alsa_output.x",
                "Studio Headphones",
                vec![],
                CLASS_HEADPHONES,
                false,
            ),
            (
                "alsa_output.hdmi-stereo",
                "Navi 31 HDMI",
                vec![],
                CLASS_HDMI,
                false,
            ),
            (
                "bluez_output.car",
                "Car Kit",
                vec![("device.bus", "bluetooth")],
                CLASS_CAR,
                true,
            ),
            (
                "alsa_output.pci.analog-stereo",
                "Built-in Audio",
                vec![],
                CLASS_SPEAKER,
                false,
            ),
            (
                "bluez_output.2",
                "Portable Speaker",
                vec![("device.bus", "bluetooth")],
                CLASS_SPEAKER,
                true,
            ),
        ];
        for (name, desc, raw, class, bt) in cases {
            let props: HashMap<String, String> = raw
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let (got, got_bt) = classify_sink(name, desc, &props);
            assert_eq!((got.as_str(), got_bt), (class, bt), "{}", name);
        }
    }

    #[test]
    fn classify_source_cases() {
        let empty = HashMap::new();
        assert_eq!(
            classify_source("alsa_input.usb-tonor", "TONOR TC30", &empty),
            CLASS_MICROPHONE
        );
        assert_eq!(
            classify_source("alsa_input.webcam", "Webcam C920", &empty),
            CLASS_CAMERA
        );
        assert_eq!(
            classify_source("bluez_input.x", "Headset Mic", &empty),
            CLASS_HEADSET
        );
    }

    #[test]
    fn resolve_volume_sink_follows_dsp() {
        let inputs = "Sink Input #7\n\t\tSink: 43\n\t\tProperties:\n\t\t\tnode.name = \"tuning_sink_output\"\nSink Input #9\n\t\tSink: 43\n\t\tProperties:\n\t\t\tapplication.name = \"Firefox\"\n\t\t\tnode.name = \"Firefox\"\n";
        let short = "43\talsa_output.pci-0000_00_1f.3.analog-stereo\tPipeWire\n115\ttuning_sink\tPipeWire\n";
        let run = FnRunner(move |_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("pactl list sink-inputs") {
                Ok(inputs.to_string())
            } else if key.starts_with("pactl list sinks short") {
                Ok(short.to_string())
            } else {
                Err("no stub".into())
            }
        });
        assert_eq!(
            resolve_volume_sink(&Ctx::new(), &run, "alsa_output.pci.x"),
            "alsa_output.pci.x"
        );
        assert_eq!(
            resolve_volume_sink(&Ctx::new(), &run, "tuning_sink"),
            "alsa_output.pci-0000_00_1f.3.analog-stereo"
        );
        assert_eq!(
            resolve_volume_sink(&Ctx::new(), &run, "easyeffects_sink"),
            "easyeffects_sink"
        );
    }

    #[test]
    fn resolve_easyeffects() {
        let inputs = "Sink Input #3\n\t\tSink: 43\n\t\tProperties:\n\t\t\tapplication.name = \"EasyEffects\"\n\t\t\tnode.name = \"easyeffects\"\n";
        let run = FnRunner(move |_: &Ctx, name: &str, args: &[&str]| {
            let key = format!("{} {}", name, args.join(" "));
            if key.starts_with("pactl list sink-inputs") {
                Ok(inputs.to_string())
            } else {
                Ok("43\talsa_output.pci.x\tPipeWire\n".to_string())
            }
        });
        assert_eq!(
            resolve_volume_sink(&Ctx::new(), &run, "easyeffects_sink"),
            "alsa_output.pci.x"
        );
    }

    #[test]
    fn app_sink_input_ids_filters_chains() {
        let inputs = "Sink Input #1\n\t\tProperties:\n\t\t\tapplication.name = \"Firefox\"\nSink Input #2\n\t\tProperties:\n\t\t\tnode.name = \"tuning_chain\"\nSink Input #3\n\t\tProperties:\n\t\t\tapplication.name = \"EasyEffects\"\n";
        assert_eq!(app_sink_input_ids(inputs), vec!["1"]);
    }

    #[test]
    fn sink_available_cases() {
        assert!(pactl::sink_available(&[]));
        assert!(pactl::sink_available(&[
            Port {
                availability: "not available".into()
            },
            Port {
                availability: "available".into()
            },
        ]));
        assert!(!pactl::sink_available(&[Port {
            availability: "not available".into()
        }]));
    }

    #[test]
    fn collector_status() {
        let mut c = Collector::new(Arc::new(stub_audio()));
        c.has = Arc::new(|_| true);
        let st = c.collect(&Ctx::new()).unwrap();
        assert_eq!(st.volume_pct, 80);
        assert!(!st.muted);
        assert_eq!(st.volume_sink, "alsa_output.pci-0000_00_1f.3.analog-stereo");
        assert_eq!(st.sinks.len(), 2);
        assert_eq!(st.sinks[0].info.class, CLASS_HEADSET);
        assert!(st.sinks[0].info.bluetooth);
        assert!(!st.sinks[1].available);
        assert_eq!(st.sink.name, st.volume_sink);
        assert_eq!(st.source.class, CLASS_MICROPHONE);
        assert!(st.sources[0].is_default);
        assert_eq!(st.input_volume_pct, 80);
        assert!(!st.input_muted);
    }

    #[test]
    fn collector_requires_pactl() {
        let mut c = Collector::new(Arc::new(stub_audio()));
        c.has = Arc::new(|_| false);
        assert!(c.collect(&Ctx::new()).is_err());
    }

    #[test]
    fn monitor_name_and_snapshot() {
        let mut c = Collector::new(Arc::new(stub_audio()));
        c.has = Arc::new(|_| true);
        let m = Monitor::new_with_collector(c);
        assert_eq!(m.name(), "audio");
        let v = m.snapshot(&Ctx::new()).unwrap();
        assert_eq!(v["volume_pct"], 80);
        // Go embeds SinkInfo/SourceInfo: the JSON must stay flat.
        assert_eq!(v["sinks"][0]["name"], "bluez_output.44_73_D6_A4_73_3F.1");
        assert_eq!(v["sinks"][0]["class"], CLASS_HEADSET);
        assert_eq!(v["sinks"][0]["bluetooth"], true);
        assert!(v["sources"][0].get("class").is_some());
    }
}
