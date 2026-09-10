#!/usr/bin/env bash
# Isolated first-person viewmodel visual/visibility checks; no desktop input.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/hand-config/hither-sdf
cp tools/smoke-settings.json tools/build/hand-config/hither-sdf/settings.json
test ! -e /tmp/.X93-lock
tools/vendor/xvfb/usr/bin/Xvfb :93 -screen 0 1280x720x24 -nolisten tcp > tools/build/hand-xvfb.log 2>&1 &
hand_display_pid=$!
hand_game_pid=""
trap 'if [[ -n "$hand_game_pid" ]]; then kill "$hand_game_pid" 2>/dev/null || true; fi; kill "$hand_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:93 BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/hand-config" HITHER_WORLD_SEED=721
"${HITHER_BINARY:-target/release/hither-sdf}" > tools/build/hand-runtime.log 2>&1 &
hand_game_pid=$!
sleep "${HITHER_SMOKE_WAIT:-12}"
hand_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
xdotool windowfocus --sync "$hand_window"
import -window "$hand_window" tools/build/hand-idle.png
xdotool keydown w
sleep 1
import -window "$hand_window" tools/build/hand-walk.png
xdotool keyup w
xdotool key space
sleep 0.2
import -window "$hand_window" tools/build/hand-jump.png
sleep 0.8
xdotool key F3
sleep 0.5
import -window "$hand_window" tools/build/hand-third-person.png
xdotool key F3
sleep 0.4
import -window "$hand_window" tools/build/hand-second-person.png
xdotool key F3
sleep 0.4
import -window "$hand_window" tools/build/hand-return.png
xdotool key Escape
sleep 0.5
import -window "$hand_window" tools/build/hand-paused-a.png
sleep 0.5
import -window "$hand_window" tools/build/hand-paused-b.png
if rg -n 'ERROR|panicked' tools/build/hand-runtime.log; then exit 1; fi
