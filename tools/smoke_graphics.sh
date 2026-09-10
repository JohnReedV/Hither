#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/graphics-config/hither-sdf
cp tools/smoke-settings.json tools/build/graphics-config/hither-sdf/settings.json
test ! -e /tmp/.X95-lock
tools/vendor/xvfb/usr/bin/Xvfb :95 -screen 0 1280x720x24 -nolisten tcp > tools/build/graphics-xvfb.log 2>&1 &
graphics_display_pid=$!
graphics_game_pid=""
trap 'if [[ -n "$graphics_game_pid" ]]; then kill "$graphics_game_pid" 2>/dev/null || true; wait "$graphics_game_pid" 2>/dev/null || true; fi; kill "$graphics_display_pid" 2>/dev/null || true; wait "$graphics_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:95 BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/graphics-config"
"${HITHER_TEST_BINARY:-target/release/hither-sdf}" > tools/build/graphics-runtime.log 2>&1 &
graphics_game_pid=$!
sleep "${HITHER_SMOKE_WAIT:-6}"
graphics_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
xdotool windowfocus --sync "$graphics_window"
import -window "$graphics_window" tools/build/graphics-world.png
if [[ "${HITHER_GRASS_TRAMPLE_TEST:-0}" == "1" ]]; then
    xdotool mousemove_relative -- 0 900
    xdotool keydown w
    sleep 1
    import -window "$graphics_window" tools/build/grass-first-person-step.png
    xdotool keyup w
    if rg -n 'ERROR|panicked' tools/build/graphics-runtime.log; then exit 1; fi
    exit 0
fi
xdotool key Escape
sleep 0.5
import -window "$graphics_window" tools/build/graphics-pause.png
xdotool mousemove --window "$graphics_window" 640 400 click 1
sleep 0.5
xdotool mousemove --window "$graphics_window" 740 489 click 1
sleep 0.3
import -window "$graphics_window" tools/build/graphics-options.png
xdotool mousemove --window "$graphics_window" 640 550 click 1
sleep 0.5
import -window "$graphics_window" tools/build/graphics-advanced.png
if [[ "${HITHER_GRASS_DISTANCE_TEST:-0}" == "1" ]]; then
    xdotool mousemove --window "$graphics_window" 788 214 click 1
    xdotool type --clearmodifiers '64'
    xdotool key Return
    for grass_detail in 16 64; do
        xdotool mousemove --window "$graphics_window" 788 307 click 1
        xdotool type --clearmodifiers "$grass_detail"
        xdotool key Return
        sleep 1
        xdotool key Escape Escape
        sleep 15
        import -window "$graphics_window" "tools/build/grass-detail-${grass_detail}.png"
        xdotool key Escape
        sleep 0.3
        xdotool mousemove --window "$graphics_window" 640 400 click 1
        sleep 0.3
        xdotool mousemove --window "$graphics_window" 640 550 click 1
        sleep 0.3
    done
    rg '"render_distance": 64' tools/build/graphics-config/hither-sdf/settings.json
    rg '"detail_distance": 64' tools/build/graphics-config/hither-sdf/settings.json
    if rg -n 'ERROR|panicked' tools/build/graphics-runtime.log; then exit 1; fi
    exit 0
fi
if [[ "${HITHER_DISTANCE_TEST:-0}" == "1" ]]; then
    xdotool mousemove --window "$graphics_window" 788 214 click 1
    xdotool type --clearmodifiers '64'
    xdotool key Return
    xdotool mousemove --window "$graphics_window" 788 307 click 1
    xdotool type --clearmodifiers '32'
    xdotool key Return
    sleep 1
    rg '"render_distance": 64' tools/build/graphics-config/hither-sdf/settings.json
    rg '"detail_distance": 32' tools/build/graphics-config/hither-sdf/settings.json
    import -window "$graphics_window" tools/build/graphics-distances.png
    if rg -n 'ERROR|panicked' tools/build/graphics-runtime.log; then exit 1; fi
    exit 0
fi
if [[ "${HITHER_GRAPHICS_PREVIEW:-0}" == "1" ]]; then
    if rg -n 'ERROR|panicked' tools/build/graphics-runtime.log; then exit 1; fi
    exit 0
fi
xdotool mousemove --window "$graphics_window" 788 214 click 1
xdotool type --clearmodifiers '999'
xdotool key Return
sleep 0.5
import -window "$graphics_window" tools/build/distance-max.png
xdotool mousemove --window "$graphics_window" 788 214 click 1
xdotool type --clearmodifiers '8'
xdotool key Return
sleep 0.5
import -window "$graphics_window" tools/build/distance-min.png
xdotool mousemove --window "$graphics_window" 640 252 mousedown 1
xdotool mousemove --window "$graphics_window" 700 252
sleep 0.3
xdotool mouseup 1
import -window "$graphics_window" tools/build/distance-slider.png
xdotool mousemove --window "$graphics_window" 788 214 click 1
xdotool type --clearmodifiers '64'
xdotool key Return
sleep 1
rg '"render_distance": 64' tools/build/graphics-config/hither-sdf/settings.json
xdotool mousemove --window "$graphics_window" 820 169 click 1
sleep 1
import -window "$graphics_window" tools/build/graphics-low-menu.png
xdotool mousemove --window "$graphics_window" 640 402 click 1
sleep 1
import -window "$graphics_window" tools/build/graphics-msaa2.png
xdotool click 1
sleep 1
xdotool click 1
sleep 1
import -window "$graphics_window" tools/build/graphics-msaa8.png
xdotool click 1
sleep 0.5
xdotool click 1
xdotool mousemove --window "$graphics_window" 640 449 click 1
xdotool mousemove --window "$graphics_window" 640 543 click 1
sleep 2
xdotool key Escape Escape
sleep 0.5
import -window "$graphics_window" tools/build/graphics-low-world.png
xdotool key Escape
sleep 0.3
xdotool mousemove --window "$graphics_window" 640 400 click 1
sleep 0.3
xdotool mousemove --window "$graphics_window" 640 550 click 1
sleep 0.3
xdotool mousemove --window "$graphics_window" 460 169 click 1
sleep 1
import -window "$graphics_window" tools/build/graphics-restored-native.png
sleep 1
if rg -n 'ERROR|panicked' tools/build/graphics-runtime.log; then exit 1; fi
kill "$graphics_game_pid"
wait "$graphics_game_pid" || true
"${HITHER_TEST_BINARY:-target/release/hither-sdf}" > tools/build/graphics-restart.log 2>&1 &
graphics_game_pid=$!
sleep 5
graphics_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
import -window "$graphics_window" tools/build/graphics-restart.png
if rg -n 'ERROR|panicked' tools/build/graphics-restart.log; then exit 1; fi
