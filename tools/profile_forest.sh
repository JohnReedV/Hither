#!/usr/bin/env bash
# Isolated native benchmark; no changes to the player's settings or running game.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/profile-config/hither-sdf
cp tools/profile-settings.json tools/build/profile-config/hither-sdf/settings.json
test ! -e /tmp/.X94-lock
"${HITHER_XVFB:-tools/vendor/xvfb/usr/bin/Xvfb}" :94 -screen 0 1280x720x24 -nolisten tcp > tools/build/profile-xvfb.log 2>&1 &
profile_display_pid=$!
profile_game_pid=""
trap 'if [[ -n "$profile_game_pid" ]]; then kill "$profile_game_pid" 2>/dev/null || true; wait "$profile_game_pid" 2>/dev/null || true; fi; kill "$profile_display_pid" 2>/dev/null || true; wait "$profile_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:94 BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/profile-config"
sleep 0.5
HITHER_PROFILE_FOREST=1 HITHER_WORLD_SEED=721 HITHER_PREVIEW_BIOME="${1:-forest}" target/release/hither-sdf > "tools/build/profile-${1:-forest}-${2:-latest}.log" 2>&1 &
profile_game_pid=$!
for attempt in $(seq 1 30); do
    kill -0 "$profile_game_pid"
    profile_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1 || true)
    if [[ -n "$profile_window" ]]; then xdotool windowfocus --sync "$profile_window"; break; fi
    sleep 0.1
done
wait "$profile_game_pid"
profile_game_pid=""
rg 'PROFILE|ERROR|panicked' "tools/build/profile-${1:-forest}-${2:-latest}.log"
if rg -q 'ERROR|panicked' "tools/build/profile-${1:-forest}-${2:-latest}.log"; then exit 1; fi
