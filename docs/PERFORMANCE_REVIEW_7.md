# Goblin settlement geometry and simulation

The reviewer's mesh counts were correct: 446,926 triangles per master character, and 83,043 with the old body-only distance mesh. The 1 ms planning allowance covered navigation steering, not resident settling and swept walking. Den point lights also rendered the selected character geometry into their shadow maps.

## Changes

- Four complete character LODs contain **69,948 / 28,144 / 7,588 / 2,734 triangles**. The original rig, joint palette, inverse bind matrices and four animation clips are preserved. Skin sculpt, pores and clothing folds are baked into tangent-space normal maps. Texture sizes decrease with projected size. The untouched master remains available with `HITHER_GOBLIN_PREVIEW=1 HITHER_GOBLIN_SCULPT=1`.
- LOD selection uses projected crown height from the actual world render resolution and camera FOV. Thresholds are 600 / 220 / 80 pixels with 10% hysteresis. The closest gameplay level has 84.35% fewer triangles than the master. Twenty characters at the farthest level total 54,680 triangles, down from 1,660,860 with the old reduced detail.
- Close visual meshes use separate skinned proxies on the existing shadow-only layer 1, with the smallest complete LOD: 2,734 triangles per character. At levels 2 and 3 the visible mesh is already cheap, so it also casts shadows; the redundant proxy is hidden and its animated-bound updates stop. They share live joints and use dynamic animated bounds. World lights already see layers 0 and 1; gameplay cameras exclude layer 1. Lamp count, range and quality settings are preserved.
- Resident physics runs at 30 Hz, with interpolated rendering. Authoritative poses remain separate from the interpolated transforms used to draw residents. Settling, swept walking, stair/ledge checks, neighbor separation and fair rotation through the planning allowance remain active. The existing 50 ms stalled-frame simulation clamp is preserved; this is unrelated to benchmark duration.

The existing exact per-tick floor/body query cache and indexed den collision representation are retained. No approximate navigation mesh or cached static room-shadow implementation is introduced. Those would need separate correctness and invalidation work; they are not prerequisites for removing these confirmed character costs.

## Validation

The gameplay asset checker validates all five parts, four LODs and 24 sampled animation poses: 480 combinations. Checks cover matching bind matrices and joint palettes, finite attributes, normalized skin weights, and bidirectional sampled surface distance. A separate sampled clothing-clearance comparison found at most 0.42 mm additional penetration in normalized model space in the closest LOD (about 0.18 mm at game scale); the master already has some intersecting clothing vertices. This is a relative regression check, not a claim of intersection-free clothing. The master GLB SHA-256 remains `bc34df74db16cdac92127f4a4a2d404058ca154aaa7892b15696c3f0e32a23bd`.

Focused Rust tests cover LOD hysteresis, frame-rate-independent physics tick counts, distant-coordinate movement, rejected wall steps, scale and population streaming. Headless den lifecycle checks retained moving populations of 8/8 at seed 721 and 20/20 at seed 42 over 30 simulated seconds. Their timings were collected during compilation and are not FPS evidence.

Native captures show the reduced mesh animating and its separate shadow following it. Validation artifacts are under `tools/build/review7-performance/` and `tools/build/goblin-gameplay/`.

## Measurement protocol

Use the same seed-42 20-resident den, 3840 x 2160 world rendering, 1280 x 720 offscreen presentation, FXAA, High shadows, 260/35 m render/detail distances, 60-second warmup and two eight-second stationary windows. All builds retain the corrected two-frame GPU completion bound from review 6. No CPU-idle gates, load-based exclusions or concurrent builds are used during FPS tests.

Attribution modes act after 45 seconds of real time. Freezing stops resident AI/physics while animations remain active. Hiding removes the resident hierarchy from rendering while its simulation continues. The corrected point-shadow diagnostic modifies only `ExtractedPointLight` in the render world before shadow views are created. It preserves main-world lamp ownership, intensity and priority, as well as directional shadows; main-world light visibility work continues. The interventions run separately, not cumulatively. Since residents wander, these are repeatable scene comparisons rather than pixel-identical captures.

Attribution binaries use optimization level 3 without LTO. Final before/after binaries both use optimization level 3, fat LTO and one codegen unit. Do not pool the two sets. The baseline is the corrected build installed after review 6, not the much older unbounded benchmark.

The first six attribution runs are retained in `attribution/all-runs.json`. The initial point-shadow intervention modified gameplay shadow ownership and therefore changed the lamp-budget priority: its 109.2 / 108.6 FPS result is confounded and must not be interpreted as the cost of shadows. A corrected render-world intervention is measured separately. The first gameplay candidate also kept duplicate shadow skins at every distance; it reduced visible triangles but did not improve average overview FPS, prompting the shared distant-geometry refinement.

## Repeated optimized-release results

Two runs per build per scene, ordered before / after / after / before. Each run has two eight-second stationary windows following a 60-second warmup. All eight release runs are retained. FPS below is the arithmetic mean of the four window summaries for each build; percentile columns average the reported window percentiles rather than pooling raw frames.

| View | Before FPS | After FPS | Change | Median ms before / after | p99 ms before / after |
|---|---:|---:|---:|---:|---:|
| overview | 119.90 | 120.80 | +0.75% | 8.178 / 8.100 | 10.985 / 11.000 |
| interior | 107.65 | 111.70 | +3.76% | 9.090 / 8.795 | 12.252 / 11.665 |

The overview is effectively unchanged within observed run variation. The interior shows a modest improvement. These measurements do not support claiming a large general den FPS improvement from the character changes on this RTX 5090. The geometry and simulation workload reductions are real, but much of the remaining cost is elsewhere in point-light shadow rendering.

## Every release run

| View | Build / run | First window FPS | Second window FPS | p99 ms, first / second | Log |
|---|---|---:|---:|---:|---|
| overview | before 0 | 119.7 | 120.9 | 11.22 / 9.99 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-overview/0-before/runtime.log) |
| overview | final 1 | 121.7 | 121.4 | 10.89 / 11.15 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-overview/1-final/runtime.log) |
| overview | final 2 | 121.1 | 119.0 | 10.71 / 11.25 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-overview/2-final/runtime.log) |
| overview | before 3 | 120.3 | 118.7 | 10.97 / 11.76 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-overview/3-before/runtime.log) |
| interior | before 0 | 107.8 | 106.0 | 12.45 / 13.40 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-interior/0-before/runtime.log) |
| interior | final 1 | 112.1 | 112.7 | 11.78 / 11.78 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-interior/1-final/runtime.log) |
| interior | final 2 | 111.0 | 111.0 | 12.02 / 11.08 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-interior/2-final/runtime.log) |
| interior | before 3 | 109.7 | 107.1 | 10.77 / 12.39 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/release-interior/3-before/runtime.log) |

## Attribution measurements

These use separate binaries without LTO and are not pooled with the release measurements. The freeze and hide runs are single exploratory comparisons. The corrected shadow run keeps the main-world lamp selection unchanged and disables point-light shadows only in the render world. It does not remove main-world shadow visibility work. Its large effect includes both shadow-map generation and shadow sampling; it does not independently measure how much a static shadow cache would save.

| Diagnostic | First window FPS | Second window FPS | First p99 ms | Log |
|---|---:|---:|---:|---|
| isolation control | 121.8 | 122.3 | 11.11 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution/0-isolation/runtime.log) |
| isolation HITHER_PROFILE_FREEZE_GOBLINS | 123.0 | 123.9 | 9.62 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution/1-isolation/runtime.log) |
| isolation HITHER_PROFILE_HIDE_GOBLINS | 127.4 | 128.1 | 10.51 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution/2-isolation/runtime.log) |
| isolation HITHER_PROFILE_NO_POINT_SHADOWS **confounded; do not use for attribution** | 109.2 | 108.6 | 12.07 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution/3-isolation/runtime.log) |
| simulation control | 121.6 | 121.5 | 11.13 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution/4-simulation/runtime.log) |
| gameplay control | 121.3 | 120.9 | 9.44 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution/5-gameplay/runtime.log) |
| isolation-corrected control | 116.8 | 115.8 | 10.88 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution-corrected/0-isolation-corrected/runtime.log) |
| isolation-corrected HITHER_PROFILE_NO_POINT_SHADOWS | 232.0 | 232.7 | 7.52 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review7-performance/attribution-corrected/1-isolation-corrected/runtime.log) |

The corrected point-shadow pair rises from 116.8 to 232.0 FPS while preserving lamp ownership and illumination. That is a much larger effect than freezing simulation or hiding residents in this overview. Static room-shadow caching remains unimplemented: a correct cache needs light/geometry invalidation and separation of dynamic casters. Disabling shadows is a diagnostic only and is not part of the shipped fix.

## Reproduction and artifacts

- `tools/build_goblin.py` rebuilds and validates the gameplay derivative automatically. Independent commands are `tools/.venv/bin/python tools/build_goblin_gameplay.py` and `tools/.venv/bin/python tools/check_goblin_gameplay.py`.
- `cargo test --bin hither-sdf world::goblins -- --test-threads=1`: 10 focused tests passed. This is not a full-suite or strict-Clippy claim.
- `tools/build/review7-performance/run.py` contains the measurement settings; `summary.json` and `release-runs.json` contain the final data. Source/build commands and frozen assets are in the `final-v2` snapshot and the retained review-6 `fixed` baseline.
- Before SHA-256: `d939ee9188214546a76d57b930876a4510c495cfdd76be0a6bc6d58a23ae8928`.
- After SHA-256: `d769678c8d12e334b13f852c73d61fe373f1ded5a2fd886246c0ec5f05c93efe`.


The exact tested after binary is installed at `target/release/hither-sdf`; its hash is recorded in `installed.json`. Native final interior captures are in `tools/build/review7-performance/native-final/`. The final asset checker also verifies the baked skin and clothing normal-map references and texture dimensions for all four LODs.
