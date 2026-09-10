# Terrain shading and streaming follow-up

The audit correctly identified distance-gated terrain shading, unaccounted downstream mesh/command work, unbounded terrain retirement scanning, duplicate terrain generation, and detail-linked sun-shadow range. These are distinct costs; source inspection alone does not establish their FPS impact.

## Changes

- Finished terrain surface shading is selected by projected footprint and available cache resolution, independently of geometry detail distance. Nearby fragments retain fine procedural detail and reuse four slow-changing climate fields from 12.5 cm pages: snow climate, mountain coverage, woodland density and snowline variation. The extra FP16 payload occupies 8 MiB; total surface storage is approximately 40 MiB. Climate interpolation falls back at page borders. Small snow patches, altitude/slope retention, fractures, grains, lighting and tracks remain dynamic/procedural.
- A shared snow calculation feeds terrain color, mountain shading, powder normals and tracks. Rock/ground contact shading applies the cached terrain blend once. Non-terrain SDF materials cannot consume the terrain cache.
- Streaming admission combines its measured scope time with smoothed downstream estimates. Render-world timing surrounds mesh allocation/staging/preparation; a conservative extraction estimate includes other asset classes in Bevy's public extraction set. Terrain command batches are measured when applied. Existing entity, byte and worker limits remain. A waiting layer reserves the first actual installation on its next priority frame, including time and byte admission; unused reservations expire. This prevents small early uploads from repeatedly excluding a larger waiting job. Bevy's generic byte limiter was investigated and not used: mesh allocation precedes it in this version.
- Terrain retirement scans only after a window change, visits at most 128 queue entries and targets 0.5 ms per frame, including bookkeeping and mesh removal. Actual leaf-first hierarchy retirement separately retains its count bound and targets 0.5 ms. Return teleports recheck current residency before removing anything. Obsolete queued mesh work cannot be readmitted while retirement catches up.
- Admitted CPU terrain jobs get a 20 ms head start before duplicate GPU sampling. Ready CPU results cancel pending GPU requests before installation. Slow CPU jobs still allow GPU fallback replacement. Visible fallback replacement retains priority. CPU/GPU sample transfer was not added: CPU samples remain authoritative and GPU readback would add a separate synchronization/lifetime path.
- Sun-shadow distance has its own saved slider/numeric field, capped by render distance. New settings default to 24 m. Existing files migrate to their former detail range, capped at 128 m, and users can override it. Shadow map quality is unchanged. The 0.999 cascade overlap is retained for the existing radial selection/oblique-view coverage; reducing it safely requires changing how coverage is constructed.

The 1.5 ms installation target is **not a hard frame-time guarantee**. Engine operations are atomic and can overshoot. Downstream estimates measure CPU extraction/staging/submission, not GPU completion; extraction timing is conservative rather than exclusive attribution. Terrain command calibration does not measure every other adapter's deferred command batch. Static room-shadow caching and shared CPU/GPU terrain samples are not implemented.

## Validation

- 40 rendering tests: shader validation, page invalidation/residency, camera/depth/shadow contracts, settings serialization and climate approximation.
- 13 terrain tests: retained geometry, camera/shadow visibility, LOD bounds, continuous traversal, bounded retirement and return teleports.
- 15 streaming tests: admission, downstream feedback, worker lifetime, fairness and hierarchy retirement.
- The climate test evaluates 10,000 positions across seeds 42 and 721. The initial half-metre candidate exceeded the 0.002 absolute field tolerance, especially at woodland transitions, so it was rejected. Fine-page errors in the sampled snow/mountain/woodland fields remain below 0.0008 (0.08 percentage points of the full coverage range). The initial failure and refinement logs are retained.

## Measurement protocol

Compare the previous installed full-LTO release in `tools/build/review7-performance/final-v2` against this change's frozen full-LTO release. On the NVIDIA GeForce RTX 5090, both use the same 4K High/FXAA settings, seed 721, render radius, 35 m detail **and 35 m shadow distance**, and two-frame offscreen GPU completion bound. Thus no reported gain can come from shortening shadows. Each run warms up for 60 seconds, measures 8 seconds stationary, then follows the same 16-second route at 40 m/s. Each scene uses before/after/after/before order. Builds and captures run separately from performance measurements. No CPU-idle gate or 30-second runtime cap is used.

Artifacts and every completed run are retained under `tools/build/review8-performance/`. The initial non-LTO smoke test uses an abbreviated warmup and is functional validation only; its timings are not comparable to release results.

## Release results

The strongest repeatable stationary results are mountains (+9.14%) and boreal snow (+7.51%). Forest stationary performance is effectively unchanged (-0.22%). Both updated boreal traversal runs exceeded both baseline runs; the average improved 30.69%, with a lower average p99 frame time (41.55 to 18.10 ms). Spikes are not eliminated: one updated snow traversal still reached 29.48 ms at p99.

Mountain and forest traversal means improved, but individual run ranges overlap substantially. Their typical frame times barely changed (mountains: median 4.85 to 4.90 ms; forest: 5.22 to 5.17 ms). Do not interpret those average FPS gains as a dependable speedup or a general elimination of traversal stutter. These are route-specific measurements on this machine.

| Scene | Phase | Before FPS | After FPS | Change | Before p99 ms | After p99 ms |
|---|---|---:|---:|---:|---:|---:|
| mountains | stationary | 407.55 | 444.80 | +9.14% | 2.67 | 2.60 |
| mountains | moving | 169.40 | 185.60 | +9.56% | 23.93 | 20.21 |
| forest | stationary | 341.55 | 340.80 | -0.22% | 3.46 | 3.54 |
| forest | moving | 167.45 | 181.40 | +8.33% | 19.67 | 19.04 |
| boreal | stationary | 237.80 | 255.65 | +7.51% | 4.42 | 4.17 |
| boreal | moving | 166.35 | 217.40 | +30.69% | 41.55 | 18.09 |

Means of the two runs per version; all individual runs follow.

| Scene | Run | Version | Phase | FPS | Median ms | p95 ms | p99 ms |
|---|---:|---|---|---:|---:|---:|---:|
| mountains | 0 | before | stationary | 406.0 | 2.46 | 2.52 | 2.59 |
| mountains | 0 | before | moving | 195.2 | 4.82 | 7.93 | 9.35 |
| mountains | 1 | final-v2 | stationary | 445.5 | 2.24 | 2.30 | 2.58 |
| mountains | 1 | final-v2 | moving | 195.7 | 4.84 | 7.83 | 8.92 |
| mountains | 2 | final-v2 | stationary | 444.1 | 2.25 | 2.30 | 2.62 |
| mountains | 2 | final-v2 | moving | 175.5 | 4.96 | 8.24 | 31.50 |
| mountains | 3 | before | stationary | 409.1 | 2.44 | 2.51 | 2.76 |
| mountains | 3 | before | moving | 143.6 | 4.88 | 29.82 | 38.50 |
| forest | 0 | before | stationary | 342.3 | 2.91 | 2.99 | 3.42 |
| forest | 0 | before | moving | 147.5 | 5.23 | 25.40 | 31.98 |
| forest | 1 | final-v2 | stationary | 339.6 | 2.93 | 3.04 | 3.56 |
| forest | 1 | final-v2 | moving | 172.0 | 5.20 | 7.15 | 30.78 |
| forest | 2 | final-v2 | stationary | 342.0 | 2.91 | 3.01 | 3.53 |
| forest | 2 | final-v2 | moving | 190.8 | 5.14 | 7.03 | 7.29 |
| forest | 3 | before | stationary | 340.8 | 2.92 | 3.02 | 3.49 |
| forest | 3 | before | moving | 187.4 | 5.21 | 7.12 | 7.36 |
| boreal | 0 | before | stationary | 237.8 | 4.20 | 4.33 | 4.43 |
| boreal | 0 | before | moving | 169.3 | 4.20 | 7.29 | 41.88 |
| boreal | 1 | final-v2 | stationary | 255.3 | 3.91 | 4.03 | 4.19 |
| boreal | 1 | final-v2 | moving | 224.9 | 4.23 | 6.00 | 6.71 |
| boreal | 2 | final-v2 | stationary | 256.0 | 3.90 | 4.02 | 4.15 |
| boreal | 2 | final-v2 | moving | 209.9 | 4.21 | 6.12 | 29.48 |
| boreal | 3 | before | stationary | 237.8 | 4.20 | 4.32 | 4.40 |
| boreal | 3 | before | moving | 163.4 | 4.57 | 11.73 | 41.22 |


## Earlier candidate runs (not pooled with the final release)

The first full-LTO candidate had the same shader changes, but lacked the final reservation fairness fix and still scanned retirement entries while stationary. Its complete mountain ABBA run is retained below and in `tools/build/review8-performance/mountains/all-runs.json`. No run was discarded for poor FPS.

| Run | Version | Phase | FPS | Median ms | p95 ms | p99 ms |
|---:|---|---|---:|---:|---:|---:|
| 0 | before | stationary | 405.8 | 2.46 | 2.52 | 2.61 |
| 0 | before | moving | 195.1 | 4.84 | 7.85 | 9.14 |
| 1 | final | stationary | 446.7 | 2.23 | 2.28 | 2.62 |
| 1 | final | moving | 175.0 | 4.94 | 8.20 | 32.90 |
| 2 | final | stationary | 439.8 | 2.26 | 2.34 | 2.80 |
| 2 | final | moving | 193.7 | 4.93 | 7.82 | 9.09 |
| 3 | before | stationary | 405.4 | 2.46 | 2.53 | 2.81 |
| 3 | before | moving | 172.5 | 4.94 | 8.27 | 35.57 |

The abbreviated non-LTO smoke run is retained in `smoke-mountains/all-runs.json`: 340.6 FPS stationary and 162.6 moving. It includes startup/loading effects and is **not a release performance comparison**. In total, 17 completed offscreen profiler runs are retained: 12 final release, 4 initial candidate/control and 1 smoke.

## Reproducible build

- Before binary SHA256: `d769678c8d12e334b13f852c73d61fe373f1ded5a2fd886246c0ec5f05c93efe`.
- Final binary SHA256: `d31391548b69eebc562847981aab7b082af2db2d8888bd0ad51c4c0accaf5d8e`.
- Final source/assets/compiler invocation: `tools/build/review8-performance/final-v2/`.
- Raw runs, settings, log paths and checksums: `mountains-v2/all-runs.json`, `forest/all-runs.json`, `boreal/all-runs.json` beneath that review directory.
- Measurements retain the previous release's shipping optimization flags (opt-level 3, fat LTO, one codegen unit) and dependencies. The installed binary is copied from the tested snapshot; Cargo fingerprints are not modified.

## Native visual checks and installation

Pinned, settled forest close-up and mountain views were captured at 1920×1080 presentation with 4K world rendering, matching camera positions and shadow distance. Inspection found no new missing terrain, visible cache seams or loss of the existing ground/fracture/snow patterns in these two views. Vegetation, rock contacts and shadows remain present. These checks are visual comparisons, not FPS measurements.

- [Forest before](/home/johnreed/Desktop/projects/Hither/tools/build/review8-performance/native-forest-before/scene.png)
- [Forest after](/home/johnreed/Desktop/projects/Hither/tools/build/review8-performance/native-forest-after/scene.png)
- [Mountains before](/home/johnreed/Desktop/projects/Hither/tools/build/review8-performance/native-mountains-before/scene.png)
- [Mountains after](/home/johnreed/Desktop/projects/Hither/tools/build/review8-performance/native-mountains-after/scene.png)

The tested release is installed at `target/release/hither-sdf`; its checksum matches the final snapshot. All benchmark and capture processes launched for this review have finished. The full test suite and strict Clippy were not run; the 68 targeted tests and runtime/visual checks above are the validation boundary.
