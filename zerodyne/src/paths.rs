//! Runtime locations: prefer XDG_RUNTIME_DIR (per-user tmpfs), fall back to
//! XDG_STATE_HOME so headless runs still work.

use std::path::PathBuf;

pub fn dir() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_RUNTIME_DIR")
        && !v.is_empty()
    {
        return PathBuf::from(v).join("zerodyne");
    }
    state_dir().join("zerodyne")
}

pub fn state_dir() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_STATE_HOME")
        && !v.is_empty()
    {
        return PathBuf::from(v).join("zerodyne");
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".local")
        .join("state")
        .join("zerodyne")
}

pub fn sock_file() -> PathBuf {
    dir().join("zerodyne.sock")
}

pub fn pid_file() -> PathBuf {
    dir().join("zerodyne.pid")
}
