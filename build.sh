#!/usr/bin/env bash
# Build the Pantheon bundles on Linux/macOS. See build.cmd for the Windows path.
set -euo pipefail
cd "$(dirname "$0")"
echo "Building Pantheon (pnpm tauri build)..."
pnpm tauri build "$@"
echo
echo "Done. Artifacts under src-tauri/target/release/bundle/"
