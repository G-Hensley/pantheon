#!/usr/bin/env bash
set -euo pipefail
export PANTHEON_HERDR_ROOT="${PANTHEON_HERDR_ROOT:-/tmp/pantheon-herdr-demo}"
export PANTHEON_HERDR_BIN="${PANTHEON_HERDR_BIN:-/tmp/pantheon-herdr-dist/bin/herdr-core}"
export PANTHEON_HERDR_CODEX="${PANTHEON_HERDR_CODEX:-$(command -v codex)}"
export XDG_DATA_HOME="$PANTHEON_HERDR_ROOT/gui-data"
export XDG_CACHE_HOME="$PANTHEON_HERDR_ROOT/gui-cache"
exec "${CARGO_TARGET_DIR:-$HOME/.cache/pantheon-herdr-target}/debug/pantheon"
