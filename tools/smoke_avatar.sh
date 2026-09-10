#!/usr/bin/env bash
# Optional local visual smoke test. Requires Xvfb, xdotool and ImageMagick.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/test-config/hither-sdf
cp tools/smoke-settings.json tools/build/test-config/hither-sdf/settings.json
avatar_xvfb="${HITHER_XVFB:-tools/vendor/xvfb/usr/bin/Xvfb}"
test ! -e /tmp/.X92-lock
"$avatar_xvfb" :92 -screen 0 1280x720x24 -nolisten tcp > tools/build/xvfb.log 2>&1 &
avatar_display_pid=$!
avatar_game_pid=""
trap 'if [[ -n "$avatar_game_pid" ]]; then kill "$avatar_game_pid" 2>/dev/null || true; fi; kill "$avatar_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:92
export BEVY_ASSET_ROOT="$PWD"
export XDG_CONFIG_HOME="$PWD/tools/build/test-config"
sleep 0.5
target/release/hither-sdf > tools/build/smoke-runtime.log 2>&1 &
avatar_game_pid=$!
sleep 5
avatar_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
xdotool windowfocus --sync "$avatar_window"
import -window "$avatar_window" tools/build/smoke-first.png
if [[ "${HITHER_SMOKE_CHAT:-0}" == "1" ]]; then
    xdotool key t
    xdotool type --clearmodifiers --delay 20 'Hello from the courtyard!'
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-typing.png
    xdotool key Return
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-sent.png
    xdotool key t
    xdotool type --clearmodifiers --delay 10 'Second message'
    xdotool key Return
    xdotool key t
    xdotool type --clearmodifiers --delay 10 'Unfinished draft'
    xdotool key Up
    sleep 0.2
    import -window "$avatar_window" tools/build/chat-recall-newest.png
    xdotool key Up
    sleep 0.2
    import -window "$avatar_window" tools/build/chat-recall-oldest.png
    xdotool key Down Down
    sleep 0.2
    import -window "$avatar_window" tools/build/chat-recall-draft.png
    xdotool key Escape
    for index in $(seq 1 14); do
        xdotool key t
        xdotool type --clearmodifiers --delay 1 "Scroll test message $index"
        xdotool key Return
    done
    xdotool key t
    xdotool mousemove --window "$avatar_window" 200 580
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-scroll-bottom.png
    xdotool click --repeat 20 --delay 30 4
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-scroll-top.png
    xdotool click --repeat 20 --delay 30 5
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-scroll-restored.png
    xdotool key Escape
    xdotool key t
    xdotool type --clearmodifiers --delay 10 'wasd e '
    xdotool key F3 space
    xdotool mousemove_relative -- 80 30
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-controls-blocked.png
    xdotool key Escape
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-cancelled.png
    xdotool key Escape
    sleep 0.3
    import -window "$avatar_window" tools/build/chat-pause.png
    xdotool key Escape
    sleep 0.2
fi
if [[ "${HITHER_SMOKE_SPECTATOR:-0}" == "1" ]]; then
    xdotool key slash
    sleep 0.2
    import -window "$avatar_window" tools/build/spectator-slash.png
    xdotool type --clearmodifiers --delay 20 's'
    xdotool key Tab
    sleep 0.2
    import -window "$avatar_window" tools/build/command-completion.png
    xdotool key Return
    sleep 0.2
    import -window "$avatar_window" tools/build/spectator-enabled.png
    xdotool keydown space sleep 0.35 keyup space
    xdotool keydown s sleep 0.5 keyup s
    xdotool mousedown 1 keydown space sleep 0.2 keyup space mouseup 1
    xdotool mousemove_relative -- 0 180
    sleep 0.3
    import -window "$avatar_window" tools/build/spectator-flight.png
    xdotool keydown Shift_L sleep 0.3 keyup Shift_L
    xdotool key slash
    xdotool type --clearmodifiers --delay 20 'spectate'
    xdotool key Escape
    sleep 0.2
    import -window "$avatar_window" tools/build/spectator-cancel.png
    xdotool key slash
    xdotool type --clearmodifiers --delay 20 'spectate'
    xdotool key Return
    sleep 2.5
    import -window "$avatar_window" tools/build/spectator-return.png
    xdotool mousemove_relative -- 0 -180
fi
xdotool key F3 keydown w sleep 0.25 keyup w sleep 0.15
import -window "$avatar_window" tools/build/smoke-third.png
xdotool key F3 sleep 0.15
import -window "$avatar_window" tools/build/smoke-front.png
xdotool mousemove_relative -- 240 0
sleep 0.16
import -window "$avatar_window" tools/build/smoke-turn-right.png
xdotool mousemove_relative -- -480 0
sleep 0.50
import -window "$avatar_window" tools/build/smoke-turn-left.png
xdotool mousemove_relative -- 240 0
sleep 0.8
xdotool keydown a sleep 0.18
import -window "$avatar_window" tools/build/smoke-strafe.png
xdotool keyup a keydown space sleep 0.20
import -window "$avatar_window" tools/build/smoke-jump.png
xdotool keyup space sleep 0.9 key Escape sleep 0.1
import -window "$avatar_window" tools/build/smoke-pause.png
xdotool key Escape key F3 sleep 0.1
import -window "$avatar_window" tools/build/smoke-return-first.png
if rg -n 'ERROR|panicked' tools/build/smoke-runtime.log; then exit 1; fi
echo 'Visual smoke frames saved to tools/build/smoke-*.png'
