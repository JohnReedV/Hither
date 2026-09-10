# Foliage and distance consistency follow-up

The previous foliage correction fixed inward-facing normals in the shared crown
builder. It applied to all users of that builder, but did not solve conifer
coverage: separately clustering needles and snow into solid volumes changed the
ratio of white snow to green needles and filled the gaps between branches.

## Changes

- Winter and alpine conifers use their authored reduced branch/snow meshes at
  distance. Reduced needles preserve source normals and colors. These meshes
  still reduce needle triangles sixfold, without the solid-volume approximation.
- Oak reduced leaves also preserve source shading normals. Broadleaf crown
  clustering retains the previous shared outward-normal correction.
- Tree and rock visual LOD use the actual three-dimensional camera position.
  Gameplay residency remains player-centered, with streaming padding.
- Rock mesh selection and small-rock visibility/residency follow detail distance.
  Stationary setting edits rebuild the detail window independently of large rocks.
- Static terrain and rock surface caches blend in beyond detail distance, with
  footprint filtering and procedural fallback when cached detail is insufficient.
- Directional shadow range is `min(detail distance, render distance)` for all
  material families. Quality selects map resolution. Cascade selection uses
  radial distance instead of camera-forward depth, blends over the last 20% of
  the first cascade, and fades shadows over the last 15% of the total range.
  Both cascade projections include the near volume to support radial selection.

Render distance remains the outer visibility boundary. Detail distance controls
fine visual content inside that boundary; distant silhouettes remain simplified.
Whole-object bounds and hysteresis avoid repeated mesh switching at a threshold.
Texture mip and subpixel filtering still follow projected size. Collision and
NPC gameplay distances are independent of visual detail.

The shared directional-shadow function is replaced in Bevy's existing shader
asset, preserving its import identity and dependency metadata. It is based on the
pinned Bevy 0.19.1 implementation; an engine upgrade must review the replacement.
It adds no shadow maps or geometry draw passes. The coverage overlap and greater
selected shadow range can increase caster work; conifer far meshes also contain
more triangles than the incorrect volume approximation.

## Validation

- 107 focused tests passed, one ignored. Includes exact normal/color preservation,
  stationary rock-detail edits, settings constraints, streaming, and renderer tests.
- `cargo fmt --check` passed.
- Four isolated rendered captures passed with no shader validation errors or
  panics: old/new at detail 205, new at detail 44, and a rotated camera.
- Frozen sources, captures, diagnostic binaries, and raw timings live under
  `tools/build/distance-consistency/`.

## Performance

RTX 5090, i9-14900KS; 3840×2160 world target, high quality/FXAA, render 512, detail 205, seed 721. Diagnostic optimized builds use identical compiler overrides (LTO off, 16 codegen units); these are comparative checks, not shipping-build FPS promises. The final offscreen presentation target is 1280×720. Traversal is 40 m/s for 16 seconds; stationary sampling is 8 seconds. Runs reject overlapping game/build processes.

### Initial repeated runs (20-second warm-up)

| Scene / phase | Before FPS (runs) | After FPS (runs) | Before median ms (runs) | After median ms (runs) |
|---|---|---|---|---|
| boreal / stationary | 142.0, 140.7 | 99.4, 142.4 | 7.01, 7.04 | 6.95, 6.93 |
| boreal / moving | 48.7, 50.5 | 63.8, 66.0 | 17.55, 17.62 | 14.84, 15.19 |
| forest / stationary | 111.7, 83.8 | 81.2, 82.3 | 7.77, 7.97 | 8.19, 8.12 |
| forest / moving | 51.8, 51.8 | 52.1, 50.9 | 17.4, 17.78 | 17.91, 17.94 |

The initial stationary runs contain inconsistent long-frame spikes in both versions. They do not support a precise sustained FPS percentage. The additional comparison below uses a 60-second warm-up to separate settled rendering from initial preparation. All original logs are retained.

### Additional settled comparison (60-second warm-up)

| Scene / phase | Before FPS | After FPS | FPS change | Median ms before → after | p95 ms before → after |
|---|---:|---:|---:|---|---|
| boreal / stationary | 141.0 | 141.2 | +0.1% | 7.0 → 7.02 | 8.53 → 8.62 |
| boreal / moving | 47.6 | 65.2 | +37.0% | 17.4 → 15.39 | 24.5 → 20.75 |
| forest / stationary | 125.6 | 91.3 | -27.3% | 7.9 → 7.93 | 9.31 → 47.85 |
| forest / moving | 56.0 | 45.0 | -19.6% | 17.99 → 18.36 | 25.3 → 27.41 |

This is a limited fixed-route comparison. It does not guarantee unchanged performance for every world, view, or distance setting.

### Broadleaf repeat, reverse order

The same 60-second warm-up case was repeated once in reverse order (after, then before) because the first pair contained large spikes. This final pair was stable. It does not erase the earlier spiky results.

| Phase | Before FPS | After FPS | Change | Median ms before → after | p95 ms before → after |
|---|---:|---:|---:|---|---|
| stationary | 126.2 | 123.5 | -2.1% | 7.84 → 8.07 | 9.43 → 9.29 |
| moving | 56.7 | 54.0 | -4.8% | 17.93 → 17.74 | 24.52 → 28.05 |

The stable comparison shows negligible stationary cost for conifers and a modest broadleaf cost (2.1% stationary, 4.8% traversal). Earlier runs include substantially worse averages from intermittent spikes, so these stable results cannot establish an across-the-board FPS guarantee or rule out a streaming regression. Larger detail settings now request larger shadow and small-rock coverage, which can add work.
