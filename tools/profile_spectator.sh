#!/usr/bin/env bash
# Full world renderer, real spectator mode, no OS window or swapchain.
set -euo pipefail
cd "$(dirname "$0")/.."
biome="${1:-boreal}"
speed="${2:-40}"
label="${3:-latest}"
profile_root="$PWD/tools/build/spectator-${biome}-${speed}-${label}"
mkdir -p "$profile_root/config/hither-sdf"
python3 - "$profile_root/config/hither-sdf/settings.json" <<'PY'
import json, os, sys
settings = {
    "max_fps": 1000, "show_fps": False, "display_mode": "windowed",
    "resolution": int(os.environ.get("HITHER_PROFILE_RESOLUTION", 12)),
    "anti_aliasing": "fxaa", "texture_quality": "high", "shadow_quality": "high",
    "grass_blades": True,
    "render_distance": float(os.environ.get("HITHER_PROFILE_RENDER_DISTANCE", 260)),
    "detail_distance": float(os.environ.get("HITHER_PROFILE_DETAIL_DISTANCE", 35)),
}
with open(sys.argv[1], "w") as f:
    json.dump(settings, f, indent=2)
PY
timeout 120s env \
    BEVY_ASSET_ROOT="${HITHER_PROFILE_ASSET_ROOT:-$PWD}" XDG_CONFIG_HOME="$profile_root/config" \
    HITHER_PROFILE_OFFSCREEN=1 HITHER_PROFILE_FOREST=1 HITHER_PROFILE_SPECTATOR=1 \
    HITHER_PROFILE_SPEED="$speed" HITHER_PROFILE_HEIGHT="${HITHER_PROFILE_HEIGHT:-8}" \
    HITHER_WORLD_SEED="${HITHER_WORLD_SEED:-721}" HITHER_PREVIEW_BIOME="$biome" \
    "${HITHER_PROFILE_BINARY:-target/release/hither-sdf}" > "$profile_root/runtime.log" 2>&1
rg '^PROFILE' "$profile_root/runtime.log"
if rg -n 'ERROR|panicked' "$profile_root/runtime.log"; then exit 1; fi
rg -q '^PROFILE moving:' "$profile_root/runtime.log"
