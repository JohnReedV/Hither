#!/usr/bin/env bash
# Run after cargo build --release. Exercise real player controls in each den.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/den-safety-config/hither-sdf
cp tools/smoke-settings.json tools/build/den-safety-config/hither-sdf/settings.json
test ! -e /tmp/.X96-lock
tools/vendor/xvfb/usr/bin/Xvfb :96 -screen 0 1280x720x24 -nolisten tcp > tools/build/den-safety-xvfb.log 2>&1 &
den_display_pid=$!
den_game_pid=""
trap 'if [[ -n "$den_game_pid" ]]; then kill "$den_game_pid" 2>/dev/null || true; wait "$den_game_pid" 2>/dev/null || true; fi; kill "$den_display_pid" 2>/dev/null || true; wait "$den_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:96 BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/den-safety-config"
export HITHER_PREVIEW_BIOME=orcs HITHER_WORLD_SEED=721 HITHER_DEN_BOTTOM=1
for den_kind in ${HITHER_SMOKE_KINDS:-0 1 2}; do
    HITHER_DEN_KIND="$den_kind" "${HITHER_SMOKE_BINARY:-target/release/hither-sdf}" > "tools/build/den-safety-${den_kind}.log" 2>&1 &
    den_game_pid=$!
    sleep "${HITHER_SMOKE_WAIT:-8}"
    den_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
    xdotool windowfocus --sync "$den_window"
    xdotool key F3
    sleep 0.5 # Let the camera-mode change render before taking the capture.
    import -window "$den_window" "tools/build/den-safety-${den_kind}-inside.png"
    # Spawn faces toward the rear, so S walks up the ramp without jumping.
    xdotool keydown s
    sleep 4
    xdotool keyup s
    sleep 0.5
    import -window "$den_window" "tools/build/den-safety-${den_kind}-outside.png"
    xdotool keydown w
    sleep 3.5
    xdotool keyup w
    sleep 1
    import -window "$den_window" "tools/build/den-safety-${den_kind}-returned.png"
    # Look back toward pursuing residents while pressing against each bank.
    # All input is confined to DISPLAY=:96, never the desktop display.
    xdotool key F3
    xdotool keydown a
    sleep 1
    xdotool keyup a
    sleep 3
    import -window "$den_window" "tools/build/den-safety-${den_kind}-left-bank.png"
    xdotool keydown d
    sleep 2
    xdotool keyup d
    sleep 3
    import -window "$den_window" "tools/build/den-safety-${den_kind}-right-bank.png"
    # Exit directly from the side-bank lane without steering toward the center.
    xdotool keydown s
    sleep 4
    xdotool keyup s
    sleep 0.5
    import -window "$den_window" "tools/build/den-safety-${den_kind}-side-exit.png"
    if rg -n 'ERROR|panicked' "tools/build/den-safety-${den_kind}.log"; then exit 1; fi
    kill "$den_game_pid"
    wait "$den_game_pid" || true
    den_game_pid=""
done
