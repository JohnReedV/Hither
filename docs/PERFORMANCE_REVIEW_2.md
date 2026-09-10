# Second rendering and streaming review

The two reviews identify real scheduling and repeated-work problems. The implemented changes preserve placement rules, grass density, authored mesh attributes, and the narrow-phase cave cutout equations.

## Implemented

- Alpine, citrus, rock and understory placement runs through bounded shared workers. Discovery inspects at most 128 cells per admission and submits at most 32 cells per worker. It remembers rejected cells, preserves entering strips across movement, and discards obsolete results. Pending output is bounded. Fallen-log discovery is also incremental.
- Installation has a separate 256-entity admission limit for the converted adapters. The 4 MiB upload cap no longer has a priority-layer exemption. Goblin geometry is partitioned by 32 m spatial bins with at most 12,000 triangle indices per piece; vertex attributes, including normals and tangents, are copied exactly. Pieces and lamps install progressively. Fallen logs install individually instead of uploading an entire chunk atomically.
- Terrain CPU baking now admits tiles intersecting sun-shadow frusta outside the gameplay-camera frustum. UI-camera visibility cannot make a tile eligible. Those tiles previously could retain the procedural vertex fallback indefinitely.
- Grass retains its desired-cell map, processes entering strips and conservative LOD distance bands, and replaces meshes on existing entities. Active trample contacts get an aggregate bound; vertices outside it skip both contact loops.
- Rock LOD updates visit eligible nearby cells. Alpine/conifer updates select roots in the old/new near-detail neighborhoods. Shared crown selection schedules reevaluation from distance to the nearest LOD boundary and accumulated camera travel, while independently handling changed/new meshes, transforms, settings, and completed shared builds. A pending worker no longer forces population-wide reevaluation.
- Surface pages bake incrementally in 8–64-row stripes. A reused page is invalidated before the first write and published only after all texels and mips are complete. GPU timestamps, when supported, adjust stripe size toward a 0.35 ms target. This is feedback admission, not a hard real-time guarantee. Without timestamp support, the cap remains eight rows. Uniform buffers and bind groups are reused. Rock surfaces use reusable input/uniform buffers and bounded vertex batches; their allocation is exposed only when all data is ready.
- Cave cutouts use a conservative 16×16 world-space grid of candidate bitmasks. Empty bins return without scanning the global lists. Color, depth, and shadow variants use the same masks and exact narrow phase. Positions outside the grid safely fall back to the full lists.
- The sky-irradiance grid is world aligned. Workers reuse overlapping samples and evaluate entering strips; the previous image remains active until a replacement is admitted. Worker lifetime and image upload use the shared coordinator.

## Measurement limits and remaining design proposals

The command-recording timer is still not a guarantee of total frame time. Entity and byte caps constrain deferred work; opt-in `HITHER_PROFILE_STREAMING=1` reports the explicit final deferred flush and render asset/preparation stages separately. Bevy can flush commands earlier to satisfy dependencies, so the final-flush metric must not be described as all ECS command application. `HITHER_PROFILE_BAKES=1` logs supported surface-bake GPU timestamps. Whole-frame percentiles remain the end-to-end measure.

The reviewers' proposals for filterable texture arrays, broader base-property cache coverage, reusable temporary terrain height data, and static shadow-map reuse require additional renderer design and validation. This change does not relax cache quality thresholds or remove fine procedural detail. Rock compute has a work-count cap rather than GPU-headroom feedback. The fixed GPU bake target is not a measurement of the frame's remaining GPU headroom. These are remaining opportunities, not claimed completed optimizations or guaranteed FPS gains.

## Validation and benchmark results

134 targeted tests pass; the existing optional log contact-sheet generator remains ignored. Native forest and goblin-settlement smoke runs pass without renderer validation errors; the forest screenshot preserves the visible scene. Surface-bake validation recorded 143 batches: 0.0428 ms median, 0.0926 ms p95, and 0.114 ms maximum. These are bake-only measurements, not whole-frame gains. Matched whole-frame results are pending CPU-isolated runs. The baseline is the source and assets at the start of this review, including the previous audit fixes. The earlier review's measurements are not reused as this baseline.

### CPU isolation

Earlier provisional runs have been archived under `tools/build/review2-performance/cpu-unverified` and are excluded from the final comparison. Another agent is compiling and testing Subtensor on this machine. The benchmark now requires 30 consecutive quiet seconds before launch: no compiler, Cargo, Subtensor executable, or competing game, total background CPU at most 100% of one logical core, and no individual background process over 50% of one logical core. During each run it samples process CPU every 0.5 seconds, excludes the benchmark game itself, and rejects runs that violate those checks. Each attempt records the CPU evidence in `cpu-attempt-N.json`. Desktop services remain running; “clear” means low measured background load, not zero operating-system activity.

Final cumulative measurements, including subsequent reviewer fixes, are recorded in [the follow-up report](PERFORMANCE_REVIEW_3.md). Earlier provisional or screened measurements are not used in that final comparison.
