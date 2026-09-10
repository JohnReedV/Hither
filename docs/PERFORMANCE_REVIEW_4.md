# Fourth performance audit: terrain reuse and profiling coverage

The audit identifies real repeated work, but its impact rankings are estimates. This change implements reusable GPU terrain samples, constant-time terrain queue membership, and complete render-preparation stage brackets. It does not implement every proposed renderer redesign. The subsequent [matched release benchmark](PERFORMANCE_REVIEW_4_BENCHMARK.md) finds mixed results: forest and mountains regress, while snow-forest traversal improves. These regressions remain unresolved.

## Changes

### Reuse provisional terrain heights and normals

New terrain tiles still appear immediately. A bounded compute cache now evaluates each requested 33 × 33 vertex lattice once and stores its height and normal. The shared material vertex shader reads these samples in color, depth, and shadow passes. Until a page is ready, the original procedural path remains available. CPU-generated terrain meshes and their retained samples continue to supply final geometry and LODs.

The cache contains 1,024 toroidal slots and uses 17,842,176 bytes (17.02 MiB) for samples, plus 16 KiB of page metadata. A page is usable only when its world origin matches the tile, including at negative coordinates. Reusing a slot for another cell therefore cannot draw another tile's elevation. World-seed changes invalidate all resident samples; added or removed clearings invalidate affected pages, including the half-metre normal-sampling margin. Camera-only underground-state changes do not invalidate height data.

Discovery requests precede the GPU baker. Installing a CPU mesh or evicting a tile removes its request. The scheduler examines at most 128 queued candidates per attempt, prioritizes the view and proximity, and submits at most eight tiles. Nonblocking GPU timestamps adapt the batch toward 0.35 ms; devices without timestamp support retain one tile per batch. The existing shared GPU admission still permits only one custom bake batch per frame. Surface material pages receive alternating terrain turns, and rock bakes retain their existing fairness reservation.

Provisional bounds remain conservative until the CPU mesh provides measured bounds. This patch does **not** read terrain samples back to the CPU or claim to eliminate that part of the audit's culling concern.

### Remove linear terrain queue membership scans

`TileQueue` owns both queue order and a hash set of queued cells. Enqueue, pop, and requeue update them together. Discovery no longer scans the deferred queue for each candidate. Current LOD requirements continue to be recomputed when queued work is consumed.

### Close the render diagnostic gap

With `HITHER_PROFILE_STREAMING=1`, diagnostics now separately report `PrepareAssets`, `PrepareMeshes`, and subsequent preparation through `Prepare`. The boundary between each pair ends one interval and starts the next. View creation is included in the final interval. A schedule regression test exercises the actual Bevy render-set ordering.

These are elapsed stage measurements, not per-streamed-object attribution or GPU upload completion measurements. Bevy's mesh allocator itself runs in `PrepareAssets`; the additional `PrepareMeshes` bracket covers work that previously fell between the reported intervals. The explicit deferred-command flush diagnostic still does not cover earlier automatic flushes.

## Remaining recommendations

| Audit item | Assessment / current scope |
|---|---|
| Downstream admission and adaptive worker concurrency | The 1.5 ms scope does not cover the whole frame. Its existing byte/entity limits remain. Per-batch downstream attribution and an adaptive controller are not implemented here; charging all render preparation to streaming would also charge persistent scene work and could unnecessarily delay coverage. |
| Surface-cache full/blended/missed pixel coverage | Blended pixels do execute both paths. Current transitions preserve image quality. No new per-pixel coverage counters or finer material pages are included. |
| Hardware-filtered texture arrays | A plausible optimization requiring a matched implementation and image/performance comparison. The existing storage-buffer mip filtering remains. |
| Independent shadow distance / cached shadows | Feature and renderer changes, not a safe reduction of current overlap. Radial cascade selection still depends on overlapping coverage. Shadow settings and coverage remain as before. |
| Larger grass regions / persistent allocations | Potentially useful at high detail distances, but requires preserving fine culling and avoiding more expensive partial updates. Current chunking remains. |

## Validation

Validation results and the precise patch are recorded under `tools/build/review4-validation/`. Native captures are under `tools/build/occlusion/`.

All 60 targeted tests pass: terrain (12), rendering (35), streaming admission (12), and render diagnostic ordering (1). `cargo fmt --check` passes. Regression coverage includes queue deferral/reentry, negative cache coordinates, toroidal collisions, local clearing invalidation, seed changes, and ignoring camera-only underground-state changes.

The forced-fallback mountain comparison uses camera position (-1408, 160, 464), exercising negative coordinates. Cached versus original procedural captures match exactly at 916,041 of 921,600 pixels (99.397%). Only 72 pixels have any channel differing by more than one 8-bit level; mean absolute channel difference is 0.002291/255. The terrain silhouette and surface appearance match on inspection. This is one controlled view, not a universal pixel-equivalence claim.

The final unit-test binary contains the seed-x-only invalidation and diagnostic setup refactoring. The native snapshot predates those two CPU-only refinements and uses the same GPU shader and bake path; both source manifests are recorded in `manifest.json`.

The first normal forest smoke test passed native shader validation. Across 66 terrain bake batches, measured GPU duration ranged from 0.0230 to 0.1833 ms, with a median of 0.1244 ms. These are **bake durations**, not before/after frame-performance numbers.

Two diagnostic switches allow the terrain paths to be compared without CPU meshes masking the result:

- `HITHER_PROFILE_FORCE_TERRAIN_FALLBACK=1`: retain provisional terrain geometry during a profile.
- `HITHER_PROFILE_NO_TERRAIN_CACHE=1`: leave those tiles on the original procedural vertex path.

Native smoke tests use a diagnostic executable built with optimization level 3 and LTO disabled, a fixed camera, seed 721, 4K internal resolution, High shadows, 260 m render distance, and 35 m detail distance. They are correctness checks, not the previous matched release benchmark protocol. A build ran alongside one diagnostic capture; its printed FPS must not be used as a performance comparison. No CPU preflight checks, rejection filters, or benchmark retries have been reintroduced.

The completed review-3 FPS results remain in `PERFORMANCE_REVIEW_3.md` and do not measure this patch.

The subsequent release benchmark runs the final tested source with fat LTO and records 20 completed, unfiltered runs. See [the FPS comparison](PERFORMANCE_REVIEW_4_BENCHMARK.md) for raw values, frame-time percentiles, and individual logs.
