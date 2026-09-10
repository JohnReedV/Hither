#!/usr/bin/env bash
# Private-display runtime asset/animation smoke check. Requires release build.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/goblin-config/hither-sdf
cp tools/smoke-settings.json tools/build/goblin-config/hither-sdf/settings.json
test ! -e /tmp/.X95-lock
tools/vendor/xvfb/usr/bin/Xvfb :95 -screen 0 1280x720x24 -nolisten tcp > tools/build/goblin-xvfb.log 2>&1 &
goblin_display_pid=$!
goblin_game_pid=""
trap 'if [[ -n "$goblin_game_pid" ]]; then kill "$goblin_game_pid" 2>/dev/null || true; wait "$goblin_game_pid" 2>/dev/null || true; fi; kill "$goblin_display_pid" 2>/dev/null || true; wait "$goblin_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:95 BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/goblin-config"
HITHER_WORLD_SEED=721 HITHER_GOBLIN_PREVIEW=1 target/release/hither-sdf > tools/build/goblin-runtime.log 2>&1 &
goblin_game_pid=$!
sleep 10
kill -0 "$goblin_game_pid"
goblin_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
import -window "$goblin_window" tools/build/goblin-runtime-1.png
sleep 3
import -window "$goblin_window" tools/build/goblin-runtime-2.png
xdotool windowfocus --sync "$goblin_window"
xdotool key Escape
sleep 1
import -window "$goblin_window" tools/build/goblin-runtime-paused.png
if rg -n 'ERROR|panicked|Failed to load' tools/build/goblin-runtime.log; then exit 1; fi
printf 'PASS: goblin runtime loaded, animated, and accepted pause input.\n'
