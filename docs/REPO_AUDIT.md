# Repository audit — 2026-09-04

## Cleanup

- Removed unused procedural-avatar WGSL helpers, facing/animation uniforms, scene-time accumulation and obsolete movement blend/phase state. The player-position uniform remains because it drives the ground contact shadow.
- Removed superseded Blender hood generation, face UV warping and arm rotations that were immediately overwritten by the IK solution. Saved animation/source equivalence checks confirm the arm cleanup preserves the poses.
- Moved the unused skin texture, old Blender backup, historical build diagnostics, logs and Python bytecode cache outside the repository. Removed the empty `assets/skins` directory.
- Kept the runtime GLB, editable Blender template, consumed source textures/landmarks, optional portrait-measurement tools, current documentation previews, vendored authoring dependencies and Rust compilation cache.

Recovery folder for the moved files (approximately 206 MiB):
`/home/johnreed/Desktop/projects/Hither-audit-recovery-zbUGgO`

The old `tools/build` directory is preserved as `build/` there. New test outputs regenerate in the repo's ignored `tools/build/`. This reduces repository clutter, not total disk usage: the recovery files still occupy disk. No permanent deletion of material assets was performed.

## Optimizations

- Position HUD refreshes at 10 Hz and only marks text changed when its displayed value changes. Apple HUD tracks counts rather than the orchard timer's change tick. FPS visibility/text also avoid redundant change notifications.
- Identical SDF uniforms no longer mark the material modified, avoiding redundant extraction/upload on unchanged frames. Removed 32 bytes of unused uniform fields.
- Mesh camera, SDF renderer and apple picker share one camera obstruction sweep instead of recomputing it independently.
- Symmetry reduces castle primitive evaluations from 16 to 10 per scene sample, preserving walls, gate and crenellations. Apple tracing iterates only over present apples.
- Smoke tests seed their own isolated windowed settings and work after generated diagnostics are removed.

These are structural reductions in work; an FPS speedup has not been benchmarked. Rendering quality, gameplay speed, apple growth probability and input bindings are unchanged.

## Verification

- `cargo clippy --all-targets -- -D warnings` passes. Targeted exceptions document intentional Bevy system parameter/query complexity and conventional camera enum names; correctness/style warnings were fixed.
- `cargo test --release`: 12 tests pass, including mirrored-distance equivalence, HUD change detection, turn-in-place behavior, avatar profile checks and apple picking/occlusion.
- `cargo build --release` passes.
- `bash tools/smoke_avatar.sh` passes with no shader errors or runtime panics; rendered front view inspected.
- `tools/.venv/bin/python tools/check_avatar_animation.py` passes loop, foot-target, connected-joint and saved-animation/source equivalence checks.
- Python authoring modules compile successfully. The active GLB/template were not regenerated or modified by this audit.
