# Rendering and streaming audit

The first three audit findings are supported by the source. This change addresses those findings before attempting the larger shader/storage redesigns suggested in findings 4–8.

- **Surface cache:** camera-distance sorting is not a geometry change. Compare the active clearing sets independent of order. Invalidate only resident pages within a changed clearing's 44 m support, expanded by 0.5 m for baked normal samples. Check both removed and added geometry, including selection changes as the camera moves.
- **First-person shadows:** Bevy 0.19.1 allocates directional cascades for each camera intersecting the sun's render layers. Exclude the hand camera from sun allocation and reuse the world camera's complete lighting bindings after Bevy prepares them. Retain the hand view-uniform offset and its independent depth buffer. Share clustered-light/probe buffers together with their offsets, not just directional matrices. Both cameras have the same pose, XY projection, viewport, HDR, MSAA and depth-capable binding layout. The hand prepass phases are removed before queueing, so its opaque forward pass writes depth without an extra geometry pass. Sun cascades for unlit/non-3D cameras are removed before CPU frustum generation and mesh culling. The world renders before the hands. These compatibility requirements must remain true when changing either camera.
- **Terrain discovery:** replace the eager square scan and mass fallback spawning with a lazy near-to-far traversal. Admit discovery and fallback creation through the same time/unit budget used for mesh installation, with at most 128 discovery steps and fewer than 32 fallback spawns per frame. Preserve unfinished entering-strip discovery across continuous movement, refresh existing LOD requests separately, and retry denied spawns. Existing geometry remains visible while replacements build; missing coverage fills progressively after a teleport or distance increase.
- **Conifer discovery:** evaluate at most 64 candidate sites per worker job, inspect at most 128 queued cells per frame, preserve unfinished discovery and append entering strips on movement. Remember rejected sites as well as accepted ones. Drain obsolete workers and reject results outside the current window. Entity installation remains separately budgeted.

Findings 4–8 describe plausible further optimizations, not measured gains. In particular, zero upload bytes for a GPU-only bake is accurate; its compute cost is separate from upload cost. The existing one-page-per-frame bake cap bounds submissions but is not a GPU-time budget. Near-surface cache use would also need to preserve fine procedural detail currently omitted by the distant bake. Neither should be changed merely to improve a benchmark at the expense of image quality.

## Validation and measurements

54 targeted tests pass, including continuous cell crossings at 1,024 m, cache invalidation, CPU sun-view eligibility and hand prepass suppression. Native checks cover camera-mode changes, independent hand depth and environmental shading beneath a forest canopy. The forest fixture retains the same 5,036 scene instances. Its global visibility count decreases because unused shadow views no longer mark extra meshes visible; the scene remains visually intact.

The final measurements show a modest clearing improvement and better average forest/mountain traversal FPS, but **no reliable snow-forest improvement**. Snow-forest traversal averages 2.9% slower, with worse mean p95/p99 frame times despite a better median. These results do not establish a universal FPS or hitching improvement.

### Conditions

- NVIDIA GeForce RTX 5090; **3840 × 2160 internal world rendering**, 1280 × 720 offscreen output. These are uncapped renderer benchmarks, not display/swapchain FPS.
- Seed 721, FXAA, high texture/shadow quality, grass enabled, maximum FPS 1,000.
- Render/detail distances: clearing 48/24 m, forest and snow forest 260/35 m, mountains 512/35 m.
- 20 s warmup, followed by 8 s stationary and 16 s traversal at 40 m/s. Clearing has two 8 s idle intervals at the same pose; the harness calls its second interval “moving” even though speed is zero.
- Two runs per version per scene. Snow forest was expanded to four because its initial within-version moving-FPS range exceeded 20% of its mean. All four results are retained, including slower runs.
- Both binaries use the same production flags: opt-level 3, fat LTO, one codegen unit, assertions/overflow checks disabled. They contain identical unrelated orc code. The baseline was rebuilt from an isolated copy with these audit changes reverted; it is not an older, unrelated executable.
- Completed uncontended baseline samples were retained while the fixes were refined. Every updated sample below uses the final corrected binary. Runs overlapping compilation or another game/test process were discarded. GPU timestamp instrumentation was disabled for FPS measurements.
- Each table entry is the arithmetic mean of the reported per-run statistic. Percentile columns are means of per-run percentiles, not pooled percentiles. FPS change is `(after / before - 1) × 100`.

### Average FPS and frame times

| Scene | Phase | Before FPS | After FPS | FPS change | Median ms before → after | p95 ms before → after | p99 ms before → after |
|---|---|---:|---:|---:|---:|---:|---:|
| Clearing (first person) | Idle 1 | 395.65 | 410.05 | +3.6% | 2.42 → 2.35 | 3.24 → 3.01 | 3.80 → 3.49 |
| Clearing (first person) | Idle 2 | 394.60 | 408.40 | +3.5% | 2.43 → 2.35 | 3.14 → 3.06 | 3.55 → 3.52 |
| Forest | Stationary | 263.25 | 273.55 | +3.9% | 3.72 → 3.58 | 4.43 → 4.36 | 4.82 → 4.76 |
| Forest | Traversal | 148.10 | 159.95 | +8.0% | 5.59 → 5.30 | 9.88 → 10.06 | 44.01 → 26.87 |
| Snow forest | Stationary | 229.13 | 226.78 | -1.0% | 4.26 → 4.28 | 5.26 → 5.38 | 5.77 → 5.90 |
| Snow forest | Traversal | 151.38 | 146.95 | -2.9% | 5.40 → 5.11 | 17.39 → 21.69 | 39.40 → 43.55 |
| Mountains | Stationary | 359.25 | 368.05 | +2.4% | 2.73 → 2.62 | 3.34 → 3.36 | 3.78 → 3.96 |
| Mountains | Traversal | 123.45 | 135.65 | +9.9% | 7.27 → 6.45 | 12.00 → 11.00 | 38.42 → 36.10 |

### Individual runs

The logs contain the corresponding frame-time percentiles, settings and instance counts. In the clearing rows, both intervals are idle.

| Scene | Version/run | Stationary FPS | Traversal FPS | Raw log |
|---|---|---:|---:|---|
| Clearing (first person) | before 1 | 398.2 | 385.1 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/final/castle-before-0/castle-0/runtime.log) |
| Clearing (first person) | before 2 | 393.1 | 404.1 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/final/castle-before-1/castle-0/runtime.log) |
| Clearing (first person) | after 1 | 408.6 | 399.0 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/castle-after-0/castle-0/runtime.log) |
| Clearing (first person) | after 2 | 411.5 | 417.8 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/castle-after-1/castle-0/runtime.log) |
| Forest | before 1 | 264.8 | 157.2 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/final/forest-before-0/forest-0/runtime.log) |
| Forest | before 2 | 261.7 | 139.0 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/final/forest-before-1/forest-0/runtime.log) |
| Forest | after 1 | 269.8 | 173.0 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/forest-after-0/forest-0/runtime.log) |
| Forest | after 2 | 277.3 | 146.9 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/forest-after-1/forest-0/runtime.log) |
| Snow forest | before 1 | 228.4 | 113.0 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/final/boreal-before-0/boreal-0/runtime.log) |
| Snow forest | before 2 | 226.8 | 173.2 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/boreal-before-1/boreal-0/runtime.log) |
| Snow forest | before 3 | 233.2 | 145.5 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/boreal-before-2/boreal-0/runtime.log) |
| Snow forest | before 4 | 228.1 | 173.8 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/boreal-before-3/boreal-0/runtime.log) |
| Snow forest | after 1 | 225.1 | 120.9 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/boreal-after-0/boreal-0/runtime.log) |
| Snow forest | after 2 | 227.9 | 150.3 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/boreal-after-1/boreal-0/runtime.log) |
| Snow forest | after 3 | 231.8 | 129.6 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/boreal-after-2/boreal-0/runtime.log) |
| Snow forest | after 4 | 222.3 | 187.0 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/boreal-after-3/boreal-0/runtime.log) |
| Mountains | before 1 | 354.3 | 134.2 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/mountains-before-0/mountains-0/runtime.log) |
| Mountains | before 2 | 364.2 | 112.7 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/mountains-before-1/mountains-0/runtime.log) |
| Mountains | after 1 | 362.2 | 147.6 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/mountains-after-0/mountains-0/runtime.log) |
| Mountains | after 2 | 373.9 | 123.7 | [log](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/mountains-after-1/mountains-0/runtime.log) |

[Machine-readable averages](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/summary.json) · [All individual run data](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/all-runs.json) · [Methodology](/home/johnreed/Desktop/projects/Hither/tools/build/audit-performance/verified/methodology.json)

### Reproducibility

The preserved snapshots are in `tools/build/audit-performance/snapshots/before` and `after`, including source, Cargo files and the binaries. Assets are shared through a link to this project's unchanged assets directory. Use `tools/benchmark_render.py` with the corresponding snapshot as `--asset-root` and its binary as `--binary`.

- before binary SHA-256: `4947e5d6f39678500d3c5552e7250e92ba5ddfc8902001b5786f64e683af1a57`
- after binary SHA-256: `931adc1c2abc61113e14b89e0f7bf5a2fafc9335118dd0642208a45f27a9048f`
