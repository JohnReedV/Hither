#!/usr/bin/env bash
# Isolated close-up: two live frames and two paused frames, no desktop input.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p tools/build/torch-config/hither-sdf
python3 - <<'CONFIG'
import json
from pathlib import Path
settings = json.loads(Path('tools/smoke-settings.json').read_text())
settings['show_fps'] = False
Path('tools/build/torch-config/hither-sdf/settings.json').write_text(json.dumps(settings))
CONFIG
test ! -e /tmp/.X96-lock
tools/vendor/xvfb/usr/bin/Xvfb :96 -screen 0 1280x720x24 -nolisten tcp > tools/build/torch-xvfb.log 2>&1 &
torch_display_pid=$!
torch_game_pid=""
trap 'if [[ -n "$torch_game_pid" ]]; then kill "$torch_game_pid" 2>/dev/null || true; wait "$torch_game_pid" 2>/dev/null || true; fi; kill "$torch_display_pid" 2>/dev/null || true; wait "$torch_display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:96 BEVY_ASSET_ROOT="$PWD" XDG_CONFIG_HOME="$PWD/tools/build/torch-config"
export HITHER_WORLD_SEED=721 HITHER_PREVIEW_BIOME=orcs HITHER_TORCH_INSPECT=1
target/release/hither-sdf > tools/build/torch-runtime.log 2>&1 &
torch_game_pid=$!
sleep "${HITHER_SMOKE_WAIT:-15}"
torch_window=$(xdotool search --onlyvisible --name 'Signed Distance Fields' | head -n 1)
xdotool windowfocus --sync "$torch_window"
import -window "$torch_window" tools/build/torch-live-a.png
sleep 0.4
import -window "$torch_window" tools/build/torch-live-b.png
xdotool key Escape
sleep 1
import -window "$torch_window" tools/build/torch-paused-a.png
sleep 1
import -window "$torch_window" tools/build/torch-paused-b.png
if rg -n 'ERROR|panicked' tools/build/torch-runtime.log; then exit 1; fi
