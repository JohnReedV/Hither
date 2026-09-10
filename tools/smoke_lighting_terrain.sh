#!/usr/bin/env bash
# Distant alpine receiver/depth regression; isolated display and player settings.
set -euo pipefail
cd "$(dirname "$0")/.."
export HITHER_WORLD_SEED=21
export HITHER_TEST_RENDER_DISTANCE=496 HITHER_TEST_DETAIL_DISTANCE=205
export HITHER_TEST_SHADOW_QUALITY="${HITHER_TEST_SHADOW_QUALITY:-high}"
export HITHER_TEST_POSE='-4984,500,18280,2.6575,-0.837'
export HITHER_PROFILE_SPECTATOR=1 HITHER_PROFILE_SPEED=0
export HITHER_TEST_CAPTURE_SECONDS="${HITHER_TEST_CAPTURE_SECONDS:-30}"
export HITHER_PROFILE_WARMUP=32
bash tools/smoke_occlusion.sh wall "${1:-lighting-terrain-regression}"
