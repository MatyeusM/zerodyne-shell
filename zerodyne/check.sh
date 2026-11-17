#!/usr/bin/env bash
# Same gates CI should run before this becomes the official daemon.
set -euo pipefail
cd "$(dirname "$0")"
cargo fmt --all -- --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline
cargo build --release --offline
