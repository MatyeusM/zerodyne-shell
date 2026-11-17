//! One-shot PATH probe for the external tools the daemon shells out to.
//! Monitors skip probes whose tool is missing instead of forking and failing;
//! daemon info publishes the verdict so QML can render "unavailable".

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

/// Keep in sync with the tools actually used by monitors/interactions.
pub const TOOLS: &[&str] = &[
    "ip",
    "nmcli",
    "iw",
    "ping",
    "qrencode",
    "curl",
    "pactl",
    "wpctl",
    "systemctl",
    "hyprctl",
    "hyprlock",
    "amd-smi",
    "powerprofilesctl",
];

static CACHE: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<String, bool>> {
    CACHE.get_or_init(|| RwLock::new(probe(TOOLS)))
}

/// Probe every known tool once per process and return the cached verdict.
pub fn probe_all() -> HashMap<String, bool> {
    cache().read().unwrap().clone()
}

/// Whether `name` is on PATH (cached; unknown names are probed on demand).
pub fn available(name: &str) -> bool {
    if let Ok(map) = cache().read()
        && let Some(v) = map.get(name)
    {
        return *v;
    }
    let ok = look_path(name);
    if let Ok(mut map) = cache().write() {
        map.entry(name.to_string()).or_insert(ok);
    }
    ok
}

/// Re-probe everything (PATH may have changed).
pub fn refresh() -> HashMap<String, bool> {
    let fresh = probe(TOOLS);
    if let Ok(mut map) = cache().write() {
        *map = fresh.clone();
    }
    fresh
}

fn probe(names: &[&str]) -> HashMap<String, bool> {
    names
        .iter()
        .map(|n| (n.to_string(), look_path(n)))
        .collect()
}

fn look_path(name: &str) -> bool {
    if name.contains('/') {
        return is_executable(Path::new(name));
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|dir| is_executable(&dir.join(name)))
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.permissions().mode() & 0o111 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_cache_and_known_tools() {
        let a = probe_all();
        let b = probe_all();
        for tool in TOOLS {
            assert_eq!(a[*tool], b[*tool]);
        }
        assert!(available("sh"));
        assert!(!available("zerodyne-definitely-missing-binary"));
        assert_eq!(refresh().len(), TOOLS.len());
    }
}
