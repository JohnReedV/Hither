#!/usr/bin/env bash
# Isolated native Bevy screenshots; never touches the player's saved settings.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/test-config/hither-sdf
cp tools/smoke-settings.json tools/build/test-config/hither-sdf/settings.json
test ! -e /tmp/.X93-lock
"${HITHER_XVFB:-tools/vendor/xvfb/usr/bin/Xvfb}" :93 -screen 0 1280x720x24 -nolisten tcp > tools/build/orchard-xvfb.log 2>&1 &
orchard_display_pid=$!
orchard_game_pid=""
trap 'if [[ -n "$orchard_game_pid" ]]; then kill "$orchard_game_pid" 2>/dev/null || true; wait "$orchard_game_pid" 2>/dev/null || true; fi; kill "$orchard_display_pid" 2>/dev/null || true; wait "$orchard_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:93
export BEVY_ASSET_ROOT="$PWD"
export XDG_CONFIG_HOME="$PWD/tools/build/test-config"
sleep 0.5
target/release/hither-sdf > tools/build/orchard-runtime.log 2>&1 &
orchard_game_pid=$!
sleep 1
orchard_window=""
for attempt in $(seq 1 40); do
    kill -0 "$orchard_game_pid"
    orchard_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1 || true)
    if [[ -n "$orchard_window" ]]; then
        import -window "$orchard_window" tools/build/orchard-ready.png
        if convert tools/build/orchard-ready.png -format '%[fx:mean]' info: | awk '{exit !($1 > 0.01)}'; then break; fi
    fi
    sleep 1
done
convert tools/build/orchard-ready.png -format '%[fx:mean]' info: | awk '{exit !($1 > 0.01)}'
orchard_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
xdotool windowfocus --sync "$orchard_window"
if [[ "${HITHER_SMOKE_GRASS:-0}" == "1" ]]; then
    xdotool mousemove_relative -- 0 115
    sleep 1
    import -window "$orchard_window" tools/build/grass-courtyard.png
    xdotool key F3 F3
    sleep 1
    import -window "$orchard_window" tools/build/grass-character.png
    xdotool key F3
    xdotool mousemove_relative -- 0 -115
fi
xdotool keydown s sleep 0.18 keyup s
xdotool mousemove_relative -- 0 -35
sleep 0.5
import -window "$orchard_window" tools/build/orchard-overview.png
xdotool mousemove_relative -- 0 -15
xdotool keydown w sleep 0.30 keyup w
sleep 0.5
import -window "$orchard_window" tools/build/orchard-detail.png
if [[ "${HITHER_SMOKE_WILD:-0}" == "1" ]]; then
    # Back through the open gate, then survey both sides of the outer world.
    xdotool mousemove_relative -- 0 35
    xdotool keydown s sleep 6 keyup s
    sleep 1
    import -window "$orchard_window" tools/build/orange-world.png
    if [[ "${HITHER_SMOKE_GRASS:-0}" == "1" ]]; then
        xdotool mousemove_relative -- 0 100
        sleep 1
    import -window "$orchard_window" tools/build/grass-world.png
        if [[ "${HITHER_SMOKE_SNOW:-0}" == "1" ]]; then
            cp tools/build/grass-world.png tools/build/snow-tracks.png
            # Standing still must not refresh footprints; they expire at 30s.
            sleep 31
            import -window "$orchard_window" tools/build/snow-tracks-expired.png
        fi
        xdotool mousemove_relative -- 0 -100
    fi
    # Step around the seed-721 tree beside the gate path before continuing.
    xdotool windowfocus --sync "$orchard_window"
    xdotool keydown d sleep 1 keyup d
    # Seed 721 has a citrus tree at (3.5, 22.5), directly ahead here.
    xdotool mousemove_relative -- 0 -50
    sleep 0.5
    import -window "$orchard_window" tools/build/orange-close.png
    xdotool mousemove_relative -- 0 50
    xdotool keydown s sleep 5 keyup s
    xdotool mousemove_relative -- 180 0
    sleep 1
    import -window "$orchard_window" tools/build/orange-world-side.png
    xdotool windowfocus --sync "$orchard_window"
    xdotool keydown s sleep 5 keyup s
    sleep 1
    import -window "$orchard_window" tools/build/orange-world-streamed.png
fi
if rg -n 'ERROR|panicked' tools/build/orchard-runtime.log; then exit 1; fi
echo 'Tree overview and detail saved in tools/build.'
