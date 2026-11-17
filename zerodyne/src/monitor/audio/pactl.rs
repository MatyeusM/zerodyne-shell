//! Tolerant deserialization of `pactl -f json list sinks/sources` payloads.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize, Default)]
pub struct VolumeValue {
    #[serde(default, rename = "value_percent")]
    pub value_percent: String,
}

#[derive(Deserialize, Default)]
pub struct Port {
    #[serde(default)]
    pub availability: String,
}

#[derive(Deserialize, Default)]
pub struct PactlSink {
    #[serde(default)]
    pub index: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub volume: HashMap<String, VolumeValue>,
    #[serde(default)]
    pub ports: Vec<Port>,
    #[serde(default)]
    pub properties: HashMap<String, Value>,
}

#[derive(Deserialize, Default)]
pub struct PactlSource {
    #[serde(default)]
    pub index: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub properties: HashMap<String, Value>,
}

/// pactl occasionally emits non-string property values; stringify tolerantly
/// instead of failing the whole list parse the way Go's `map[string]string`
/// would.
pub fn props(raw: &HashMap<String, Value>) -> HashMap<String, String> {
    raw.iter()
        .filter_map(|(k, v)| match v {
            Value::String(s) => Some((k.clone(), s.clone())),
            Value::Null => None,
            other => Some((k.clone(), other.to_string())),
        })
        .collect()
}

/// description, device.description property, then name.
pub fn sink_description(s: &PactlSink, props: &HashMap<String, String>) -> String {
    if !s.description.is_empty() {
        return s.description.clone();
    }
    if let Some(v) = props.get("device.description")
        && !v.is_empty()
    {
        return v.clone();
    }
    s.name.clone()
}

/// No ports, or at least one port not marked "not available".
pub fn sink_available(ports: &[Port]) -> bool {
    if ports.is_empty() {
        return true;
    }
    ports.iter().any(|p| p.availability != "not available")
}

/// First channel whose `value_percent` parses as an integer ("37%").
pub fn sink_json_volume(vol: &HashMap<String, VolumeValue>) -> i32 {
    for ch in vol.values() {
        let raw = ch
            .value_percent
            .strip_suffix('%')
            .unwrap_or(&ch.value_percent);
        if let Ok(v) = raw.parse::<i32>() {
            return v;
        }
    }
    0
}
