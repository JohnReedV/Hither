#!/usr/bin/env bash
# Isolated-display visual and shader check for generated mountain settlements.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/goblin-den-config/hither-sdf
cp tools/smoke-settings.json tools/build/goblin-den-config/hither-sdf/settings.json
test ! -e /tmp/.X94-lock
tools/vendor/xvfb/usr/bin/Xvfb :94 -screen 0 1280x720x24 -nolisten tcp > tools/build/goblin-den-xvfb.log 2>&1 &
den_display_pid=$!
den_game_pid=""
trap 'if [[ -n "$den_game_pid" ]]; then kill "$den_game_pid" 2>/dev/null || true; wait "$den_game_pid" 2>/dev/null || true; fi; kill "$den_display_pid" 2>/dev/null || true; wait "$den_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:94 BEVY_ASSET_ROOT="${HITHER_DEN_ASSET_ROOT:-$PWD}" XDG_CONFIG_HOME="$PWD/tools/build/goblin-den-config"
RUST_LOG=warn,hither_sdf=info,hither_den_preview=info HITHER_WORLD_SEED="${HITHER_WORLD_SEED:-721}" HITHER_GOBLIN_DEN_PREVIEW="${HITHER_GOBLIN_DEN_PREVIEW:-1}" "${HITHER_DEN_BINARY:-target/release/hither-sdf}" > tools/build/goblin-den-runtime.log 2>&1 &
den_game_pid=$!
for attempt in $(seq 1 90); do
    kill -0 "$den_game_pid"
    if rg -q 'Goblin den .*houses' tools/build/goblin-den-runtime.log; then break; fi
    sleep 1
done
rg -q 'Goblin den .*houses' tools/build/goblin-den-runtime.log
sleep "${HITHER_DEN_SETTLE_SECONDS:-8}"
den_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
import -window "$den_window" tools/build/goblin-den-overview.png
xdotool windowfocus --sync "$den_window"
if [[ "${HITHER_DEN_INSPECT:-0}" == 1 ]]; then
    xdotool key e
    sleep 2
    import -window "$den_window" tools/build/goblin-den-lore.png
fi
xdotool key Escape
sleep 1
import -window "$den_window" tools/build/goblin-den-paused.png
if rg -n 'ERROR|panicked|Failed to load' tools/build/goblin-den-runtime.log; then exit 1; fi
printf 'PASS: generated den rendered and accepted pause input.\n'
