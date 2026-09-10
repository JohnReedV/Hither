# Final V4 terrain and den measurements

> Measurement correction: these historical offscreen runs did not bound queued GPU work. A later probe observed up to 88 pending frames. Preserve the raw data, but do not treat these short-run CPU frame rates as validated GPU throughput. See PERFORMANCE_REVIEW_6.md for corrected measurements.

This final build also fixes distant-coordinate goblin acceleration and preserves authored grass visibility bounds. Headless checks at 80 updates per second confirmed movement from every resident in both benchmark dens: 8/8 and 20/20. The old controller could stall at distant coordinates; the after build therefore restores simulation work as well as optimizing it.

This isolates the fifth-review changes against the preceding fourth-review release. Both measured binaries use optimization level 3, fat LTO, one codegen unit, and identical dependency artifacts. Frozen source/asset roots and binary SHA-256 values are in `tools/build/review5-performance/measured-v4/binaries.json`. Concurrent teleport-command edits are included identically in both final measured snapshots.

Black Ops III was closed before the clean comparison suites. Earlier contended runs remain separately under `final/`, and the completed CPU/cache-only comparison remains under `clean/`; neither is pooled here. The suite runs before/after/after/before for each scene: two runs per build, for 24 completed runs in total. Every completed comparison run is retained. There are no CPU gates, load-based exclusions, retries, concurrent builds, or CPU sampling/GPU timestamp tracing in this suite. Diagnostic and image-check runs are separate.

World rendering is 3840 × 2160, presented to a 1280 × 720 offscreen target. Both builds use FXAA, High shadows, matching render/detail distances and seeds. Dens warm up for 60 seconds, then collect two stationary eight-second windows with the full resident population present. Outdoor runs warm up for 20 seconds, collect eight seconds stationary, then move at 40 m/s for 16 seconds. Clearing stays stationary for its second eight-second window. Den/forest distance is 260 m, mountains 512 m, clearing 48 m; detail is 35 m except clearing at 24 m.

Values below are arithmetic means of per-run summaries. Percentile means are not pooled frame percentiles. These are repeated spot checks, not a formal confidence interval; raw ranges and individual results remain visible.

| Scene / phase | Before FPS | After FPS | Change | Median ms before → after | p95 ms before → after | p99 ms before → after |
|---|---:|---:|---:|---:|---:|---:|
| Goblin den, 20 residents (seed 42) / stationary | 84.30 | 96.60 | +14.59% | 11.57 → 10.18 | 14.43 → 12.41 | 21.34 → 13.65 |
| Goblin den, 20 residents (seed 42) / later stationary | 84.55 | 96.50 | +14.13% | 11.59 → 10.18 | 14.20 → 12.52 | 17.63 → 13.63 |
| Goblin den, 8 residents (seed 721) / stationary | 100.35 | 116.10 | +15.70% | 9.61 → 8.52 | 12.39 → 9.93 | 18.58 → 11.16 |
| Goblin den, 8 residents (seed 721) / later stationary | 102.15 | 113.80 | +11.40% | 9.55 → 8.60 | 11.61 → 10.55 | 15.00 → 11.28 |
| Forest / stationary | 309.30 | 314.75 | +1.76% | 3.16 → 3.13 | 3.80 → 3.64 | 4.25 → 4.06 |
| Forest / traversal | 176.90 | 148.60 | -16.00% | 4.83 → 4.60 | 7.78 → 22.60 | 24.27 → 29.87 |
| Snow forest / stationary | 238.85 | 238.45 | -0.17% | 4.12 → 4.10 | 4.91 → 5.02 | 5.47 → 5.57 |
| Snow forest / traversal | 163.95 | 165.30 | +0.82% | 4.42 → 4.31 | 19.71 → 19.38 | 39.00 → 42.17 |
| Mountains / stationary | 385.60 | 401.35 | +4.08% | 2.54 → 2.42 | 3.12 → 3.01 | 3.59 → 3.45 |
| Mountains / traversal | 142.30 | 170.35 | +19.71% | 5.08 → 5.06 | 21.90 → 8.77 | 42.91 → 25.09 |
| Clearing / stationary | 438.50 | 447.05 | +1.95% | 2.21 → 2.20 | 2.75 → 2.61 | 3.12 → 2.94 |
| Clearing / later stationary | 448.00 | 438.00 | -2.23% | 2.19 → 2.21 | 2.61 → 2.77 | 2.88 → 3.12 |

## Interpretation

The two den scenes improved by 14.59% and 15.70% in the first measurement window, with gains also present in their later windows. Forest traversal averaged 16.00% slower (176.90 to 148.60 FPS); its updated runs were 192.5 and 104.7 FPS. Snow forest averaged almost unchanged, and mountain traversal improved but also varied substantially. These measurements do not establish a universal FPS gain or prove all streaming stalls fixed. No samples were excluded.

The exact measured V4 executable is installed at `target/release/hither-sdf`. Its SHA-256 is `689a7884d8b56da72db6335a9f384c0aee60c9be50c262dcc37d7e2201c7d9b0`.

## Individual final runs

| Scene | Build / run | Stationary FPS | Second-window FPS | Runtime log |
|---|---|---:|---:|---|
| Goblin den, 20 residents (seed 42) | before 0 | 82.7 | 83.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-large-before-0/runtime.log) |
| Goblin den, 20 residents (seed 42) | after 0 | 97.4 | 95.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-large-after-0/runtime.log) |
| Goblin den, 20 residents (seed 42) | after 1 | 95.8 | 97.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-large-after-1/runtime.log) |
| Goblin den, 20 residents (seed 42) | before 1 | 85.9 | 85.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-large-before-1/runtime.log) |
| Goblin den, 8 residents (seed 721) | before 0 | 96.1 | 98.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-before-0/runtime.log) |
| Goblin den, 8 residents (seed 721) | after 0 | 110.2 | 107.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-after-0/runtime.log) |
| Goblin den, 8 residents (seed 721) | after 1 | 122.0 | 119.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-after-1/runtime.log) |
| Goblin den, 8 residents (seed 721) | before 1 | 104.6 | 105.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/den-before-1/runtime.log) |
| Forest | before 0 | 309.6 | 189.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/forest-before-0/forest-0/runtime.log) |
| Forest | after 0 | 315.9 | 192.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/forest-after-0/forest-0/runtime.log) |
| Forest | after 1 | 313.6 | 104.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/forest-after-1/forest-0/runtime.log) |
| Forest | before 1 | 309.0 | 164.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/forest-before-1/forest-0/runtime.log) |
| Snow forest | before 0 | 239.9 | 184.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/boreal-before-0/boreal-0/runtime.log) |
| Snow forest | after 0 | 239.2 | 145.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/boreal-after-0/boreal-0/runtime.log) |
| Snow forest | after 1 | 237.7 | 184.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/boreal-after-1/boreal-0/runtime.log) |
| Snow forest | before 1 | 237.8 | 143.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/boreal-before-1/boreal-0/runtime.log) |
| Mountains | before 0 | 388.8 | 159.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/mountains-before-0/mountains-0/runtime.log) |
| Mountains | after 0 | 403.0 | 182.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/mountains-after-0/mountains-0/runtime.log) |
| Mountains | after 1 | 399.7 | 157.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/mountains-after-1/mountains-0/runtime.log) |
| Mountains | before 1 | 382.4 | 125.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/mountains-before-1/mountains-0/runtime.log) |
| Clearing | before 0 | 437.9 | 454.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/castle-before-0/castle-0/runtime.log) |
| Clearing | after 0 | 446.9 | 440.1 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/castle-after-0/castle-0/runtime.log) |
| Clearing | after 1 | 447.2 | 435.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/castle-after-1/castle-0/runtime.log) |
| Clearing | before 1 | 439.1 | 441.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v4/castle-before-1/castle-0/runtime.log) |

## Intermediate traversal investigation

V3 showed a snow-forest traversal slowdown with large run-to-run variation. Separate CPU profiles found biome/grass generation and visibility work prominent in both builds. An eight-run diagnostic feature isolation did not remove the spikes when GPU bounds/readback or irradiance hysteresis were disabled. Those diagnostic results are retained under terrain-isolation/ and are not pooled into this optimized release comparison. V4 fixes an independently verified grass bounds overwrite; it does not claim to eliminate all streaming stalls.

## Earlier iterations

The first index implementation completed four large-den runs before refinement: stationary 78.55 → 79.80 FPS (+1.59%), later stationary 79.20 → 79.65 FPS (+0.57%). Those measurements remain in the parent directory’s all-runs.json with their original binary hash and logs. They are not mixed into the final comparison. The later CPU/cache-only clean ABBA block measured 78.35 → 84.05 FPS (+7.28%) before adding the distance mesh. The contended eight-run iteration remains under final/, with its GPU-contention note.

## Validation

181 unique targeted tests passed across rendering, terrain, den geometry, navigation, streaming, chat, and location commands. Two broader orc tests failed in the parallel run and passed in isolated reruns; both logs are retained. The den floor regression checks chamber and tunnel positions against the original repeated-raycast results. Native entrance/deep-tunnel checks preserve dark clipped tunnel ends. A tunnel-mouth traversal reproduces the old black sky wedge and removes it on the patched shader.

The goblin distance-mesh exporter and checker verify the original joint palette and identical inverse bind matrices, finite vertex data and normalized weights. Twenty-four sampled animated poses passed with maximum sampled surface deviation of 0.0025411 normalized metres (about 1.1 mm at runtime). The original nearby mesh is retained; distant resident geometry falls from 446,926 to 83,043 triangles. Native near, transition, distant and return captures passed without asset/shader errors.

The provisional-terrain image comparison forces fallback geometry, then compares uncached procedural vertices/conservative bounds with cached vertices/tighter GPU-derived bounds at 4K. 99.3893% of captured pixels match exactly; mean absolute 8-bit channel difference is 0.0023224. No missing mountain peaks were observed. The captured presentation images are 1280 × 720. See `terrain-image-comparison.json` and the `mountains-review5-*` images under `tools/build/occlusion/`.


## Reproduced sky defect

[Before: black wedge](../tools/build/review5-performance/sky-mouth-before/up.png) · [After: outdoor sky restored](../tools/build/review5-performance/sky-mouth-after/up.png)

[Distance transition](../tools/build/goblin-lod/native-transition/far.png) · [Close-up return](../tools/build/goblin-lod/native/return.png)
