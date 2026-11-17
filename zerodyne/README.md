# zerodyne

The official zerodyne daemon — a Rust rewrite of the former Go implementation.
It is the local observation/control backend for the quickshell desktop: QML polls
`status <target>` and calls `exec <op>` over a line-delimited JSON Unix socket.

`mise run build` compiles it to `bin/zerodyne`, which is the path
`services/Zerodyne.qml` resolves via `Quickshell.shellPath("bin/zerodyne")`.

## Build & checks

```sh
mise run build                    # -> bin/zerodyne
mise run check                    # fmt check + clippy -D warnings + tests
cd zerodyne && ./check.sh         # same gates, plus a release build
```

The release profile is size-first (`opt-level = "z"`, LTO, single codegen unit,
`panic = "abort"`, strip) and produces a ~720 KB binary, versus ~6 MB for Go.

## Layout

| File | Responsibility |
| --- | --- |
| `src/ipc.rs` | newline-delimited JSON request/response protocol |
| `src/runner.rs` | the single `Runner` boundary + `Ctx` deadlines/cancellation |
| `src/paths.rs`, `src/caps.rs` | runtime paths, cached PATH capability probe |
| `src/monitor/` | read-only `status <target>` monitors |
| `src/interaction/` | state-changing `exec <op>` handlers |
| `src/daemon.rs` | lock, socket, dispatch, idle exit, lazy client helpers |
| `src/main.rs` | CLI entry point |
| `tests/lifecycle.rs` | end-to-end socket/lifecycle integration tests |

Each `.rs` file is ≤300 lines excluding its `#[cfg(test)]` module. Edition 2024,
no regex dependency (the four simple patterns are hand-written).

## Deliberate implementation notes

- **Command runner**: one shared `Runner` trait. `OutputRunner` discards stderr
  (monitor probes), `CombinedRunner` keeps it (interactions, so error text like
  "Secrets were required" survives).
- **Cancellation**: `Ctx` carries both a deadline (daemon caps requests at 30s)
  and a `CancelToken`. `run_process` kills the child when either fires, so
  `stop()` interrupts an in-flight Wi-Fi scan instead of waiting for its 25s
  timeout. `stop()` also verifies the process is gone and cleans up
  socket/pidfile even after SIGKILL.
- **Concurrency — is async needed?** No. Workloads are short subprocess calls
  plus one background scanner, so `std::thread` is sufficient and keeps the
  dependency graph (and binary) small. Accepted connections use 128 KB stacks
  and a 64-client cap with progressive accept-error backoff; gateway/internet
  pings run in two scoped threads so two unreachable hosts cost ~1s, not ~2s.
  Registries/trackers use `RwLock`/`Mutex`; signals use an async-signal-safe
  self-pipe that wakes the daemon's `poll` loop.
- **Duration parity**: `parse_duration_nanos`/`go_duration` port Go's
  `time.ParseDuration`/`Duration.String` (signs, both micro signs, fractional
  rendering, overflow rejection) so the wire format stays identical.
- **Float encoding**: `Response::ok` normalises whole `f64` values to integers
  so the wire JSON matches Go's `encoding/json` (`62`, not `62.0`).
- **Capabilities**: cached once per process, published in `status` info.
