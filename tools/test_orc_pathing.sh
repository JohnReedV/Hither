#!/usr/bin/env bash
# Run the full orc/navigation suite, including focused pursuit regressions.
set -euo pipefail
cd "$(dirname "$0")/.."
# Time-sliced planning is tested with optimized code and runtime assertions.
# These overrides are local to this runner; the project's dev profile is untouched.
export CARGO_PROFILE_TEST_OPT_LEVEL=3 CARGO_PROFILE_TEST_LTO=false
export CARGO_PROFILE_TEST_CODEGEN_UNITS=16 CARGO_PROFILE_TEST_INCREMENTAL=false
export CARGO_PROFILE_TEST_DEBUG=false
mkdir -p tools/build
cargo test -- world::navigation:: world::orcs:: --list > tools/build/orc-pathing-tests.txt
pathing_count=$(rg -c ': test$' tools/build/orc-pathing-tests.txt)
printf 'Running %s orc/navigation tests.\n' "$pathing_count"
# Den tests publish gate states globally; keep the complete suite serial.
cargo test -- world::navigation:: world::orcs:: --test-threads=1 "$@"
