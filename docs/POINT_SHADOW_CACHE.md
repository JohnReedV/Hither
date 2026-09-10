# Point-shadow face reuse

The optimized release passed native visual validation and is installed. No FPS benchmarks were run after the user instructed this change. The in-progress benchmark was terminated immediately; its partial logs remain separate from this change.

Goblin den geometry is static, but the existing renderer clears and redraws all six faces of every admitted point light every frame. This change retains a completed face when all of its potential casters are explicitly static opaque den meshes and the complete shadow inputs are unchanged. Faces containing residents, procedural shaders, unknown casters, or alpha materials retain the ordinary draw path.

The cache stores the actual physical texture and array-face slot with its current light owner. It invalidates on camera-driven light reassignment, texture replacement/resizing, light projection/position/bias changes, caster visibility changes, static transforms or mesh/material handle changes, and mesh/material/image/shader asset events. Asset revisions invalidate conservatively across the cache. A resident entering a previously static face immediately disables reuse; leaving it forces a fresh clear/draw before it can become reusable again.

Only successful, fully prepared shadow draws become cache entries. Pending/failed pipelines and missing render meshes prevent reuse. Occlusion-culling shadow views retain Bevy's full early/late lifecycle. Point-map resolution, shadow filtering, lamp intensity/range, geometry, and residents are unchanged. Directional shadow rendering is untouched. Spot-light rendering retains its normal path.

The integration replaces Bevy's two shared shadow draw wrappers while preserving their early/late render schedule dependencies. It does not modify Bevy's registry source, delete render phases, or cache dynamic shadows. Keeping phases alive preserves their incremental batching state. CPU visibility, specialization/batching, and GPU preprocessing still run; this specifically removes redundant depth clears and geometry draws on eligible static faces. It is not a separate static/dynamic depth-layer cache for faces containing residents.

The native validation uses normal windowed rendering and camera movement, with no FPS collector. An opt-in validation message confirms the real render path reused a completed face. Focused tests exercise static-input stability, transform/asset invalidation, dynamic-caster admission, alpha/unloaded-material rejection, and owner/projection changes.


Completed validation:

- Four cache tests and eight lighting tests passed (`tests-shadow-cache-final.log`, `tests-lighting-shadow-cache.log`).
- Strict binary Clippy passed (`clippy-shadow-cache-final2.log`). Concurrent lint-only edits elsewhere in the checkout were preserved; duplicate annotations were removed.
- A native den run passed startup/render validation and emitted the cache-reuse confirmation. Room geometry/lighting remained intact in the near/return captures (`native-shadow-cache-smoke-v2`). The final release additionally requires empty current/previous pending-shadow queues before retaining a map.
- The initial uninitialized-schedule integration failed at startup; it was corrected to use the initialized schedule API and was never installed. Its diagnostic log is retained.

Artifacts are under `tools/build/review9-performance/`. No performance percentage is claimed for this cache without measurement.


Final release validation: `native-shadow-cache-release` completed without render errors. Its cache-reuse confirmation is guarded by a nonempty static caster list, so it verifies reuse of populated geometry rather than just an empty face. Near/return captures show intact room surfaces and active residents after camera movement. No FPS collector was enabled.

Installed executable: `target/release/hither-sdf`, SHA-256 `13552dc3361d97397ca38bbd3cdfbc9510641faa2b3bc7e2f25b87dfe6477c51`. Source/assets were verified byte-for-byte against `shadow-cache-release` before installation. The previous executable was preserved in `before-shadow-cache-install`. All validation game processes have exited.
