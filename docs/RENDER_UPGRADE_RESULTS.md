# Projected rendering: measured results

Measured on an NVIDIA RTX 5090 (driver 580.173.02) and Intel Core i9-14900KS.
Both versions use the same optimized Rust profile: opt-level 3, fat LTO,
one codegen unit, and no debug assertions. Source and asset snapshots are
preserved under `tools/build/render-upgrade/before` and `after`, with SHA-256
manifests alongside them. Unrelated concurrent cave/art changes were excluded
from the comparison snapshot; the live workspace retains those changes.

Each case uses seed 721, a 3,840 × 2,160 world render target, FXAA, high shadow
and texture quality, grass enabled, and 35 m detail distance. The final UI target
is 1,280 × 720. Rendering is offscreen to remove presentation/VSync limits.
Each independent process warms up for 20 seconds, samples a stationary view
for 8 seconds, then travels east at 40 m/s for 16 seconds. Each case is repeated
twice. The benchmark rejects runs overlapping Rust compilation or another game
process. These are whole-frame measurements, not isolated GPU pass timings.

The comparison includes the new shadow reception and unified tone mapping;
it is not a pixel-identical image-quality comparison. It measures the combined
implementation, not an attribution of speedup to each individual change.
Fixed warm-up routes test the practical streaming behavior at each distance;
they do not prove that every distant background job has settled.

FPS values below average the two recorded runs. Mean frame time is 1,000/FPS
and is approximate because the original logger rounds FPS to one decimal.
FPS change is `(after / before - 1) × 100`; negative numbers are regressions.

| Scene | Phase | Before FPS | After FPS | FPS change | Before frame ms | After frame ms | Before p95 ms | After p95 ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Forest, 260 m | stationary | 200.4 | 289.5 | +44.5% | 4.99 | 3.45 | 5.10 | 3.95 |
| Forest, 260 m | moving | 103.4 | 169.1 | +63.5% | 9.67 | 5.91 | 11.86 | 9.41 |
| Boreal, 260 m | stationary | 184.6 | 362.9 | +96.6% | 5.42 | 2.76 | 5.53 | 3.27 |
| Boreal, 260 m | moving | 119.8 | 154.2 | +28.8% | 8.35 | 6.49 | 21.75 | 10.25 |
| Mountains, 512 m | stationary | 233.7 | 379.6 | +62.4% | 4.28 | 2.63 | 4.36 | 3.01 |
| Mountains, 512 m | moving | 114.2 | 113.1 | -1.0% | 8.76 | 8.84 | 26.45 | 13.96 |
| Forest, 1,024 m | stationary | 37.7 | 115.3 | +205.8% | 26.53 | 8.67 | 28.98 | 11.97 |
| Forest, 1,024 m | moving | 27.6 | 35.3 | +27.7% | 36.17 | 28.33 | 48.49 | 41.31 |

The largest measured gain is the stationary 1,024 m forest: 37.7 to 115.3 FPS
(+205.8%). Traversal gains range from +27.7% to +63.5% in the forest cases.
Mountain traversal changes from 114.2 to 113.1 FPS (-1.0%), which is small relative
to baseline run variation; its mean recorded p95 drops from 26.45 to 13.96 ms.
These outcomes support the long-distance geometry/selection changes, but do not
establish a universal speedup or attribute gains to any single subsystem.

The final comparison includes the last CPU optimization: settled foliage skips
redundant LOD selection, and shared crown generation is admitted once per source
mesh. Preliminary implementation results are retained under `after-v1-results`
and are not the results reported in the table above.

## Individual recorded runs

These are the original logged summaries. Small differences should be interpreted
in light of run-to-run variation, particularly during traversal. Two repeats are
not a confidence interval or a guarantee on other hardware.

| Scene | Phase | Version | Run | FPS | Median ms | p95 ms | p99 ms |
|---|---|---|---:|---:|---:|---:|---:|
| Forest, 260 m | stationary | before | 1 | 201.4 | 4.96 | 5.06 | 5.55 |
| Forest, 260 m | stationary | before | 2 | 199.3 | 5.01 | 5.14 | 5.66 |
| Forest, 260 m | stationary | after | 1 | 291.3 | 3.39 | 3.91 | 4.23 |
| Forest, 260 m | stationary | after | 2 | 287.6 | 3.41 | 3.99 | 4.30 |
| Forest, 260 m | moving | before | 1 | 97.8 | 10.10 | 11.91 | 59.63 |
| Forest, 260 m | moving | before | 2 | 109.1 | 9.34 | 11.81 | 13.29 |
| Forest, 260 m | moving | after | 1 | 183.6 | 4.71 | 8.92 | 10.45 |
| Forest, 260 m | moving | after | 2 | 154.7 | 5.14 | 9.90 | 36.33 |
| Boreal, 260 m | stationary | before | 1 | 185.1 | 5.40 | 5.50 | 5.90 |
| Boreal, 260 m | stationary | before | 2 | 184.1 | 5.43 | 5.56 | 5.95 |
| Boreal, 260 m | stationary | after | 1 | 364.1 | 2.74 | 3.28 | 3.64 |
| Boreal, 260 m | stationary | after | 2 | 361.7 | 2.76 | 3.27 | 3.70 |
| Boreal, 260 m | moving | before | 1 | 131.6 | 6.84 | 10.69 | 36.25 |
| Boreal, 260 m | moving | before | 2 | 107.9 | 6.51 | 32.82 | 67.94 |
| Boreal, 260 m | moving | after | 1 | 173.3 | 5.06 | 9.35 | 11.40 |
| Boreal, 260 m | moving | after | 2 | 135.1 | 5.40 | 11.16 | 66.60 |
| Mountains, 512 m | stationary | before | 1 | 234.2 | 4.27 | 4.36 | 4.81 |
| Mountains, 512 m | stationary | before | 2 | 233.2 | 4.29 | 4.35 | 4.86 |
| Mountains, 512 m | stationary | after | 1 | 382.1 | 2.60 | 2.98 | 3.40 |
| Mountains, 512 m | stationary | after | 2 | 377.1 | 2.63 | 3.04 | 3.44 |
| Mountains, 512 m | moving | before | 1 | 117.8 | 6.30 | 13.84 | 49.46 |
| Mountains, 512 m | moving | before | 2 | 110.6 | 5.85 | 39.06 | 50.02 |
| Mountains, 512 m | moving | after | 1 | 113.4 | 8.79 | 14.09 | 16.38 |
| Mountains, 512 m | moving | after | 2 | 112.8 | 8.79 | 13.83 | 16.70 |
| Forest, 1,024 m | stationary | before | 1 | 37.6 | 26.70 | 28.99 | 29.04 |
| Forest, 1,024 m | stationary | before | 2 | 37.8 | 26.57 | 28.97 | 29.10 |
| Forest, 1,024 m | stationary | after | 1 | 115.0 | 8.38 | 12.00 | 17.91 |
| Forest, 1,024 m | stationary | after | 2 | 115.6 | 8.33 | 11.95 | 14.05 |
| Forest, 1,024 m | moving | before | 1 | 27.7 | 35.21 | 48.37 | 51.51 |
| Forest, 1,024 m | moving | before | 2 | 27.6 | 35.33 | 48.61 | 51.75 |
| Forest, 1,024 m | moving | after | 1 | 35.4 | 26.23 | 41.30 | 46.54 |
| Forest, 1,024 m | moving | after | 2 | 35.2 | 26.77 | 41.32 | 46.36 |

## Validation and implementation limits

- 105 focused regression tests passed; one expensive existing test was ignored.
- The broader suite recorded 267 passes, two ignored tests, and two Orc encounter
  failures under concurrent test load. Both encounter tests passed isolated runs
  on the untouched baseline and updated game. See `all-regressions.log`,
  `before-tests/encounter-tests.log`, and `encounters-isolated.log`.
- `cargo check --tests` and formatting checks passed.
- Native UI smoke passed minimum/maximum render distance, resolution changes,
  MSAA 2/8/off, FXAA, native-resolution restoration, and restart.
- Headless mountain traversal validated the final compositor and surface/shadow
  shaders. Near-ground and aerial screenshots were inspected.
- Installation and upload budgets are cooperative; a single indivisible job
  may overshoot. Deferred ECS application and driver time are outside the timer.
- Terrain uses a mipmapped static surface clipmap; rocks use a bounded vertex
  surface cache. Lighting, footprints and collisions remain independent.
- Static shadow-map reuse and dynamic resolution are not enabled. Moving cascade
  projections require explicit invalidation before shadow maps can safely be reused.

See [implementation details](PROJECTED_RENDERING.md). Raw benchmark summaries,
runtime logs, build commands, source manifests, and machine-readable comparison
are under `tools/build/render-upgrade/`. Reproduce each version with:

```sh
python3 tools/benchmark_render.py \
  --binary tools/build/render-upgrade/VERSION/bin/hither_sdf-264370f2a5a26f57 \
  --asset-root tools/build/render-upgrade/VERSION \
  --output tools/build/render-upgrade/VERSION-results \
  --repeats 2 --cases forest boreal mountains forest-far
```

Replace `VERSION` with `before` or `after`. The repository's compiler profiles
were not weakened for the comparison. Diagnostic builds were used only for
functional validation.
