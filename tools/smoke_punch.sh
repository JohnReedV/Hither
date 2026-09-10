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
sleep "${HITHER_SMOKE_WAIT:-100}"
hand_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
xdotool windowfocus --sync "$hand_window"
ffmpeg -y -loglevel error -threads 2 -f x11grab -framerate 60 -video_size 1280x720 -i :93 -t 3 -c:v libx264 -preset ultrafast -crf 18 -pix_fmt yuv420p tools/build/hand-punch.mp4 > tools/build/hand-punch-capture.log 2>&1 &
hand_capture_pid=$!
sleep 0.5
xdotool click 1
sleep 0.75
xdotool click 1
sleep 0.75
xdotool click 1
wait "$hand_capture_pid"
# Freeze mid-strike, then verify the visible fist stays still beneath the menu.
xdotool click 1
sleep 0.08
xdotool key Escape
sleep 0.3
import -window "$hand_window" tools/build/punch-paused-a.png
sleep 0.3
import -window "$hand_window" tools/build/punch-paused-b.png
python3 - <<'CHECK'
from PIL import Image, ImageChops
a=Image.open('tools/build/punch-paused-a.png').crop((450,390,1050,720))
b=Image.open('tools/build/punch-paused-b.png').crop((450,390,1050,720))
assert ImageChops.difference(a,b).getbbox() is None, 'Paused punch moved'
CHECK
xdotool key Escape
sleep 0.7
import -window "$hand_window" tools/build/punch-recovered.png
python3 - <<'CHECK'
from PIL import Image, ImageStat
image = Image.open('tools/build/punch-recovered.png').convert('RGB')
assert max(ImageStat.Stat(image).stddev) > 3, 'Renderer has not produced a visible scene'
CHECK
# F3 cycles first -> third -> second; capture the body overlay in both views.
for mode in third second; do
    xdotool key F3
    sleep 0.5
    ffmpeg -y -loglevel error -threads 2 -f x11grab -framerate 60 -video_size 1280x720 -i :93 -t 3 -c:v libx264 -preset ultrafast -crf 18 -pix_fmt yuv420p "tools/build/$mode-person-punch.mp4" &
    hand_capture_pid=$!
    sleep 0.4
    xdotool click 1
    sleep 0.95
    xdotool keydown w
    xdotool click 1
    sleep 0.5
    xdotool keyup w
    wait "$hand_capture_pid"
done
if rg -n 'ERROR|panicked'  tools/build/hand-runtime.log; then exit 1; fi
