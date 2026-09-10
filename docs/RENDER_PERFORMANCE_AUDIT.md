# Six rendering fixes: implementation and measured results

## Implementation

1. **Forest streaming.** Keep a persistent nearest-first queue. Cell/radius changes
   retain pending work and add only entering strips using the conifer streamer’s
   existing strip iterator. Unchanged frames pop up to four chunks without
   rebuilding a wanted set or sorting the entire window. Shrinking, diagonal
   movement and teleports use the same bounded window logic.
2. **Static shader inputs.** Footprint topology has a revision independent of age;
   bins rebuild only after additions or expiry. Each immutable goblin den lazily
   caches its exposed tunnel segments. A bounded current-window cache retains
   portal and orc candidates; distance filtering remains exact, including nearest
   orc truncation. Stationary cameras reuse completed uniform arrays. Candidate
   keys include cell, radius and seed. Portal exposure is computed outside shared
   placement-cache locks.
3. **Coarse grass.** Consume the same random numbers, then reject discarded blades
   before trigonometry, geometry and terrain sampling. Preserve floating-point
   multiplication order as well as RNG order. The accepted blade geometry is
   unchanged; discarded blades no longer evaluate terrain height or gradient.
4. **Static rock contacts.** Bake world-space ground height and normalized contact
   normals for both mesh LODs in at most four background jobs. Shared vertex IDs
   and per-instance `MeshTag` offsets address one GPU storage buffer, preserving
   shared meshes and material instancing. Upload only completed allocation ranges
   in the render schedule. Both depth and color passes read the same data. Until
   data is ready (or if capacity is exhausted), retain procedural contact shading.
   Visible rocks get priority. Reclaim/coalesce allocations on removal events;
   stale jobs cannot attach results to despawned entities. The buffer is capped at
   64 MiB and also respects the adapter’s storage-binding limit.
5. **LOD work.** Evaluate forest LOD once per tree, with per-chunk spatial rejection
   against old/new detail spheres. Update the four child meshes only on a tree’s
   transition. Retain hysteresis and support vertical movement, detail changes and
   teleports. Rock LOD traversal stops when camera position is unchanged; newly
   streamed rocks receive their initial LOD and visibility at spawn.
6. **Tree texture mipmaps.** Generate full chains through 1×1. Color filtering
   decodes/encodes sRGB around averaging; normal filtering averages signed vectors
   and renormalizes; roughness uses linear scalar filtering. Texture quality selects
   an existing filtered mip and retains its entire tail. Returning to High restores
   the saved original. Both nonsquare and odd-sized images are covered by tests.

A visual check also caught Bevy replacing the procedural terrain fallback’s
conservative height bounds with the undisplaced mesh bounds. Terrain tiles now use
`NoAutoAabb`, retaining explicit bounds until a baked mesh supplies tighter bounds.
This correctness fix is applied to **both** comparison builds, so missing terrain
cannot count as a performance improvement.

## Measurement protocol

- NVIDIA GeForce RTX 5090; 3840×2160 world render target; FXAA; high texture/shadow
  quality; seed 721; 1,000 FPS cap.
- Offscreen rendering without a swapchain or GPU timing instrumentation. Results
  measure this renderer workload, not native display/presentation FPS.
- Castle: fixed camera, 48 m render distance and 24 m detail.
- Forest/boreal: 260 m render distance; mountains: 512 m; all flights use 35 m
  detail and move at 40 m/s along the same route.
- Each fresh process warms up for 20 seconds, records eight stationary seconds,
  then sixteen moving seconds. The castle moving phase keeps its camera fixed.
- Two castle runs and three runs per flight scene, per version. Before/after order
  reverses in the second repetition. The third flight pairs address observed
  streaming variability.
  The harness rejects runs overlapping another game, test executable or Rust
  compiler. Settings and complete runtime logs are retained per run.
- Both builds use identical release dependencies and compiler flags: optimization
  level 3, fat LTO and one codegen unit. `baseline-v2` freezes the current gameplay
  code before these six changes; `optimized-v2` contains the six changes. Earlier
  terrain/shading work is common to both. Unrelated concurrent gameplay edits are
  therefore not counted in this comparison.
- Aggregate mean frame time is the arithmetic average of per-run mean frame times;
  FPS is 1,000 divided by that average. p95 is the average of per-run p95s, not a
  pooled percentile. FPS gain is `(after/before - 1) × 100`; frame-time reduction is
  `(1 - after_ms/before_ms) × 100`. These percentages are not interchangeable.

| Scene | FPS before → after | Mean ms before → after | p95 ms before → after | FPS gain |
|---|---:|---:|---:|---:|
| castle | 444.6 → 434.0 | 2.25 → 2.30 | 2.70 → 2.71 | -2.4% |
| forest | 101.0 → 102.6 | 9.90 → 9.74 | 12.27 → 12.19 | +1.6% |
| boreal | 118.1 → 125.3 | 8.47 → 7.98 | 19.40 → 10.88 | +6.1% |
| mountains | 103.3 → 126.0 | 9.68 → 7.93 | 24.23 → 13.97 | +22.1% |

Equal-weight geometric mean FPS change across the four scenes: **+6.4%**.

Individual moving-phase FPS (castle uses its fixed view in this phase):

| Scene | Before runs (FPS) | After runs (FPS) |
|---|---|---|
| castle | 442.9, 446.3 | 433.1, 434.9 |
| forest | 99.4, 107.6, 96.7 | 104.8, 104.4, 98.9 |
| boreal | 137.3, 119.8, 102.4 | 130.6, 124.5, 121.2 |
| mountains | 87.3, 114.9, 112.4 | 117.7, 130.0, 131.3 |

Flight results show substantial run-to-run variation. Small percentage differences should not be treated as established general gains. The castle control regressed; the table retains that result. These figures do not establish a universal FPS increase or isolate the contribution of each individual fix.


The before/after frame numbers below refer specifically to this paired workload.
They are not an estimate extrapolated from removed calls or theoretical blade counts.

## Validation and raw evidence

54 targeted tests pass in the final workspace test build: rendering (13), terrain
(7), forest streaming (1), conifers (8), geology/contact allocation (7), snow (4),
grass (12), and cached portal/orc comparisons (2). Portal comparison includes an
actual exposed tunnel and camera/range changes. The separate temporary grass
oracle compares all mesh attributes and indices with the old generator and passes.
`cargo check --offline` passes. Strict Clippy still reports 26 pre-existing issues
outside these new changes; it is not reported as passing.

Native GPU checks pass for mountains, forest Low/Medium textures, FXAA and 4× MSAA.
The final mountain screenshot was inspected after the fallback-bounds fix. The
terrain regression also exercises Bevy’s automatic bounds-update system.


The raw source/binary snapshots, logs and comparison data are under
`tools/build/render-audit/`:

- `baseline-v2/`, `optimized-v2/`: frozen source/assets, executable, exact build command.
- `baseline-v2-results/results.json`, `optimized-v2-results/results.json`: individual
  run settings and stationary/moving FPS, median/p95/p99 frame times and log paths.
- `verified-six-tests.log`: the final 54 targeted tests.
- `new-six-tests-final.log`: includes the separate old-generator grass comparison.
- `final-six-smoke/`: accepted screenshots and runtime/shader logs after the bounds fix.
- `grass-reference-verification.rs`: temporary old-generator comparison used for
  bit-for-bit mesh verification; excluded from production source.
- `new-six-smoke/`: preliminary shader checks for FXAA, MSAA and texture quality.
  These screenshots exposed the fallback-bounds bug described above and are not
  the final visual acceptance evidence.

Reproduce a run with `python3 tools/benchmark_render.py --binary <snapshot>/hither-sdf
--asset-root <snapshot> --output <results> --cases castle forest boreal mountains
--repeats 2`. The paired driver is `tools/build/render-audit/paired_v2.py`.
