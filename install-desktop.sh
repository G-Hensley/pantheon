#!/usr/bin/env bash
# Install Pantheon as a desktop application on Linux: build the release binary,
# drop it in ~/.local/bin, register icons and a desktop entry, adopt the state
# the pre-rename Mosaic build left behind, and retire the Mosaic launcher.
# Re-run it after code changes to update the installed copy.
#
# Everything lands under $HOME, so no root is needed.
set -euo pipefail
cd "$(dirname "$0")"

BIN_DIR="$HOME/.local/bin"
# One root for all three, because Tauri resolves the app data directory through
# XDG_DATA_HOME. Hard-coding ~/.local/share here would put the launcher and the
# data it is meant to hand over in different places on a machine that sets it.
DATA_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="$DATA_ROOT/applications"
ICON_DIR="$DATA_ROOT/icons/hicolor"

# The pre-rename identity. Both names are spelled out rather than derived,
# because the whole point of this block is that the two differ.
LEGACY_APP="mosaic"
LEGACY_ID="com.gavinhensley.mosaic"
CURRENT_ID="com.gavinhensley.pantheon"

adopt_state=1
clean_legacy=1
force=0

usage() {
  cat <<'USAGE'
Usage: ./install-desktop.sh [options]

  --keep-current-state     Do not adopt the UI state saved by the Mosaic build.
  --keep-legacy-launcher   Leave the Mosaic binary, desktop entry, and icons in place.
  --force                  Install even while Pantheon or Mosaic is running.
  -h, --help               Show this message.
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --keep-current-state) adopt_state=0 ;;
    --keep-legacy-launcher) clean_legacy=0 ;;
    --force) force=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

# A running instance holds its WebKit local-storage database open and rewrites
# it on exit, so adopting state underneath one would be overwritten or corrupted
# the moment that instance quits. Retiring a launcher while its binary is live
# is merely confusing, but the state handover is not safe, so both wait.
running="$( { pgrep -x pantheon || true; pgrep -x "$LEGACY_APP" || true; } | tr '\n' ' ')"
if [ -n "${running// /}" ] && [ "$force" -eq 0 ]; then
  echo "Pantheon or Mosaic is running (pids: ${running% }). Quit it first, or pass --force" >&2
  echo "to install the binary only and skip the state handover." >&2
  exit 1
fi
if [ -n "${running// /}" ]; then
  echo "==> A session is running; skipping state handover and launcher cleanup"
  adopt_state=0
  clean_legacy=0
fi

# Rewrite one entry of a GNOME favorites list. Kept as a pure function so the
# substitution can be exercised without a session bus. Reads the old list on
# stdin, writes the new one on stdout.
swap_favorite() {
  # Read once. Deciding with `grep` on the stream would consume the input the
  # substitution still needs, and the function would return nothing at all.
  local list
  list="$(cat)"
  case "$list" in
    *"'pantheon.desktop'"*)
      # Pantheon is already pinned, so the Mosaic entry is dropped rather than
      # duplicated. The comma goes with it, on whichever side it sits.
      printf '%s' "$list" | sed -e "s/'mosaic\.desktop', //" \
        -e "s/, 'mosaic\.desktop'//" -e "s/'mosaic\.desktop'//"
      ;;
    *)
      printf '%s' "$list" | sed -e "s/'mosaic\.desktop'/'pantheon.desktop'/"
      ;;
  esac
}

# Removing mosaic.desktop while it is pinned to the dash leaves a dead
# favorite that GNOME renders as a blank tile, so the pin moves with the app.
# Anything other than GNOME has no such list and is left alone.
repin_favorite() {
  command -v gsettings >/dev/null || return 0
  local key="org.gnome.shell favorite-apps"
  local before
  # shellcheck disable=SC2086
  before="$(gsettings get $key 2>/dev/null || true)"
  case "$before" in
    *"'mosaic.desktop'"*) ;;
    *) return 0 ;;
  esac
  local after
  after="$(printf '%s' "$before" | swap_favorite)"
  # shellcheck disable=SC2086
  if gsettings set $key "$after" 2>/dev/null; then
    echo "==> Repinned the dash favorite to pantheon.desktop"
    echo "    previous value: $before"
  else
    echo "    warning: could not update the dash favorite; re-pin Pantheon by hand" >&2
  fi
}

echo "==> Building release binary"
pnpm tauri build --no-bundle

echo "==> Installing binary to $BIN_DIR/pantheon"
mkdir -p "$BIN_DIR"
install -m 755 src-tauri/target/release/pantheon "$BIN_DIR/pantheon"

echo "==> Installing icons"
mkdir -p "$ICON_DIR/scalable/apps"
cp app-icon.svg "$ICON_DIR/scalable/apps/pantheon.svg"
for sz in 32 64 128; do
  [ -f "src-tauri/icons/${sz}x${sz}.png" ] || continue
  mkdir -p "$ICON_DIR/${sz}x${sz}/apps"
  cp "src-tauri/icons/${sz}x${sz}.png" "$ICON_DIR/${sz}x${sz}/apps/pantheon.png"
done
if [ -f src-tauri/icons/128x128@2x.png ]; then
  mkdir -p "$ICON_DIR/256x256/apps"
  cp src-tauri/icons/128x128@2x.png "$ICON_DIR/256x256/apps/pantheon.png"
fi

echo "==> Installing desktop entry"
mkdir -p "$APP_DIR"
# Exec goes through a login shell so ~/.profile puts ~/.local/bin on PATH. The
# graphical session does not reliably include it. Agent CLIs must therefore be
# installed or linked in ~/.local/bin; a non-interactive bash does not process
# PATH additions below ~/.bashrc's interactivity guard. $HOME is expanded here
# because the desktop entry spec does not allow variables in Exec.
cat > "$APP_DIR/pantheon.desktop" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Pantheon
GenericName=Multi-agent cockpit
Comment=Run parallel AI coding sessions side by side, connected by a shared brain
Exec=bash -lc "exec $BIN_DIR/pantheon"
Icon=pantheon
Terminal=false
Categories=Development;IDE;
Keywords=AI;agent;terminal;claude;codex;opencode;cockpit;
StartupNotify=true
StartupWMClass=pantheon
EOF

# Pane roster, chosen project, layout, and appearance live in the webview's
# local storage, and WebKit keys that store by bundle identifier. The rename
# changed the identifier, so `readStored`'s "mosaic." to "pantheon." key
# adoption cannot reach it: the keys are intact but sitting in a different
# database file. Carrying that one file across is what makes the first Pantheon
# launch look like the last Mosaic one.
#
# Only the packaged app's origin moves. `http_localhost_1420.*` belongs to
# `pnpm tauri dev`, which is a separate profile and not what is being installed.
if [ "$adopt_state" -eq 1 ]; then
  legacy_store="$DATA_ROOT/$LEGACY_ID/localstorage"
  current_store="$DATA_ROOT/$CURRENT_ID/localstorage"
  legacy_db="$legacy_store/tauri_localhost_0.localstorage"
  current_db="$current_store/tauri_localhost_0.localstorage"

  if [ ! -f "$legacy_db" ]; then
    : # Nothing to adopt: either a fresh machine or the handover already ran.
  elif [ -f "$current_db" ] && [ "$current_db" -nt "$legacy_db" ]; then
    echo "==> Pantheon state is newer than Mosaic's; leaving it alone"
  else
    echo "==> Adopting UI state from $LEGACY_ID"
    mkdir -p "$current_store"
    if [ -f "$current_db" ]; then
      backup="$current_store/pre-adopt-$(date +%Y%m%d%H%M%S)"
      mkdir -p "$backup"
      cp -p "$current_store"/tauri_localhost_0.localstorage* "$backup/"
      echo "    previous state backed up to $backup"
    fi
    # The -wal and -shm siblings carry writes the main file has not absorbed
    # yet, so they travel together or the copy loses the most recent session.
    cp -p "$legacy_store"/tauri_localhost_0.localstorage* "$current_store/"
    echo "    Mosaic's state is untouched and remains the rollback"
  fi
fi

# Worktrees, per-session agent config, and the shared brain's markdown are NOT
# touched here. `compatible_app_data_dir` deliberately keeps using
# $LEGACY_ID for those when it exists, because worktree registrations and saved
# pane records hold absolute paths beneath it. Deleting that directory would
# strand real work, so this only ever retires the launcher.
if [ "$clean_legacy" -eq 1 ]; then
  removed=0
  for stale in \
    "$BIN_DIR/$LEGACY_APP" \
    "$APP_DIR/$LEGACY_APP.desktop" \
    "$ICON_DIR/scalable/apps/$LEGACY_APP.svg" \
    "$ICON_DIR"/*/apps/"$LEGACY_APP.png"
  do
    [ -e "$stale" ] || continue
    rm -f "$stale"
    echo "==> Removed stale Mosaic file $stale"
    removed=1
  done
  if [ "$removed" -eq 1 ]; then
    echo "    ($DATA_ROOT/$LEGACY_ID kept: it still holds worktrees and context)"
    repin_favorite
  fi
fi

# A validation warning must not abort an install whose files are already in
# place, but it should not pass unremarked either.
command -v desktop-file-validate >/dev/null && { desktop-file-validate "$APP_DIR/pantheon.desktop" || echo "warning: pantheon.desktop failed validation" >&2; }
command -v update-desktop-database >/dev/null && update-desktop-database "$APP_DIR" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true

echo
echo "Done. Pantheon is in your app grid; run it with 'pantheon' or launch it from there."
