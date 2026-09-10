# Terrain streaming and goblin-den review

This pass addresses the confirmed cache-residency and provisional-bounds defects, reduces repeated irradiance rebuilds, and investigates the reported goblin-den slowdown and black sky shapes. Historical measurements are retained in `PERFORMANCE_REVIEW_5_BENCHMARK.md`; corrected GPU-completion-bounded measurements supersede them in `PERFORMANCE_REVIEW_6.md`.

## Goblin-den navigation

A native CPU profile of the seed-721 den found `RayMesh::hit` to be the hottest application function: 12.76% of sampled CPU time. The call stacks lead through `navigation::walk`, `World::floors`, and `goblin_dens::levels`. This profile includes startup and warmup; it is attribution evidence, not a steady-state FPS comparison. GPU opaque-pass time in that instrumented run was about 1.05 ms, versus roughly 11 ms overall frames. That does not measure total GPU work: Bevy's shadow passes are absent from those timestamp diagnostics.

Each footprint formerly repeated a full 3D BVH traversal for successive floor levels, at nine footprint positions. The immutable support mesh now builds a one-metre XZ triangle index on the meshing worker. A query looks up its column once, filters out triangles that do not cover its XZ footprint once, and resolves the same nearest hits, 2.5 cm restart gap, and 24-level limit. It retains the original floating-point ray origins: a regression test caught millimetre differences when a prototype computed all heights from one high origin.

Tunnel excavation queries now inspect a conservative spatial index of segments. Every eligible segment remains in original order, and comparisons against the original full scan are bitwise identical. Body-clearance sampling obtains the built collision mesh and surface elevation once per body query. Goblins now also use the existing scoped exact-query cache already used by orcs. It reuses support/body results during one synchronous AI update and resets for the next update. Resident counts, floor geometry and collision thresholds are preserved. The later movement correction below restores motion at distant coordinates.

## Distant-coordinate movement and planning

The old goblin controller fed accepted displacement back into its acceleration state. At den coordinates thousands of metres from the origin, an initial sub-millimetre step can round to zero in `f32`; this reset the requested speed to zero and could prevent acceleration indefinitely. Requested speed is now independent from observed animation speed. A bounded remainder carries only numeric rounding error into the next requested step; rejected collision movement is never accumulated. Facing follows the planned direction while moving, avoiding unstable headings from quantized tiny steps.

Navigation uses the existing one-millisecond shared planning allowance in slices that exclude physics work, and rotates the first resident served each frame. Existing paths and swept collision remain in use. Tests cover acceleration at 30, 80, 144 and 240 FPS through coordinates up to 32 km, plus rejection without accumulated wall penetration.

## Goblin rendering

Each 44 cm resident previously used 446,926 triangles at every distance, including a 428,096-triangle body. Twenty residents contributed almost nine million triangles before repeated shadow passes. The new distance-only body has 64,213 triangles; unchanged clothing, eyes, nails and stitching bring the whole distant resident to 83,043 triangles (81.42% fewer). Full detail remains within 4.5 m, reduced detail begins beyond 5.5 m, and the intervening band retains the current choice.

The derivative shares the original materials, live skeleton, animation and shadow settings. It swaps only the body mesh handle before animated visibility bounds are updated. Missing/pending derivative assets retain the full mesh. The offline exporter explicitly remaps joint indices to the original palette and copies the original inverse bind matrices, avoiding exporter-order assumptions. No duplicate actor or animation player is spawned. `build_goblin.py` regenerates the derivative through `build_goblin_lod.py`.

`check_goblin_lod.py` tests all four clips in 24 sampled poses on the original skeleton, verifies finite attributes and normalized weights, and compares over 10,000 source surface points per pose against the reduced triangles. The maximum sampled error is 0.0025411 normalized metres, approximately 1.1 mm at runtime. This is sampled geometric validation, supplemented by native runtime checks; it is not a claim of pixel-identical distant geometry.

## Sky at cave draw boundaries

The former sky shader projected nearby tunnel segments into angular masks. Those masks could cover unrelated outdoor sky. Merely removing them exposed blue sky at the clipped end of a tunnel, which the native entrance check caught.

The replacement passes only excavation volumes intersecting the actual radial draw boundary. The sky shader caps a pixel only when its endpoint lies inside one of those physical volumes and below terrain. Native cave walls continue to provide occlusion. The sky calculation also runs only for actual background pixels, instead of being calculated before surface shading overwrites it.

The user confirmed this occurs across dens, so reproduction used a separate known seed. A seed-721 tunnel-mouth traversal reproduced a black sky wedge on the old build; the corresponding patched view removed it. Entrance and deep-tunnel checks also retain dark clipped passage ends. This validates the same class of defect without claiming to recreate the original session.

## Terrain residency and bounds

Toroidal aliases now converge on a deterministic visibility/distance winner. A displaced wanted tile cannot evict a better resident; moving the view or retiring the resident changes eligibility. This prevents stationary rebake ping-pong without removing the procedural fallback.

A second GPU dispatch reduces all 1,089 provisional mesh vertices, including edges, into conservative min/max bounds with a 5 cm numerical margin. Nonblocking readback publishes bounds before visibility checks. Slot revisions reject stale results after eviction, seed changes, or clearing edits; invalidation restores the full conservative envelope. Publication is deduplicated by revision. CPU meshes keep their own exact mesh bounds.

## Irradiance movement

The irradiance field retains its current world-grid placement within a hysteresis region. The threshold leaves half the spare coverage for asynchronous construction, accounts for half a texel at the texture edge, and is capped at 16 m. Completed work is accepted only while it still covers the entire current render distance. Default-distance movement no longer rebuilds the 192 KiB texture at every 2.5 m boundary; teleports and distance changes still request a new field.

## Grass visibility bounds

Grass installs bounds expanded by 20 cm for shader wind and trampling. Bevy's automatic bounds calculation could overwrite that padding when `Mesh3d` changed for a LOD swap. Grass now declares its bounds authoritative with `NoAutoAabb`. A regression exercises the real Bevy bounds system, including a control mesh, and confirms padding survives initial creation and replacement. This also avoids redundant automatic bounds work for those manually bounded chunks.

## Audit scope

The audit also proposes finer surface pages, pixel coverage instrumentation, downstream streaming cost prediction, independent shadow policies, hardware-filtered cache textures, and grass instancing. These are substantial redesign or measurement proposals. This pass does not claim measured gains from those unimplemented proposals. In particular, it does not lower shadow quality, reduce the resident population, simplify collision, or change the scene's graphics settings to obtain FPS gains.

## Focused validation and iteration

The first optimized index iteration measured a 1.59% stationary large-den FPS difference; this preliminary result was not a controlled final performance claim. Its four completed runs remain in the original `all-runs.json`; they are not mixed with the refined build. The refined floor-query microbenchmark resolved the same 4,480 queries in 0.935 ms versus 4.967 ms for repeated BVH queries (5.314x). That is a CPU query measurement, not an FPS claim.

The targeted checks cover 181 unique tests, including seven goblin tests and fifteen grass tests. Two broader orc tests failed when run concurrently and passed individually, including wall-overlap recovery and moving-lid revision checks. The original failures and isolated reruns are retained in the final test logs. The later large-den perf recording lost samples, so no quantitative CPU ranking is claimed from it.

## Evidence

Raw build commands, source diff, binary hashes, diagnostic logs, images, and matched run logs are under `tools/build/review5-performance/`. Benchmarking uses separate frozen asset roots, matching optimized builds and settings, and retains every completed comparison run. No CPU gates or load-based retries are used.
