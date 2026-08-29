#!/usr/bin/env bash
# Launch Pantheon in dev mode on Linux/macOS. The Windows dev.cmd exists to set up
# the MSVC environment first; there is no equivalent step here, so this is a thin
# wrapper kept for parity with the documented entry point.
set -euo pipefail
cd "$(dirname "$0")"
echo "Starting Pantheon (pnpm tauri dev)..."
exec pnpm tauri dev "$@"
