#!/usr/bin/env bash
# Fixed camera views on an isolated display. Never changes player settings.
# HITHER_VISIBILITY_MODE=legacy|depth|gpu selects a same-build comparison.
set -euo pipefail
cd "$(dirname "$0")/.."
case_name="${1:-wall}"
case "$case_name" in
  wall) pose='0,1.65,3.15,1.5707963,0' ;;
  doorway) pose='0,1.65,2.5,3.1415927,0' ;;
  above) pose='0,5,3.15,1.5707963,0' ;;
  plains|forest|tundra|boreal|mountains|orcs) pose='' ;;
  *) echo 'Expected wall, doorway, above, plains, forest, tundra, boreal, mountains, or orcs' >&2; exit 2 ;;
esac
pose="${HITHER_TEST_POSE:-$pose}"
label="${2:-culled}"
output="$PWD/tools/build/occlusion/$case_name-$label"
mkdir -p "$output/config/hither-sdf"
cat > "$output/config/hither-sdf/settings.json" <<JSON
{"max_fps":1000,"grass_blades":${HITHER_TEST_GRASS_BLADES:-true},"show_fps":false,"display_mode":"${HITHER_TEST_DISPLAY_MODE:-windowed}","mouse_sensitivity":1.0,"anti_aliasing":"${HITHER_TEST_AA:-fxaa}","shadow_quality":"${HITHER_TEST_SHADOW_QUALITY:-medium}","resolution":${HITHER_TEST_RESOLUTION:-0},"render_distance":${HITHER_TEST_RENDER_DISTANCE:-48},"detail_distance":${HITHER_TEST_DETAIL_DISTANCE:-24}}
JSON
test ! -e /tmp/.X93-lock
tools/vendor/xvfb/usr/bin/Xvfb :93 -screen 0 "${HITHER_TEST_SCREEN:-1280x720x24}" -nolisten tcp > "$output/display.log" 2>&1 &
display_pid=$!
game_pid=''
trap 'if [[ -n "$game_pid" ]]; then kill "$game_pid" 2>/dev/null || true; wait "$game_pid" 2>/dev/null || true; fi; kill "$display_pid" 2>/dev/null || true; wait "$display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:93 BEVY_ASSET_ROOT="${HITHER_TEST_ASSET_ROOT:-$PWD}" XDG_CONFIG_HOME="$output/config"
export HITHER_PROFILE_FOREST=1 HITHER_PROFILE_SPEED="${HITHER_PROFILE_SPEED:-0}" HITHER_WORLD_SEED="${HITHER_WORLD_SEED:-1}"
unset HITHER_PREVIEW_BIOME HITHER_PROFILE_POSE
if [[ -n "$pose" ]]; then export HITHER_PROFILE_POSE="$pose"; else export HITHER_PREVIEW_BIOME="$case_name"; fi
sleep 0.5
"${HITHER_TEST_BINARY:-target/release/hither-sdf}" > "$output/runtime.log" 2>&1 &
game_pid=$!
if [[ -n "${HITHER_TEST_WINDOW_WIDTH:-}" ]]; then
    window=$(xdotool search --sync --onlyvisible --pid "$game_pid" | head -n 1)
    xdotool windowsize "$window" "$HITHER_TEST_WINDOW_WIDTH" "${HITHER_TEST_WINDOW_HEIGHT:?}"
fi
if [[ "${HITHER_PROFILE_OFFSCREEN:-0}" != "1" ]]; then
    sleep "${HITHER_TEST_CAPTURE_SECONDS:-12}"
    kill -0 "$game_pid"
    window=$(xdotool search --onlyvisible --pid "$game_pid" | head -n 1)
    import -window "$window" "$output/view.png"
fi
wait "$game_pid"
game_pid=''
rg 'World seed|PROFILE|TIMING|ERROR|panicked' "$output/runtime.log"
if rg -q 'ERROR|panicked' "$output/runtime.log"; then exit 1; fi
