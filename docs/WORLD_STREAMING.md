# World streaming

`world::streaming::StreamingPlugin` coordinates terrain, grass, citrus, conifers,
oaks, alpine trees, rocks, understory, fallen timber, goblin dens/residents, orc
dens and rock-contact baking. Feature plugins register adapters in its
`Layer` sets. Generators and resident entity/mesh maps stay in their feature
modules; there is no second copy of the world or its gameplay state.

The projected rendering extension adds measured preparation budgets, shared
visual priorities, surface caches and independent shadow policy. See
[PROJECTED_RENDERING.md](PROJECTED_RENDERING.md) for the current architecture.
The dated validation below describes the earlier streaming baseline.

## Shared limits

The coordinator resets once per Update, after camera movement and shader inputs
and before any streaming adapter. Adapters share 64 weighted work units per
frame, with at most 32 for one layer. A tree/resident costs one unit, a terrain
or grass upload/vegetation chunk four, a timber chunk/orc den eight, and a goblin
settlement sixteen. These are admission weights, **not millisecond guarantees**:
chunk complexity, deferred ECS commands and GPU uploads vary.

Sixteen units are reserved for a rotating waiting layer, preventing starvation
from fixed system ordering. Unused credit expires. Original local batch limits
remain as additional safeguards.

Terrain, grass, timber, dens and rock contacts share twelve background slots.
One slot is reserved for a rotating waiting layer. A `BuildTask` keeps its slot
through computation and while awaiting installation. Workers capture their
permits: cancelling an obsolete task cannot falsely free capacity while its
synchronous computation still runs. Ready results that cannot fit the current
installation budget stay pending. A consumed completion is never polled again.

Eviction is never gated by admission credit. Terrain retains immediate procedural
fallback tiles while baked meshes wait, so loading pressure cannot open ground
holes. Collision and support queries remain independent of visible residency.

## Render distance

Adapters use the current render-distance setting directly (terrain) or through
`view_distance::Range`. The coordinator refreshes that envelope before adapters,
including the actual player/camera separation with a minimum four-metre allowance.
Detail distance can reduce fine geometry but cannot extend the visible radius.
The existing SDF/depth and grass radial clipping enforce the exact camera-space
visible boundary.

Chunks are conservative loading regions: cell corners, crowns, den extents and
small eviction margins can remain resident beyond the visible radius without
extending rendering. Analytic shader surfaces do not require regional mesh generation.
Finite shared model/material banks remain reusable assets; visible instances
still obey the common depth cutoff. Active orcs retain their
home until returning, preserving encounter rules across visual boundaries.

Range changes and teleports prune obsolete work before further installations.
Grass also rejects results for obsolete detail levels. Dens process completions
each frame instead of releasing a half-second batch. Goblin residents install
incrementally without restarting an already populated den. Fallen timber meshes
build on workers and are explicitly released on eviction.

A shared rectangle-difference iterator supplies entering cells for citrus,
conifers, alpine trees, rocks, understory and timber. These retain overlapping
queued work instead of rescanning an entire window at each crossing. Terrain
and grass still inspect their windows on discrete position/detail changes to
refresh the detail requirements of retained cells.

## Validation

Coordinator tests cover aggregate admission, starvation, worker/result lifetime,
negative coordinates, shrinking/growing windows, teleports and applying a
1024-to-16-metre change before adapters. Feature tests cover deterministic revisits,
mesh reclamation, detail changes, stale jobs and terrain fallback coverage.

Use the full-renderer `tools/profile_spectator.sh` routes at 20/40 m/s and the
graphics distance smoke test. Compare matched source, assets, settings and hardware.
Fewer admitted chunks per frame alone is not evidence of improved performance:
check loading convergence and visible coverage alongside median/p95/p99 timings.

### Verification, September 6, 2026

- Current-source targeted suite: **96 passed, 1 ignored**, covering streaming,
  terrain, vegetation, geology, orchard, goblins, rendering and visible den loading.
  The optimized diagnostic test executable used fast linking, with debug assertions
  and overflow checks enabled. Log: `tools/build/streaming-final-regressions.log`.
- `cargo check --offline`, formatting, and the shipping `cargo build --offline
  --release` passed. Shipping retains optimization level 3, fat LTO and one codegen
  unit; no Cargo profile settings were changed.
- The shipping executable passed a forest UI/render check at **1024 -> 16 -> 64 m**.
  Screenshots at both extremes were inspected; scenery disappears and returns
  with the setting. Artifacts: `tools/build/streaming-distance/`.
- A diagnostic boreal 40 m/s flight at 260 m and a shipping mountain 40 m/s flight
  at 512 m completed without renderer errors. These are functional smoke checks,
  not an uncontended paired performance benchmark. No FPS improvement is claimed.
- The broader graphics script exposed a resolution-switch depth-copy error
  (`Depth32Float`, 1280 -> 320). The saved **pre-change executable reproduced the
  same error**. It is separate from render-distance changes and remains unfixed.
  Reproduction logs: `tools/build/streaming-resolution-baseline/`.
- Strict Clippy remains blocked by existing/unrelated warnings and lint findings.
  The broad early-snapshot suite was not a clean pass: cave-art and orc encounter
  assertions failed, and the run reached its 300-second limit. Neither is counted
  as passing validation of the complete game.
