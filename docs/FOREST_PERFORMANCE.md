# Forest rendering and performance

The latest terrain/shader/streaming implementation and matched measurements are
in [Rendering performance audit](RENDER_PERFORMANCE_AUDIT.md). The historical
measurements below describe their respective earlier implementations.

## Rendering design

- Orange and boreal forests retain their seeded stands, clearings, placement,
  density and draw distance. Citrus shoots now use five broader alternating
  leaves instead of eight heavily overlapping leaves, at **all** distances.
  The full material, normal map and roughness map remain shared across detail levels.
- Visible botanical meshes no longer also serve as expensive shadow casters.
  Each tree shares a separate porous shadow mesh derived from its own branches,
  leaves or snow-covered sprays. Layer 1 is shadow-only; the gameplay camera
  sees layer 0 and the sun sees both. Trunks and major scaffolds remain.
  Small shadow details are approximated, not rendered as thousands of sub-texel
  faces. The same shadow mesh is used at every distance, without a material swap.
- Two 1024-pixel cascades replace four 2048-pixel cascades. The first covers
  18 meters and the outer still reaches 150 meters. This trades some fine
  shadow resolution for substantially less repeated work.
- A depth prepass rejects hidden foliage before expensive PBR shading.
  The SDF pass also tests that depth before shading hidden ground/sky.
  Grass uses the **same wind displacement** in its depth and color passes.
  FXAA replaces 4x MSAA, avoiding multisampled 4K buffers while retaining edge smoothing.
- Castle rays first intersect a conservative bounding box and are limited by
  scene prepass depth before marching. Terrain uses culled world-aligned 32 m
  tiles. Workers cache elevation/normals; a procedural one-metre fallback covers
  tiles while work completes. Near support retains the one-metre lattice and
  distant LOD is constrained to 4 cm error. Existing valid detail is reused at
  distance-band crossings. Ground color, snow, footprints and cave masks share
  the original shader. See the latest audit for the measured comparison.
- Physics and vegetation sample the same two triangles per grid cell, with a
  bounded thread-local cache of seeded lattice heights. This keeps tree roots,
  grass and player support aligned while avoiding repeated procedural evaluations
  in movement and vegetation placement. Hills and biome masks are preserved;
  the one-metre triangulation approximates the smooth generator.
- Distant ground skips irrelevant castle shadow/AO traces. Zero-weight texture
  detail and absent snow calculations are skipped; camera climate is computed
  once on the CPU.
- Grass retains its density, color variation, bend and wind, but uses two
  curved sections: three triangles/five vertices per blade instead of five/eight.
  Bounded chunks fit 16-bit indices. Four background meshing jobs maximum keep
  biome/collision sampling off the frame thread; obsolete jobs are discarded,
  and only finished, still-relevant chunks reach the renderer.
- Conifer streaming retains the overlap of successive view windows, including
  queued sites, and evaluates only entering rows/columns. A one-cell move at a
  260 m draw distance now evaluates 157 candidate cells instead of 24,649. Tests
  compare the result against full regeneration for movement, resizing, negative
  coordinates and teleports. This preserves all accepted and rejected sites;
  density, detail meshes, draw distance and the per-frame spawn limit are unchanged.
- Fruit remains batched per tree, all tree mesh assets are shared, streaming is
  bounded and nearest-first, and stationary trees do not rewrite mesh handles.
  Lossless upload compaction removes unused vertices and zero-area triangles.
  Visible collision, apple picking, character animation and tracks are unchanged.

## Elevation regression fix (September 6)

The elevation tracer could take up to 2,048 procedural-height steps per screen
pixel. Grazing views across hills amplified that work. A separate CPU spike came
from regenerating the entire conifer candidate window at each cell crossing.
The grid renderer and incremental window updates above remove both sources.

Paired release-build spectator measurements on the RTX 5090, seed 721,
3840×2160 world target, 260 m render distance, 35 m detail distance, high
textures/shadows and FXAA. Each run warms up for 20 seconds, measures eight
stationary seconds, then flies east at 40 m/s for 16 seconds. Forest flight is
at y=24 m and boreal at y=8 m. Runs with overlapping compilers or another game
instance were rejected and repeated. These are full-renderer offscreen timings,
not native-display FPS.

| Biome | Moving FPS before → after | Moving median ms | Moving p95 ms | Moving p99 ms |
| --- | ---: | ---: | ---: | ---: |
| Temperate forest | 44.9 → 73.0 | 21.55 → 13.12 | 27.37 → 18.10 | 51.67 → 19.09 |
| Boreal forest | 40.9 → 81.9 | 24.12 → 12.03 | 31.70 → 17.88 | 48.56 → 19.46 |

Boreal main opaque GPU pass median fell from 16.122 to 4.355 ms. Stationary FPS
rose from 50.9 to 108.0 in temperate forest and 50.0 to 126.9 in boreal forest.
Results depend on route and hardware; these runs establish an improvement on
the sampled hill routes, not a universal minimum FPS. Logs are in
`tools/build/spectator-{forest,boreal}-40-terrain-{before,after}-quiet/runtime.log`.
Additional post-fix runs at 20 m/s measured 85.4 FPS / 13.83 ms p95 in
temperate forest and 95.0 FPS / 13.12 ms p95 in boreal forest. A plains-start
40 m/s route measured 162.3 FPS / 12.01 ms p95. These supplementary runs are
smoke checks, not paired before/after comparisons.
Matching binary/shader snapshots are in `tools/build/terrain-perf-baseline` and
`tools/build/terrain-perf-after`; select both binary and asset root when comparing.

Validation includes 132 passing non-cave regression tests, a successful release
build, and rendered forest/boreal screenshots and a cave-entrance surface smoke check. The separately changing cave test
suite is excluded from this count. Tests cover triangle/support agreement,
negative coordinates, mesh reuse and release on range changes, preserved
clearings, and incremental placement equivalence. Sampled height error against
the smooth generator stays below 8 cm; physics uses the exact rendered triangles.

## Historical flat-world measurements

The native diagnostic on the RTX 5090 at 2560x1440 isolated shadows as the major
cost: the initial orange forest averaged 221 FPS moving; disabling shadows for
diagnosis gave 622 FPS, while disabling grass alone gave 227 FPS. These
diagnostic switches are not enabled in normal play.

Final tests use the actual native display at 3840x2160, borderless fullscreen,
Vulkan, 1000 FPS cap, a four-second warm-up, eight stationary seconds, then a
32-meter automated traverse over eight seconds. Timings include rendering,
presentation and streaming. Results vary with seed/view and desktop scheduling;
an average above 400 FPS is not a guaranteed 2.5-ms worst-case frame.

Verified fixed-view results (yaw 0, pitch 0.06; RTX 5090 / i9-14900KS):

| Forest | Seed | Moving average FPS | Median frame | p95 frame |
| --- | ---: | ---: | ---: | ---: |
| Orange | 721 | 513.6 | 1.89 ms | 2.45 ms |
| Orange | 1973 | 465.2 | 2.16 ms | 2.83 ms |
| Boreal | 721 | 533.2 | 1.82 ms | 2.32 ms |
| Boreal | 1 | 588.8 | 1.66 ms | 2.08 ms |

The profiling camera locks position/orientation to the route after movement
input, so desktop pointer motion cannot silently change the view. Results are
stored in `tools/build/native-*-verified-*.log`. These sampled routes do not
prove a 400 FPS minimum at every possible position, resolution or world seed.

Earlier approximately 66–74 FPS numbers came from Xvfb presentation and are
**not representative native-display FPS**. Do not compare those directly with
these measurements. Triangle counters include shadow views and omit GPU-only
grass and terrain meshes, so they are not total rendered triangles or draw-call counts.

## Reproduce

### Spectator flight without a visible window

The attempted SDF/grass optimizations from September 6 were reverted after a
reported spectator-flight regression. Isolated SDF timings and 4 m/s walking
tests did not establish whole-game performance at long render distances.
A subsequent conifer-placement cache trial was also rejected: at 4K, 260 m
render distance and 35 m detail distance, its 40 m/s route improved the orange
forest but regressed the boreal forest. The terrain renderer and incremental
conifer windows described above supersede the subsequent September 6 per-pixel
elevation tracer and full-window conifer rescans. They do not reinstate the rejected placement-cache trial.

Use the full-renderer spectator benchmark for future changes:

```sh
cargo build --release
bash tools/profile_spectator.sh boreal 40 baseline
bash tools/profile_spectator.sh forest 40 baseline
bash tools/profile_spectator.sh boreal 20 baseline
```

This creates **no OS window** and does not capture desktop input. Both cameras
render to image targets; the world uses the normal mesh, shadow, depth-prepass,
SDF, FXAA, streaming and spectator systems. Settings are isolated in
`tools/build/spectator-<biome>-<speed>-<label>/config`. Defaults are seed 721,
3840×2160 world rendering, high textures/shadows, 260 m render distance, 35 m
detail distance and an 8 m flight height. The logical UI target is 1280×720;
the logged physical window size describes that logical Window resource, not
the world render resolution.

Each run warms up for 20 seconds, samples eight stationary seconds, then flies
east for 16 seconds at the requested speed in real spectator mode. The log
reports average FPS and median/p95/p99 frame time, plus render-pass CPU/GPU
diagnostics. `HITHER_PROFILE_BINARY` selects a comparison build and
`HITHER_PROFILE_ASSET_ROOT` selects its matching asset/shader snapshot;
`HITHER_WORLD_SEED`, `HITHER_PROFILE_HEIGHT`, `HITHER_PROFILE_RESOLUTION`,
`HITHER_PROFILE_RENDER_DISTANCE` and `HITHER_PROFILE_DETAIL_DISTANCE` override
the defaults. Compare identical settings/routes/build profiles in both run
orders, without concurrent GPU work. Include open terrain, both forest types,
20/40 m/s flight and frame-time tails before accepting an optimization.

These timings cover the full renderer without window presentation. They are
not directly interchangeable with native-display or Xvfb measurements.

### Native-display walking benchmark

```sh
cargo build --release
HITHER_PROFILE_FULLSCREEN=1 bash tools/profile_native.sh forest
HITHER_PROFILE_FULLSCREEN=1 bash tools/profile_native.sh boreal
HITHER_PROFILE_FULLSCREEN=1 HITHER_WORLD_SEED=1973 bash tools/profile_native.sh forest seed1973
```

The script opens and focuses a temporary game window on the current display,
uses isolated settings and exits automatically. Do not run concurrent GPU-heavy
work during measurement. Logs in `tools/build/native-*.log` include actual final
physical resolution, mean FPS, median, p95 and p99 frame time. Windowed tests
omit `HITHER_PROFILE_FULLSCREEN`; the default 1280x720 logical window becomes
2560x1440 on this desktop's 2x scaling.

`HITHER_PROFILE_NO_SHADOWS`, `HITHER_PROFILE_NO_GRASS` and
`HITHER_PROFILE_NO_TREES` are opt-in diagnostic ablations used only by the
profiling plugin. The latter two hide currently loaded geometry after warm-up;
newly streamed objects can appear, so use stationary samples for strict
ablation comparisons. Normal play installs none of these profiling systems.

Historical flat-world validation: 68 passing release tests and strict Clippy, including shadow-layer isolation, mesh/asset reuse,
streaming eviction, finite nondegenerate grass, unchanged fruit attachment and
picking, deterministic tree placement and the additional 50% oak reduction.

## Whole-scene occlusion

The world camera uses Bevy's two-phase GPU occlusion culling in addition to
frustum culling. Opaque and alpha-tested geometry contributes actual surface
depth. The GPU tests conservative mesh bounds against a hierarchical depth
buffer and skips completely concealed meshes, including their vertex work.
Objects that become visible are tested against current-frame depth in the late
phase. This applies across the scene, without room flags, collision boxes or
castle-specific visibility rules. Transparent surfaces do not seal openings.
Shadows remain light-dependent: a hidden object can cast a visible shadow.

The custom `SdfMaterial` now participates in the depth prepass. Ray-marched
surfaces use exactly the same geometry and projection as the color pass.
Rasterized terrain and rocks use hardware depth, including per-sample MSAA
coverage, and retain the same cave cutouts and radial cutoff. The mask pipeline
ensures Bevy runs the custom prepass fragment shader rather than substituting a
vertex-only pass. Lighting is excluded from the depth variant.

Shader-positioned terrain and the fullscreen canvas have explicit bounds that
enclose the camera's radial volume; `NoFrustumCulling` alone does not bypass GPU
occlusion. Grass bounds include wind and trampling displacement to avoid
incorrect culling near edges. Ordinary meshes use their actual mesh bounds.

### Reproducing the comparisons

Use one release executable and one frozen asset tree for both modes:

```sh
python3 tools/benchmark_visibility.py --modes legacy gpu --cases blocked forest wall --resolution 6
python3 tools/benchmark_visibility.py --modes legacy gpu --cases blocked forest wall --resolution 12
```

`--binary` and `--asset-root` pin build artifacts; `--repeats 2` reverses the
mode order on the second repetition. The script uses isolated settings and seed
1, holds the camera still, and records mean FPS plus median/p95/p99 frame times.
It disables GPU timing queries for the FPS comparison. Other game instances or
compiler processes cause the measurement to be discarded and retried.

The `blocked` case places an ordinary opaque mesh in front of a forest, independent
of the castle. `forest` checks the cost when that blocker is absent. `wall` checks
the custom ray-marched surface. `turn` rotates through a full circle after the
stationary sample to exercise newly exposed geometry. `mountains` exercises
terrain and rocks. CPU visibility counts do not report GPU occlusion decisions.

`HITHER_VISIBILITY_MODE=legacy|depth|gpu` is honored only with
`HITHER_PROFILE_FOREST` enabled. `legacy` omits custom-surface depth and GPU
occlusion, `depth` includes surface depth without GPU culling, and `gpu` enables
both. Normal gameplay uses `gpu`.

Visual smoke checks use an isolated display and save screenshots/runtime logs:

```sh
bash tools/smoke_occlusion.sh wall
bash tools/smoke_occlusion.sh doorway
bash tools/smoke_occlusion.sh above
bash tools/smoke_occlusion.sh mountains
HITHER_TEST_AA=msaa4 bash tools/smoke_occlusion.sh above msaa4
HITHER_PROFILE_OCCLUDER=1 bash tools/smoke_occlusion.sh forest blocked
```

The earlier comparison between separately built executables has been withdrawn:
concurrent changes and background GPU work made it unsuitable as evidence of an
FPS improvement. GPU diagnostics also need to sum all view measurements from the
same report; taking the last measurement can report the hand camera alone.

### Controlled measurements, 2026-09-06

RTX 5090, seed 1, FXAA, 48 m render / 24 m detail range, offscreen rendering.
Both modes used the same optimized release executable (LTO disabled for this
iteration) and frozen assets. GPU timing instrumentation was disabled. Numbers
below use the final eight-second stationary sample after warm-up and the first
sampling interval. Runs overlapping another game or compiler were discarded.

| 3840 × 2160 view | Legacy FPS | GPU occlusion FPS | Change |
| --- | ---: | ---: | ---: |
| Forest behind an ordinary opaque mesh | 352.6 | 357.9 | +1.5% |
| Fully blocked custom-surface view | 187.8 | 233.5 | +24.3% |
| Open forest | 350.1 | 348.6 | -0.4% |

These are local measurements, not a universal FPS guarantee. Occlusion has an
upfront cost; its benefit depends on concealed geometry, resolution and the
limiting part of the frame. The open-forest result is approximately unchanged in this comparison. Raw logs and frame-time percentiles are under
`tools/build/visibility-v2/final4k/`.

The 11 retained rendering tests passed, including shared mesh/SDF projection,
radial depth cutoff, graphics settings and geometry checks. Grass streaming and
geometry tests also passed in the broad suite. That suite recorded five den
navigation failures and was stopped after four minutes while unrelated traversal
tests were still running; it is not a clean full-suite pass.

The 720p comparison did not produce a complete uncontended pair before repeated
concurrent builds and previews interrupted it. No 720p speedup is claimed.
The benchmark scripts remain available for a quiet-machine rerun.

The final release build and a check of the shared workspace passed. Visual
checks covered an ordinary opaque blocker with foreground foliage, the gate
opening, terrain/rocks, and MSAA4 above the wall. A full camera-turn run completed
without renderer errors. Compiler jobs temporarily paused during validation
were resumed before finishing.

### Sky depth precision regression

The full-screen SDF/sky draws in both the depth and color passes. Independently
compiled float32 projection calculations can disagree by a few ULPs. With
reverse-Z `GreaterEqual`, a slightly smaller color depth fails against its own
prepass depth and leaves clear-color speckles. The existing manual depth-sample
epsilon cannot override that later hardware depth test.

The analytic prepass now moves its distance away from the camera by 64 float32
relative epsilons (about 2.3 mm at 306 m); actual raymarched hits also reserve the
existing hit tolerance. This weakens occlusion conservatively. Final color depth,
radial clipping, rasterized mesh depth, and MSAA sample coverage remain unchanged.
No extra pass or depth readback is introduced.

`python3 tools/check_sdf_depth.py` checks 20,020 distance/angle cases, including
10,187 where equivalent float32 formulas produce different uncorrected depths.
An isolated native 3845×2160 GPU test intentionally factored the prepass
projection differently from the color projection: 8,070 dark pixels in the sky
strip before correction, zero afterward. This is a precision stress reproduction;
the original screenshot's seed/camera trajectory was not available, and the
unmodified shader did not reproduce its intermittent lines in the sampled views.
Runtime assets and captures are retained under `tools/build/sky-lines/` and
`tools/build/occlusion/` (ignored local artifacts).

The production projection formula with the correction also passed a native-4K
rotating spectator capture (69 sky frames, zero detected dark pixels). An MSAA4
runtime check preserved the doorway silhouette and the forest visible through
it, with no shader or renderer errors. These are correctness checks, not new
performance measurements.
A further native-4K spectator run moved at 40 m/s while turning; all 28
captured sky frames were clean and the renderer reported no errors.
