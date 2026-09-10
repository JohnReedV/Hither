#!/usr/bin/env bash
# Native-display benchmark: opt-in temporary window, isolated settings, auto-exit.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/profile-config/hither-sdf
if [[ "${HITHER_PROFILE_FULLSCREEN:-0}" == "1" ]]; then
    cp tools/profile-fullscreen-settings.json tools/build/profile-config/hither-sdf/settings.json
else
    cp tools/profile-settings.json tools/build/profile-config/hither-sdf/settings.json
fi
export BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/profile-config"
profile_game_pid=""
trap 'if [[ -n "$profile_game_pid" ]]; then kill "$profile_game_pid" 2>/dev/null || true; wait "$profile_game_pid" 2>/dev/null || true; fi' EXIT
HITHER_PROFILE_FOREST=1 HITHER_WORLD_SEED="${HITHER_WORLD_SEED:-721}" HITHER_PREVIEW_BIOME="${1:-forest}" target/release/hither-sdf > "tools/build/native-${1:-forest}-${2:-latest}.log" 2>&1 &
profile_game_pid=$!
for attempt in $(seq 1 50); do
    profile_window=$(xdotool search --onlyvisible --pid "$profile_game_pid" | head -n 1 || true)
    if [[ -n "$profile_window" ]]; then
        xdotool windowfocus --sync "$profile_window"
        if [[ -n "${HITHER_PROFILE_WIDTH:-}" ]]; then
            xdotool windowsize "$profile_window" "$HITHER_PROFILE_WIDTH" "${HITHER_PROFILE_HEIGHT:?height required}"
        fi
        xdotool getwindowgeometry "$profile_window"
        break
    fi
    sleep 0.1
done
wait "$profile_game_pid"
profile_game_pid=""
rg 'PROFILE|ERROR|panicked' "tools/build/native-${1:-forest}-${2:-latest}.log"
if rg -q 'ERROR|panicked' "tools/build/native-${1:-forest}-${2:-latest}.log"; then exit 1; fi
