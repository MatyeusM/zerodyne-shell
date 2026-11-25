//! Handler abstraction and registry. Handlers change system state and are
//! deliberately separate from monitors. Each validates its own args, reports
//! usage on misuse, and returns a structured result or a typed error.

pub mod audio;
pub mod bluetooth;
pub mod network;
pub mod power;
pub mod powerprofile;
pub mod wallpaper;

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::runner::Ctx;

/// Structured outcome of a handler execution (`message` and/or `data`).
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct ResultData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Map<String, Value>>,
}

impl ResultData {
    pub fn message(msg: impl Into<String>) -> Self {
        Self {
            message: Some(msg.into()),
            data: None,
        }
    }

    pub fn with_data(data: Map<String, Value>) -> Self {
        Self {
            message: None,
            data: Some(data),
        }
    }
}

pub trait Handler: Send + Sync {
    /// The `exec <operation>` selector, e.g. "wifi-band".
    fn name(&self) -> &'static str;
    /// One-line synopsis shown on argument errors.
    fn usage(&self) -> &'static str;
    /// Validate args, perform the operation, return the result.
    fn exec(&self, ctx: &Ctx, args: &[String]) -> Result<ResultData>;
}

pub struct Registry {
    handlers: RwLock<BTreeMap<String, Arc<dyn Handler>>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn register(&self, h: Arc<dyn Handler>) -> Result<()> {
        if h.name().is_empty() {
            return Err(anyhow!("interaction: unnamed handler"));
        }
        let mut map = self.handlers.write().unwrap();
        if map.contains_key(h.name()) {
            return Err(anyhow!("interaction: duplicate {:?}", h.name()));
        }
        map.insert(h.name().to_string(), h);
        Ok(())
    }

    pub fn must_register(&self, h: Arc<dyn Handler>) {
        self.register(h).expect("handler registration failed");
    }

    pub fn exec(&self, ctx: &Ctx, name: &str, args: &[String]) -> Result<ResultData> {
        let h = self
            .handlers
            .read()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "unknown operation {:?} (known: {})",
                    name,
                    crate::go_list(&self.names())
                )
            })?;
        h.exec(ctx, args)
    }

    pub fn names(&self) -> Vec<String> {
        self.handlers.read().unwrap().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub;
    impl Handler for Stub {
        fn name(&self) -> &'static str {
            "do"
        }
        fn usage(&self) -> &'static str {
            "do ..."
        }
        fn exec(&self, _: &Ctx, args: &[String]) -> Result<ResultData> {
            Ok(ResultData::message(format!("ran with {} args", args.len())))
        }
    }

    #[test]
    fn dispatch_and_duplicates() {
        let r = Registry::new();
        r.must_register(Arc::new(Stub));
        assert_eq!(r.names(), vec!["do"]);
        assert_eq!(
            r.exec(&Ctx::new(), "do", &["x".into()]).unwrap().message,
            Some("ran with 1 args".into())
        );
        assert!(r.exec(&Ctx::new(), "nope", &[]).is_err());
        assert!(r.register(Arc::new(Stub)).is_err());
    }
}

#[cfg(test)]
mod shape_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn result_data_json_matches_go_omitempty() {
        // Go: Result{Message: "..."} -> {"message":"..."}
        let only_message = ResultData::message("connected");
        assert_eq!(
            serde_json::to_value(&only_message).unwrap(),
            json!({"message": "connected"})
        );

        // Go: Result{Data: map[...]} -> {"data":{...}}
        let data = json!({"band": "5", "available": ["2.4", "5"]})
            .as_object()
            .unwrap()
            .clone();
        let only_data = ResultData::with_data(data);
        assert_eq!(
            serde_json::to_value(&only_data).unwrap(),
            json!({"data": {"band": "5", "available": ["2.4", "5"]}})
        );

        // Go: zero Result -> {} (and the CLI prints "ok" only for null/empty).
        assert_eq!(
            serde_json::to_value(ResultData::default()).unwrap(),
            json!({})
        );
    }
}
