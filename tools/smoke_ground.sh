#!/usr/bin/env bash
# Reproducible ground-only views on smoke_occlusion's isolated Xvfb display.
set -euo pipefail
cd "$(dirname "$0")/.."
surface="${1:-alpine}"
case "$surface" in
  plains) seed=1; pose='144,1.25,160,0,-0.6'; biome=plains ;;
  alpine) seed=721; pose='-1408,25,460,0,-0.3'; biome=mountains ;;
  cliff) seed=721; pose='-1348.511,142.18398,516.35846,2.149797,-0.40'; biome=mountains ;;
  snow) seed=721; pose='0,1.25,0,0,-0.6'; biome=tundra ;;
  litter) seed=1; pose='-128,5.050044,-128,0,-0.6'; biome=forest ;;
  *) echo 'Expected plains, alpine, cliff, snow, or litter' >&2; exit 2 ;;
esac
export HITHER_WORLD_SEED="$seed" HITHER_TEST_POSE="$pose"
export HITHER_TEST_GRASS_BLADES=false
export HITHER_TEST_RENDER_DISTANCE="${HITHER_TEST_RENDER_DISTANCE:-48}"
export HITHER_TEST_CAPTURE_SECONDS="${HITHER_TEST_CAPTURE_SECONDS:-35}"
export HITHER_PROFILE_WARMUP="${HITHER_PROFILE_WARMUP:-37}"
if [[ "${HITHER_GROUND_4K:-0}" == 1 ]]; then
    export HITHER_TEST_SCREEN=3840x2160x24 HITHER_TEST_WINDOW_WIDTH=3840 HITHER_TEST_WINDOW_HEIGHT=2160
    export HITHER_TEST_RESOLUTION=12
fi
bash tools/smoke_occlusion.sh "$biome" "ground-$surface-${2:-check}"
