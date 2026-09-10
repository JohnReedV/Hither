#!/usr/bin/env bash
# Lighting fixtures, isolated settings and Xvfb; never modifies player settings.
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-item}"
export HITHER_TEST_BINARY="${HITHER_TEST_BINARY:-$PWD/target/release/hither-sdf}"
export HITHER_TEST_CAPTURE_SECONDS=16 HITHER_PROFILE_WARMUP=12
case "$mode" in
  item|dark|stress)
    export HITHER_WORLD_SEED=2 HITHER_TEST_POSE='0,1.65,3.15,0,-0.1'
    case "$mode" in
      item) export HITHER_LIGHTING_TEST=1 ;;
      dark) export HITHER_LIGHTING_TEST=dark ;;
      stress) export HITHER_LIGHTING_TEST=128 ;;
    esac
    bash tools/smoke_occlusion.sh wall "lighting-$mode"
    ;;
  forest)
    unset HITHER_LIGHTING_TEST
    HITHER_WORLD_SEED=721 bash tools/smoke_occlusion.sh forest lighting-forest
    ;;
  den)
    unset HITHER_LIGHTING_TEST
    HITHER_WORLD_SEED=721 HITHER_DEN_BINARY="$HITHER_TEST_BINARY" bash tools/smoke_goblin_dens.sh
    ;;
  *) echo 'Expected item, dark, stress, forest, or den' >&2; exit 2 ;;
esac
