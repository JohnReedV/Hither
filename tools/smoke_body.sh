#!/usr/bin/env bash
# Isolated look-down/body QA. Never sends input to the desktop display.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/body-config/hither-sdf
cp tools/smoke-settings.json tools/build/body-config/hither-sdf/settings.json
test ! -e /tmp/.X89-lock
tools/vendor/xvfb/usr/bin/Xvfb :89 -screen 0 1280x720x24 -nolisten tcp > tools/build/body-xvfb.log 2>&1 &
body_display_pid=$!
body_game_pid=""
trap 'if [[ -n "$body_game_pid" ]]; then kill "$body_game_pid" 2>/dev/null || true; fi; kill "$body_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:89 BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/body-config" HITHER_WORLD_SEED=721
"${HITHER_BINARY:-target/release/hither-sdf}" > tools/build/body-runtime.log 2>&1 &
body_game_pid=$!
sleep "${HITHER_SMOKE_WAIT:-15}"
body_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
xdotool windowfocus --sync "$body_window"
import -window "$body_window" tools/build/body-forward.png
xdotool mousemove_relative -- 0 140
sleep 0.5
import -window "$body_window" tools/build/body-down.png
xdotool mousemove_relative -- 0 350
sleep 0.5
import -window "$body_window" tools/build/body-straight-down.png
for direction in w s a d; do
    xdotool keydown "$direction"
    sleep 0.6
    import -window "$body_window" "tools/build/body-walk-$direction.png"
    xdotool keyup "$direction"
    sleep 0.3
done
xdotool keydown space
sleep 0.2
import -window "$body_window" tools/build/body-jump.png
xdotool keyup space
sleep 0.7
import -window "$body_window" tools/build/body-land.png
xdotool mousemove_relative -- 240 0
sleep 0.2
import -window "$body_window" tools/build/body-turn.png
sleep 1.2
xdotool click 1
sleep 0.15
import -window "$body_window" tools/build/body-punch.png
sleep 0.8
xdotool key F3
sleep 0.6
import -window "$body_window" tools/build/body-third.png
xdotool key F3 F3
sleep 0.6
import -window "$body_window" tools/build/body-return.png
xdotool key Escape
sleep 0.5
import -window "$body_window" tools/build/body-pause-a.png
sleep 0.5
import -window "$body_window" tools/build/body-pause-b.png
if rg -n 'ERROR|panicked' tools/build/body-runtime.log; then exit 1; fi
