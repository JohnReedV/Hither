# Grass ground material

The terrain shader now uses overlapping, randomly oriented tapered fibres, dry stems, dark interstices, and small normal perturbations. Isotropic clump variation replaces square value-noise patches. Fine detail is filtered by world-space pixel footprint and blended with snow, woodland, slope and alpine coverage. The same broad color functions feed the distant surface cache.

Changed `assets/shaders/sdf_scene.wgsl`. Extended `tools/smoke_occlusion.sh` with a plains preview and `HITHER_TEST_GRASS_BLADES` setting to inspect the base surface independently of grass geometry.

## Validation

- `cargo build --release`: passed.
- `cargo test shared_surface_baker_validates_with_climate_payload -- --nocapture`: passed (1 test).
- `bash -n tools/smoke_occlusion.sh`: passed.
- Hidden plains and woodland captures inspected.
- Final release focused check, seed 1, pose `144,1.25,160,0,-0.6`, 1280x720, render distance 48, detail distance 24, grass blades disabled: completed without error; 60 FPS, stationary median 16.68 ms, p95 18.31 ms, p99 23.68 ms. This is a stationary smoke check, not a full performance benchmark.
- A longer final-release check at render distance 256 produced the saved after image, then panicked while accessing mesh attributes after extraction to RenderWorld. This limits broader runtime validation; the grass shader compiled and rendered, but the longer run was not clean. Log: `tools/build/occlusion/plains-grass-final/runtime.log`.
- Xvfb emitted XSETTINGS and physical-display-size warnings. No desktop window or user settings were changed.

`before.png` and `after.png` show the same seed, pose and resolution with grass geometry disabled. No full suite or moving-camera shimmer assessment was performed.

## Second detail pass: native 4K

Three independently filtered scales now combine folded blades, dry stems, broad three-leaf rosettes with veins and crescents, fine grass, and granular ground. Surface gradients follow each blade's orientation. The shared coarse/cache color remains unchanged in this pass; the changed function is evaluated by the live terrain fragment shader.

- Shared surface-baker validation test passed again (1 test).
- Final live shader rendered cleanly at 1280x720 and native 3840x2160, with grass geometry disabled, seed 1, pose `144,1.25,160,0,-0.6`, render distance 48 and detail distance 24.
- 720p stationary: 60.0 FPS, median 16.65 ms, p95 17.23 ms, p99 19.20 ms.
- Matched 4K previous material: 12.0 FPS, median 82.82 ms, p95 83.81 ms, p99 90.50 ms.
- Matched 4K refined material: 12.0 FPS, median 83.25 ms, p95 84.43 ms, p99 88.10 ms.
- These single stationary measurements establish visual operation and approximate comparative cost; they do not establish smooth 4K gameplay or moving-camera stability. The broader renderer's 4K performance remains a limitation.
- Native image: `refined-4k.png` (3840x2160). Unscaled crop: `refined-detail.png` (1200x900).
- Xvfb warnings included display sizing/XSETTINGS and a transient outdated swapchain during the 4K resize. Both runs completed without runtime errors.
