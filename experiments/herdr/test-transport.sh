#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/pantheon-herdr-target}"
export PANTHEON_HERDR_TEST_ROOT="${PANTHEON_HERDR_TEST_ROOT:-/tmp/pantheon-herdr-transport-demo}"
python3 experiments/herdr/runtime.py start --root "$PANTHEON_HERDR_TEST_ROOT"
cargo test --lib --manifest-path src-tauri/Cargo.toml --features herdr-experiment \
  live_bridge_persists_orders_buffers_resizes_and_terminates \
  -- --ignored --nocapture --test-threads=1
python3 experiments/herdr/runtime.py stop --root "$PANTHEON_HERDR_TEST_ROOT"
