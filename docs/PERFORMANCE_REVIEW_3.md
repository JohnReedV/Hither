# Follow-up traversal review

This change includes the verified fixes from the two preceding reviews, documented in [the second review](PERFORMANCE_REVIEW_2.md), plus the additional fixes below. The benchmark baseline is the frozen source at the start of the second review; it already includes the first audit's fixes. The provisional, CPU-contended second-review figures are excluded. Final results therefore measure the cumulative second and third review changes, not only the newest patch.

## Additional corrections

- Terrain tiles retain their full elevation lattice, error bounds for each geometric stride, and lazily populated normals. Worker-generated samples survive an obsolete LOD result while the tile remains loaded. LOD changes reuse this data and preserve the original geometry/error thresholds and normal equations. A bounded candidate sort prioritizes unbaked visible tiles over ordinary LOD rebuilds. Retained data is released when the tile leaves residency; this exchanges resident CPU memory for less repeated generation.
- Streamed terrain, grass, oak, citrus, conifer, alpine, rock, understory, log and den entities use shared retirement. Obsolete roots become hidden in the current command flush. Physical cleanup walks the hierarchy leaf-first with a separate limit of 256 despawns and 512 hierarchy steps per frame, preventing a parent despawn from silently deleting thousands of children at once. Logical residency is removed immediately. Cleanup and associated render-entity removal may finish over subsequent frames.
- Oak spawning counts all five entities in its hierarchy. Its authored foliage adapter only changes the displayed mesh on an authored-source transition, preserving a shared crown selection between transitions.
- Empty grass cells retain their completed LOD records without allocating entities. Leaving-cell discovery visits the rectangle difference instead of scanning all loaded cell records. Nonempty LOD replacements keep their existing entities and now use entity admission.
- Orc clearing-site lookups memoize both accepted and rejected regions in a bounded, thread-local table keyed by cell and world seed. Terrain samples no longer repeat invariant site eligibility noise; gate state remains separate and dynamic.
- Rock placement caching evicts only the least recently used entry when its 256-entry capacity is reached. Hits refresh recency; other cached placements remain available.
- Rock compute now has nonblocking GPU timestamps and adaptive batches of 64–8,192 vertices targeting 0.35 ms. Terrain and rock bake work share one batch admission per frame, with alternating reservations under contention and expiry when work disappears. Without timestamp support, rock work uses a conservative 512-vertex batch and terrain retains its eight-row fallback. Terrain sizing can grow to a full 256-row page when timestamps allow, rather than imposing a 64-row ceiling that prolongs cache misses and repeats submissions. Publication still waits for complete data. These are bounded work and feedback targets, not a guaranteed GPU deadline or a measurement of all remaining frame headroom.

## Latest reviewer follow-up

The newest review correctly identifies the costs of blended cache lookups, procedural fallback vertices, downstream streaming work, and shadow rendering. These are optimization candidates rather than seven demonstrated correctness bugs. Cache blending and cascade overlap currently protect visual quality; removing them directly would change the rendered result.

Two additional changes are included in the final build:

- Grass chunks now have abrupt, AABB-centered visibility ranges at the existing fade distance plus the full chunk radius, rounded outward to whole metres. This keeps Bevy’s lifetime range catalogue bounded rather than adding a unique GPU range for every streamed mesh. Any chunk rejected by this test has no vertex inside the shader’s fade distance. This avoids drawing completely faded chunks in depth/color passes without changing blade density, fade thresholds, wind, or contact deformation. Changes to detail distance update the ranges; newly installed/updated meshes use their current bounds.
- Terrain support queries can return height and gradient from one exact triangle lookup. Vegetation footprint checks and grass roots reuse that pair instead of fetching the same lattice vertices twice. The existing 16,384-entry thread-local terrain lattice cache already supplies shared elevation samples; adding another general lattice cache would duplicate it. The new query is checked bit-for-bit against the original height and gradient queries at integer positions, both triangle interiors, diagonal boundaries, and negative coordinates.

The previous after-build measurements are preserved under `pre-grass-range` and excluded from the final comparison. The baseline remains the frozen build from the start of the second review.

## Findings that remain proposals

The shadow cascade overlap is intentional: this renderer chooses cascades radially, so simply reducing overlap can remove valid shadow coverage at oblique angles. This patch preserves shadow distance and quality. Independent shadow controls, redesigned cascade projection/selection, and static shadow reuse need an explicit visual/invalidation design.

Hardware-filtered cache textures and different cache coverage remain GPU optimization candidates, not established gains. They are not substituted without profiling and image comparisons. Region-sized distant grass buffers, shared GPU fallback height data, broader cross-system environment sampling, and admission feedback from complete deferred/render preparation costs also remain larger changes. The existing instrumentation measures selected stages but does not attribute all frame work to streaming, so feeding its total directly into admission would mischarge unrelated rendering.

## Validation and measurement policy

222 targeted tests pass (two existing optional tests are ignored). Native runtime checks and measurements are recorded in `tools/build/review3-performance`. Both builds render offscreen without an OS window or swapchain. Some harness runs use an isolated X display, which does not change the offscreen render path. Superseded measurements and the initial 64-row-ceiling after-build are archived and excluded. Native forest and mountain runs pass without renderer validation errors; screenshots were inspected, including a forest comparison with the preserved baseline. Per-batch GPU bake timings are in `bake-validation.json`; these are not whole-frame speedups. The 32 rendering tests also pass after removing the unnecessary 64-row ceiling. The completed matched results are below. The final native forest check also passed with the grass ranges enabled; its image was inspected, and terrain bake batches measured at most 0.1164 ms (bake work only).

At the user's request, CPU preflight gating, monitoring, automatic rejection, and retries have been removed from the active benchmark harness. The final comparison uses fresh before/after runs under the same unfiltered policy. All measured frame-time spikes are included. Ordinary desktop activity can contribute to variation. Earlier screened measurements and their CPU logs are archived under `cpu-screened` and are not mixed into the final comparison. Runtime errors and hung-process timeouts are still checked.

## Matched whole-frame measurements

RTX 5090; 3840 × 2160 internal rendering into a 1280 × 720 offscreen target; seed 721; FXAA; high texture and shadow quality; grass enabled; maximum FPS 1000. Each fresh process warms up for 20 seconds, then measures eight stationary seconds. Moving scenes subsequently traverse at 40 m/s for 16 seconds. Clearing stays still in both phases (its second sample lasts eight seconds). Render/detail distances are 48/24 m for clearing, 260/35 m for forests, and 512/35 m for mountains. Bevy diagnostic GPU timers are disabled equally in both versions; the new adaptive bake timestamps remain part of the changed renderer.

Rows show arithmetic means across independent completed runs, including all slow samples; no CPU gating, filtering, or automatic retries are used. Frame-time columns are means of the individual run percentiles. FPS change is `(after / before - 1) × 100`; negative values are slower. These comparisons use this review’s own frozen baseline, not the previous audit’s reported numbers.

| Scene / phase | Runs per version | Before FPS | After FPS | FPS change | Median ms before → after | p95 ms before → after | p99 ms before → after |
|---|---:|---:|---:|---:|---:|---:|---:|
| Clearing / stationary | 2 / 2 | 430.35 | 423.30 | -1.64% | 2.26 → 2.29 | 2.79 → 2.88 | 3.28 → 3.16 |
| Clearing / later stationary | 2 / 2 | 432.55 | 429.60 | -0.68% | 2.27 → 2.27 | 2.69 → 2.80 | 3.05 → 3.08 |
| Forest / stationary | 4 / 4 | 280.93 | 302.30 | +7.61% | 3.49 → 3.24 | 4.18 → 3.91 | 4.52 → 4.30 |
| Forest / traversal | 4 / 4 | 140.62 | 170.53 | +21.26% | 5.18 → 5.05 | 18.52 → 8.65 | 45.00 → 24.22 |
| Snow forest / stationary | 2 / 2 | 228.15 | 232.65 | +1.97% | 4.26 → 4.19 | 5.32 → 5.16 | 5.81 → 5.70 |
| Snow forest / traversal | 2 / 2 | 135.85 | 203.90 | +50.09% | 5.12 → 4.60 | 23.23 → 7.04 | 50.59 → 8.81 |
| Mountains / stationary | 2 / 2 | 369.25 | 383.10 | +3.75% | 2.63 → 2.53 | 3.28 → 3.17 | 3.66 → 3.72 |
| Mountains / traversal | 2 / 2 | 149.55 | 165.95 | +10.97% | 6.24 → 5.17 | 10.18 → 9.18 | 12.43 → 25.87 |

### Interpretation

The comparison contains 20 completed, unfiltered runs: four per build for forest and two per build for each other scene. Forest was expanded because the initial within-build traversal FPS range exceeded 20% of its mean. Every completed final-policy run remains included; interrupted runs and superseded builds are excluded.

Traversal FPS averages improve by 21.26% in forest, 50.09% in snow forest, and 10.97% in mountains. The clearing stationary average decreases by 1.64%. These are observed averages, not a guarantee of a sustained speedup: forest baseline traversal ranged from 101.3 to 163.2 FPS, while the changed build ranged from 151.7 to 183.1 FPS. Desktop activity was neither gated nor filtered.

Forest and snow-forest mean p95/p99 times improve. Mountain median frame time improves from 6.235 to 5.165 ms, but mean per-run p99 worsens from 12.43 to 25.865 ms; its higher average FPS does not establish a tail-latency improvement. Small stationary differences should be treated cautiously given normal run variation.

### Individual completed runs

| Scene | Version / run | Stationary FPS | Second-phase FPS | Runtime log |
|---|---|---:|---:|---|
| Clearing | before 0 | 426.4 | 438.8 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/castle-before-0/castle-0/runtime.log) |
| Clearing | before 1 | 434.3 | 426.3 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/castle-before-1/castle-0/runtime.log) |
| Clearing | after 0 | 426.6 | 432.6 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/castle-after-0/castle-0/runtime.log) |
| Clearing | after 1 | 420.0 | 426.6 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/castle-after-1/castle-0/runtime.log) |
| Forest | before 0 | 275.1 | 163.2 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-before-0/forest-0/runtime.log) |
| Forest | before 1 | 276.6 | 101.3 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-before-1/forest-0/runtime.log) |
| Forest | before 2 | 288.9 | 154.0 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-before-2/forest-0/runtime.log) |
| Forest | before 3 | 283.1 | 144.0 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-before-3/forest-0/runtime.log) |
| Forest | after 0 | 302.8 | 164.9 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-after-0/forest-0/runtime.log) |
| Forest | after 1 | 298.0 | 151.7 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-after-1/forest-0/runtime.log) |
| Forest | after 2 | 303.4 | 183.1 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-after-2/forest-0/runtime.log) |
| Forest | after 3 | 305.0 | 182.4 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/forest-after-3/forest-0/runtime.log) |
| Snow forest | before 0 | 227.8 | 128.6 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/boreal-before-0/boreal-0/runtime.log) |
| Snow forest | before 1 | 228.5 | 143.1 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/boreal-before-1/boreal-0/runtime.log) |
| Snow forest | after 0 | 233.5 | 204.4 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/boreal-after-0/boreal-0/runtime.log) |
| Snow forest | after 1 | 231.8 | 203.4 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/boreal-after-1/boreal-0/runtime.log) |
| Mountains | before 0 | 369.0 | 148.6 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/mountains-before-0/mountains-0/runtime.log) |
| Mountains | before 1 | 369.5 | 150.5 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/mountains-before-1/mountains-0/runtime.log) |
| Mountains | after 0 | 384.1 | 180.2 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/mountains-after-0/mountains-0/runtime.log) |
| Mountains | after 1 | 382.1 | 151.7 | [Runtime log](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/mountains-after-1/mountains-0/runtime.log) |

Raw FPS and frame-time percentiles: [all runs](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/all-runs.json). Binary hashes and preserved source/assets: [manifest](/home/johnreed/Desktop/projects/Hither/tools/build/review3-performance/binaries.json). Each raw result points to its complete runtime log.
