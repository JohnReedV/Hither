# Latest-patch FPS comparison

This measures only the fourth-audit patch against the preceding release. It is not the cumulative gain from the earlier audits.

Both binaries use optimization level 3, fat LTO, one codegen unit, and the same dependency artifacts. The baseline is rebuilt from the preceding release with the separate orc walking-speed edit applied to both sides; only the nine latest-audit source/shader files differ. The current snapshot matches the final tested source. Four forest runs per build and two runs per build for the other scenes use before/after/after/before blocks, with every completed run retained. Forest was expanded after the initial patched traversal range exceeded 20% of its mean; the additional four runs were fixed in advance of running them (see sampling-decision.json). No CPU gates, run rejection, or retries are used. Builds complete before this benchmark starts.

4K internal resolution, 1280 × 720 offscreen targets, seed 721, High shadows, FXAA, 20 seconds warmup, 8 seconds stationary, then 16 seconds traversal at 40 m/s (clearing instead stays stationary for 8 more seconds). Render distance is 48 m for clearing, 260 m for the forests, and 512 m for mountains; detail distance is 24 m / 35 m. These match the previous benchmark protocol. There is no display-vsync cap.

FPS and frame-time percentiles below are arithmetic means of the per-run summaries. Percentile values are not pooled frame percentiles. This small repeat count provides a spot check, not a narrow confidence interval; individual run ranges are included below.

| Scene / phase | Before FPS | After FPS | Change | Median ms before → after | p95 ms before → after | p99 ms before → after |
|---|---:|---:|---:|---:|---:|---:|
| Clearing / stationary | 434.00 | 428.40 | -1.29% | 2.25 → 2.27 | 2.76 → 2.83 | 3.12 → 3.22 |
| Clearing / later stationary | 428.15 | 438.20 | +2.35% | 2.26 → 2.25 | 2.84 → 2.63 | 3.30 → 2.98 |
| Forest / stationary | 300.98 | 298.77 | -0.73% | 3.25 → 3.29 | 3.95 → 3.97 | 4.31 → 4.37 |
| Forest / traversal | 163.82 | 137.93 | -15.81% | 4.87 → 4.85 | 13.22 → 23.10 | 32.61 → 37.66 |
| Snow forest / stationary | 236.40 | 238.50 | +0.89% | 4.12 → 4.09 | 5.05 → 5.04 | 5.56 → 5.65 |
| Snow forest / traversal | 141.35 | 191.25 | +35.30% | 4.47 → 4.57 | 32.50 → 7.43 | 44.31 → 21.93 |
| Mountains / stationary | 381.15 | 358.80 | -5.86% | 2.55 → 2.72 | 3.16 → 3.49 | 3.71 → 3.93 |
| Mountains / traversal | 177.40 | 164.00 | -7.55% | 5.23 → 5.26 | 8.29 → 8.79 | 10.88 → 25.65 |

## Interpretation

The latest patch has mixed results. Forest traversal averages decrease from 163.825 to 137.925 FPS (−15.81%). Patched forest traversal spans 90.7–187.6 FPS, versus 152.3–180.3 for the baseline, so the result is highly variable; every run is included. Its mean p95/p99 frame times also worsen.

Snow-forest traversal increases from 141.35 to 191.25 FPS (+35.30%), with improved p95/p99 frame times. Mountains regress both stationary (−5.86%) and moving (−7.55%); their mean per-run p99 increases from 10.885 to 25.65 ms. The clearing's two stationary windows move in opposite directions (−1.29% and +2.35%), which does not establish a consistent change there.

These results do not justify calling the patch a general performance improvement. The forest and mountain regressions remain unresolved. No attribution to external CPU load is claimed: CPU activity was neither gated nor used to exclude results. This benchmark turn changes no renderer code.

## Individual runs

| Scene | Build / run | Stationary FPS | Second-phase FPS | Runtime log |
|---|---|---:|---:|---|
| Forest | before 0 | 307.9 | 165.5 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-before-0/forest-0/runtime.log) |
| Forest | after 0 | 312.3 | 105.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-after-0/forest-0/runtime.log) |
| Forest | after 1 | 308.0 | 187.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-after-1/forest-0/runtime.log) |
| Forest | before 1 | 303.2 | 152.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-before-1/forest-0/runtime.log) |
| Snow forest | before 0 | 239.9 | 142.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/boreal-before-0/boreal-0/runtime.log) |
| Snow forest | after 0 | 239.1 | 178.1 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/boreal-after-0/boreal-0/runtime.log) |
| Snow forest | after 1 | 237.9 | 204.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/boreal-after-1/boreal-0/runtime.log) |
| Snow forest | before 1 | 232.9 | 139.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/boreal-before-1/boreal-0/runtime.log) |
| Mountains | before 0 | 383.1 | 176.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/mountains-before-0/mountains-0/runtime.log) |
| Mountains | after 0 | 364.1 | 176.8 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/mountains-after-0/mountains-0/runtime.log) |
| Mountains | after 1 | 353.5 | 151.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/mountains-after-1/mountains-0/runtime.log) |
| Mountains | before 1 | 379.2 | 178.6 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/mountains-before-1/mountains-0/runtime.log) |
| Clearing | before 0 | 429.6 | 425.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/castle-before-0/castle-0/runtime.log) |
| Clearing | after 0 | 431.4 | 432.0 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/castle-after-0/castle-0/runtime.log) |
| Clearing | after 1 | 425.4 | 444.4 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/castle-after-1/castle-0/runtime.log) |
| Clearing | before 1 | 438.4 | 430.9 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/castle-before-1/castle-0/runtime.log) |
| Forest | before 2 | 303.0 | 157.2 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-before-2/forest-0/runtime.log) |
| Forest | after 2 | 285.8 | 90.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-after-2/forest-0/runtime.log) |
| Forest | after 3 | 289.0 | 167.7 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-after-3/forest-0/runtime.log) |
| Forest | before 3 | 289.8 | 180.3 | [Log](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/forest-before-3/forest-0/runtime.log) |

## Provenance

Build paths and SHA-256 digests: [binaries.json](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/binaries.json). Raw results: [all-runs.json](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/all-runs.json). Frame time details: [summary.json](/home/johnreed/Desktop/projects/Hither/tools/build/review4-performance/summary.json).
