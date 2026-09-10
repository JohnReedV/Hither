#!/usr/bin/env bash
# Hidden native renderer check. Never uses or changes the desktop display.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/mountain-config/hither-sdf
cat > tools/build/mountain-config/hither-sdf/settings.json <<'JSON'
{"max_fps":30,"show_fps":false,"display_mode":"windowed","render_distance":512.0,"detail_distance":24.0,"resolution":0}
JSON
mountain_display=":${HITHER_TEST_DISPLAY:-97}"
test ! -e "/tmp/.X${mountain_display#:}-lock"
tools/vendor/xvfb/usr/bin/Xvfb "$mountain_display" -screen 0 1280x720x24 -nolisten tcp > tools/build/mountain-xvfb.log 2>&1 &
mountain_display_pid=$!
mountain_game_pid=""
trap 'if [[ -n "$mountain_game_pid" ]]; then kill "$mountain_game_pid" 2>/dev/null || true; wait "$mountain_game_pid" 2>/dev/null || true; fi; kill "$mountain_display_pid" 2>/dev/null || true; wait "$mountain_display_pid" 2>/dev/null || true' EXIT
export DISPLAY="$mountain_display" WINIT_UNIX_BACKEND=x11
unset WAYLAND_DISPLAY
export BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/mountain-config"
export HITHER_WORLD_SEED="${HITHER_WORLD_SEED:-721}" HITHER_PREVIEW_BIOME=mountains
sleep 1
"${HITHER_TEST_BINARY:-target/release/hither-sdf}" > tools/build/mountain-runtime.log 2>&1 &
mountain_game_pid=$!
sleep "${HITHER_SMOKE_WAIT:-55}"
kill -0 "$mountain_game_pid"
mountain_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
import -window "$mountain_window" tools/build/mountain-valley.png
if (( $(identify -format '%k' tools/build/mountain-valley.png) < 100 )); then
    printf 'Renderer has not produced a scene yet; increase HITHER_SMOKE_WAIT.\n' >&2
    exit 1
fi
if rg -n 'ERROR|panicked|Shader.*error' tools/build/mountain-runtime.log; then exit 1; fi
printf 'Hidden mountain render captured: tools/build/mountain-valley.png\n'
