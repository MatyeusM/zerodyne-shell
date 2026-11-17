//! zerodyne — local observation/control daemon for the quickshell desktop.
//!
//! This crate is a Rust rewrite of the Go daemon in `../zerodyne`. It keeps
//! the same CLI, the same newline-delimited JSON IPC protocol, and the same
//! snapshot/result JSON shapes so QML (and the CLI) need no changes.
//!
//! Layer layout mirrors the Go original:
//!
//! * `ipc` — the wire protocol only.
//! * `runner` — the single boundary for spawning system commands.
//! * `monitor` — read-only observation (`status <target>`).
//! * `interaction` — state-changing operations (`exec <op>`).
//! * `daemon` — lifecycle: pidfile lock, socket serving, dispatch, idle exit.

pub mod caps;
pub mod daemon;
pub mod interaction;
pub mod ipc;
pub mod monitor;
pub mod paths;
pub mod runner;

/// Go's `fmt.Sprintf("%s", []string{...})` renders as `[a b c]`. A few error
/// messages are matched verbatim by the CLI/QML, so keep that shape.
pub(crate) fn go_list(items: &[String]) -> String {
    format!("[{}]", items.join(" "))
}
