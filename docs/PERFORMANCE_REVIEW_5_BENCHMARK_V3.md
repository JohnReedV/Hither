# Candidate V3 terrain and den measurements

> Measurement correction: these historical offscreen runs did not bound queued GPU work. A later probe observed up to 88 pending frames. Preserve the raw data, but do not treat these short-run CPU frame rates as validated GPU throughput. See PERFORMANCE_REVIEW_6.md for corrected measurements.

This candidate is retained as an intermediate result. Snow-forest traversal regressed in the measured average, so investigation continued before installation.

This isolates the fifth-review changes against the preceding fourth-review release. Both measured binaries use optimization level 3, fat LTO, one codegen unit, and identical dependency artifacts. Frozen source/asset roots and binary SHA-256 values are in `tools/build/review5-performance/measured-v3/binaries.json`. Concurrent teleport-command edits are included identically in both final measured snapshots.

Black Ops III was closed before this final suite. Earlier contended runs remain separately under `final/`, and the completed CPU/cache-only comparison remains under `clean/`; neither is pooled here. The suite runs before/after/after/before for each scene: two runs per build initially, followed by an additional matched block for the smaller den, snow forest and mountains after large within-build variation. Those three scenes have four runs per build; the suite contains 36 completed runs in total. Every completed comparison run is retained. There are no CPU gates, load-based exclusions, retries, concurrent builds, or CPU sampling/GPU timestamp tracing in this suite. Diagnostic and image-check runs are separate.

World rendering is 3840 × 2160, presented to a 1280 × 720 offscreen target. Both builds use FXAA, High shadows, matching render/detail distances and seeds. Dens warm up for 60 seconds, then collect two stationary eight-second windows with residents active. Outdoor runs warm up for 20 seconds, collect eight seconds stationary, then move at 40 m/s for 16 seconds. Clearing stays stationary for its second eight-second window. Den/forest distance is 260 m, mountains 512 m, clearing 48 m; detail is 35 m except clearing at 24 m.

Values below are arithmetic means of per-run summaries. Percentile means are not pooled frame percentiles. These are repeated spot checks, not a formal confidence interval; raw ranges and individual results remain visible.

| Scene / phase | Before FPS | After FPS | Change | Median ms before → after | p95 ms before → after | p99 ms before → after |
|---|---:|---:|---:|---:|---:|---:|
| Goblin den, 20 residents (seed 42) / stationary | 84.40 | 93.95 | +11.32% | 11.46 → 10.45 | 15.36 → 12.61 | 26.55 → 13.59 |
| Goblin den, 20 residents (seed 42) / later stationary | 85.25 | 94.15 | +10.44% | 11.42 → 10.38 | 15.18 → 12.82 | 18.47 → 13.75 |
| Goblin den, 8 residents (seed 721) / stationary | 89.90 | 94.10 | +4.67% | 10.77 → 10.61 | 13.58 → 13.02 | 20.26 → 14.09 |
| Goblin den, 8 residents (seed 721) / later stationary | 90.47 | 93.65 | +3.51% | 10.68 → 10.72 | 14.03 → 13.20 | 16.60 → 14.02 |
| Forest / stationary | 306.20 | 306.20 | +0.00% | 3.21 → 3.21 | 3.81 → 3.79 | 4.23 → 4.16 |
| Forest / traversal | 171.20 | 170.15 | -0.61% | 5.08 → 5.00 | 8.04 → 8.88 | 22.95 → 23.20 |
| Snow forest / stationary | 234.93 | 229.25 | -2.42% | 4.15 → 4.24 | 5.12 → 5.25 | 5.59 → 5.86 |
| Snow forest / traversal | 193.15 | 167.57 | -13.24% | 4.65 → 4.69 | 7.06 → 14.88 | 21.89 → 33.76 |
| Mountains / stationary | 359.98 | 373.25 | +3.69% | 2.71 → 2.59 | 3.42 → 3.31 | 3.85 → 3.79 |
| Mountains / traversal | 155.20 | 160.25 | +3.25% | 5.34 → 5.30 | 15.88 → 15.49 | 27.42 → 19.84 |
| Clearing / stationary | 406.15 | 410.95 | +1.18% | 2.38 → 2.35 | 3.05 → 3.00 | 3.61 → 3.45 |
| Clearing / later stationary | 407.65 | 418.35 | +2.62% | 2.36 → 2.33 | 3.04 → 2.91 | 3.57 → 3.24 |

## Individual final runs

| Scene | Build / run | Stationary FPS | Second-window FPS | Runtime log |
|---|---|---:|---:|---|
| Goblin den, 20 residents (seed 42) | before 0 | 87.2 | 86.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-large-before-0/runtime.log) |
| Goblin den, 20 residents (seed 42) | after 0 | 97.0 | 96.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-large-after-0/runtime.log) |
| Goblin den, 20 residents (seed 42) | after 1 | 90.9 | 91.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-large-after-1/runtime.log) |
| Goblin den, 20 residents (seed 42) | before 1 | 81.6 | 83.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-large-before-1/runtime.log) |
| Goblin den, 8 residents (seed 721) | before 0 | 87.2 | 88.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-before-0/runtime.log) |
| Goblin den, 8 residents (seed 721) | after 0 | 119.8 | 120.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-after-0/runtime.log) |
| Goblin den, 8 residents (seed 721) | after 1 | 84.3 | 85.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-after-1/runtime.log) |
| Goblin den, 8 residents (seed 721) | before 1 | 85.8 | 85.1 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-before-1/runtime.log) |
| Forest | before 0 | 305.5 | 165.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/forest-before-0/forest-0/runtime.log) |
| Forest | after 0 | 309.6 | 158.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/forest-after-0/forest-0/runtime.log) |
| Forest | after 1 | 302.8 | 181.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/forest-after-1/forest-0/runtime.log) |
| Forest | before 1 | 306.9 | 177.1 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/forest-before-1/forest-0/runtime.log) |
| Snow forest | before 0 | 234.4 | 204.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-before-0/boreal-0/runtime.log) |
| Snow forest | after 0 | 235.0 | 204.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-after-0/boreal-0/runtime.log) |
| Snow forest | after 1 | 230.1 | 169.1 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-after-1/boreal-0/runtime.log) |
| Snow forest | before 1 | 231.9 | 205.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-before-1/boreal-0/runtime.log) |
| Mountains | before 0 | 363.9 | 177.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-before-0/mountains-0/runtime.log) |
| Mountains | after 0 | 376.6 | 178.0 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-after-0/mountains-0/runtime.log) |
| Mountains | after 1 | 377.9 | 113.1 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-after-1/mountains-0/runtime.log) |
| Mountains | before 1 | 361.6 | 153.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-before-1/mountains-0/runtime.log) |
| Clearing | before 0 | 398.2 | 401.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/castle-before-0/castle-0/runtime.log) |
| Clearing | after 0 | 419.8 | 432.0 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/castle-after-0/castle-0/runtime.log) |
| Clearing | after 1 | 402.1 | 404.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/castle-after-1/castle-0/runtime.log) |
| Clearing | before 1 | 414.1 | 413.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/castle-before-1/castle-0/runtime.log) |
| Goblin den, 8 residents (seed 721) | before 2 | 88.1 | 87.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-before-2/runtime.log) |
| Goblin den, 8 residents (seed 721) | after 2 | 82.1 | 80.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-after-2/runtime.log) |
| Goblin den, 8 residents (seed 721) | after 3 | 90.2 | 88.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-after-3/runtime.log) |
| Goblin den, 8 residents (seed 721) | before 3 | 98.5 | 101.1 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/den-before-3/runtime.log) |
| Snow forest | before 2 | 240.7 | 191.0 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-before-2/boreal-0/runtime.log) |
| Snow forest | after 2 | 223.5 | 128.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-after-2/boreal-0/runtime.log) |
| Snow forest | after 3 | 228.4 | 167.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-after-3/boreal-0/runtime.log) |
| Snow forest | before 3 | 232.7 | 171.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/boreal-before-3/boreal-0/runtime.log) |
| Mountains | before 2 | 356.0 | 168.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-before-2/mountains-0/runtime.log) |
| Mountains | after 2 | 366.6 | 175.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-after-2/mountains-0/runtime.log) |
| Mountains | after 3 | 371.9 | 174.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-after-3/mountains-0/runtime.log) |
| Mountains | before 3 | 358.4 | 121.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review5-performance/measured-v3/mountains-before-3/mountains-0/runtime.log) |

## Earlier iterations

The first index implementation completed four large-den runs before refinement: stationary 78.55 → 79.80 FPS (+1.59%), later stationary 79.20 → 79.65 FPS (+0.57%). Those measurements remain in the parent directory’s all-runs.json with their original binary hash and logs. They are not mixed into the final comparison. The later CPU/cache-only clean ABBA block measured 78.35 → 84.05 FPS (+7.28%) before adding the distance mesh. The contended eight-run iteration remains under final/, with its GPU-contention note.

## Validation

164 unique targeted tests passed across rendering, terrain, den geometry, navigation, streaming, chat, and location commands. Two broader orc tests failed in the parallel run and passed in isolated reruns; both logs are retained. The den floor regression checks chamber and tunnel positions against the original repeated-raycast results. Native entrance/deep-tunnel checks preserve dark clipped tunnel ends. A tunnel-mouth traversal reproduces the old black sky wedge and removes it on the patched shader.

The goblin distance-mesh exporter and checker verify the original joint palette and identical inverse bind matrices, finite vertex data and normalized weights. Twenty-four sampled animated poses passed with maximum sampled surface deviation of 0.0025411 normalized metres (about 1.1 mm at runtime). The original nearby mesh is retained; distant resident geometry falls from 446,926 to 83,043 triangles. Native near, transition, distant and return captures passed without asset/shader errors.

The provisional-terrain image comparison forces fallback geometry, then compares uncached procedural vertices/conservative bounds with cached vertices/tighter GPU-derived bounds at 4K. 99.3893% of captured pixels match exactly; mean absolute 8-bit channel difference is 0.0023224. No missing mountain peaks were observed. The captured presentation images are 1280 × 720. See `terrain-image-comparison.json` and the `mountains-review5-*` images under `tools/build/occlusion/`.


## Reproduced sky defect

[Before: black wedge](../tools/build/review5-performance/sky-mouth-before/up.png) · [After: outdoor sky restored](../tools/build/review5-performance/sky-mouth-after/up.png)

[Distance transition](../tools/build/goblin-lod/native-transition/far.png) · [Close-up return](../tools/build/goblin-lod/native/return.png)
