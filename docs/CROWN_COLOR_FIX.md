# Crown LOD color continuity

The clustered foliage mesh had inward triangle winding. Its averaged vertex
normals therefore also pointed inward. The shared geometry builder emits tangent
attributes, and Bevy's standard-material shader does not apply the double-sided
normal flip for meshes with those attributes when the normal map is absent.
Distant crowns consequently received lighting from the wrong side, producing the
dark blue/green band at the detail boundary.

The fix reverses the crown triangle winding before the existing normal/tangent
calculation. It does not compensate with an arbitrary material tint. Source
texture averages and foliage colors remain unchanged. No vertices, triangles,
draw calls, material variants, texture samples, transition passes, or per-frame
selection work were added. Render distance, detail distance, collision and shared
mesh caching are unchanged.

A regression test checks outward face winding, outward shading normals, and the
unchanged eight-triangle cluster count. All four crown-LOD tests pass, including
color averaging, bounded geometry, and stationary/moving selection. `cargo check
--tests` and formatting pass.

Matched screenshots use seed 721, camera (192,45,80), yaw -1.5708, pitch -0.2,
512 m render distance, and 44 m detail distance. A 256 m detail reference verifies
the foliage hue against authored leaves. Screenshots and logs are under
`tools/build/crown-color/{before,after,detailed-reference}/capture/`.

The performance check compares frozen before/after sources with identical
opt-level 3, LTO-off, 16-codegen-unit diagnostic builds. These FPS values must not
be compared directly with the previous full-LTO renderer-upgrade benchmarks.
The normal-direction correction is the only runtime code difference in this pair.

## Matched 4K forest measurements

Two runs per version, seed 721, 260 m render distance, 35 m detail distance,
FXAA, high shadows, RTX 5090 / i9-14900KS; 20 s warm-up, 8 s stationary,
16 s traversal at 40 m/s. Values are averages of recorded run summaries.

| Phase | FPS before | FPS after | Median ms before | Median ms after | p95 ms before | p95 ms after |
|---|---:|---:|---:|---:|---:|---:|
| stationary | 258.2 | 256.5 | 3.81 | 3.85 | 4.63 | 4.59 |
| moving | 128.3 | 140.7 | 5.92 | 5.74 | 11.50 | 10.69 |

Traversal results vary with streaming completion and have noisy tail frames;
two repeats do not establish a precise percentage gain or loss. Raw run summaries
and logs are retained under `tools/build/crown-color/`.
