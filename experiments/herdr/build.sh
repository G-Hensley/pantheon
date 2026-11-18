#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/pantheon-herdr-target}"
pnpm tauri build --debug --no-bundle --features herdr-experiment \
  --config src-tauri/herdr-experiment.conf.json
