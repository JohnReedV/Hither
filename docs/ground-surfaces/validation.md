# Detailed natural ground surfaces

The shared terrain shader adds layered alpine blades and broad leaves, weathered mineral faces and recessed rock joints, granular scree and dirt, scattered woodland leaves and twigs, and snow drift/crust/granule detail. Normals follow each surface's detail; cliff texture is projected on three axes. Snow coverage, altitude gates, physical terrain and vegetation distances are unchanged. Snow footprints retain their final color, roughness and occlusion treatment.

`GROUND_DETAIL_RANGE = 2.0` applies to ground detail visibility, mountain material octaves and the transition to cached shading. Actual pixel size still filters fine edges. As before, apparent range depends on resolution and viewing angle rather than a universal metre cutoff.

| Existing control | Previous | Updated |
| --- | --- | --- |
| Fine soil/color distance fade | 1.5–6 m | 3–12 m |
| Turf footprint fade | 0.008–0.035 m/pixel | 0.016–0.070 m/pixel |
| Turf layer visibility | 0.22–0.70 cell/pixel | 0.44–1.40 cell/pixel |
| Surface-cache takeover | 0.06–0.15 m/pixel | 0.12–0.30 m/pixel |
| Rock-cache distance fade | detail to detail + margin | twice both distances |
| Existing alpine material octave fades | original footprint | half footprint for visibility |

The visible and compute-baked materials share the same surface functions. Flat snow now receives the bake page's actual texel footprint. The dedicated physical edge filters are deliberately not reduced with the visibility footprint.

## Reproduce

Run `bash tools/smoke_ground.sh plains|alpine|cliff|snow|litter`. Set `HITHER_GROUND_4K=1` for native 3840×2160. These use Xvfb and isolated settings with grass geometry disabled, allowing the ground material itself to be inspected. A second argument adds a capture label.

## Validation

- Shared WGSL surface-baker validation passed (1 test).
- Biome regression tests passed (9), including snow coverage endpoints, snowline transitions, mountain slopes and terrain support.
- Both smoke scripts passed Bash syntax checking.
- Initial hidden alpine check completed at 720p, approximately 60 FPS. Native 4K snow, rock, cliff and woodland captures completed without shader errors or panics.
- 4K snow/rock/cliff were around 11 FPS in this environment. The dense woodland run was slower and had substantial frame-time spikes while compilation was running. These are visual checks, not a controlled full-renderer performance comparison or a 4K performance guarantee.
- Xvfb produced XSETTINGS/display-size warnings and a transient outdated swapchain warning when resizing to 4K.

Final source validation: `cargo build --release` passed. The final shared-baker test passed after the rock and litter refinements. Native 4K captures for the final visible material are saved as `alpine-4k.png`, `cliff-4k.png`, `litter-4k.png`, `snow-4k.png`, and `plains-doubled-range-4k.png`; `overview.png` is a labeled contact sheet. Grass geometry was disabled throughout.

Final 4K visual-check medians (compilation active, so not isolated performance results): plains 89.27 ms; alpine 93.47 ms; cliff 89.73 ms; litter 95.22 ms. All completed without shader errors or panics. No full game test suite or broad moving-camera performance study was run.

Fresh launch of the rebuilt release executable and cache completed successfully in the alpine fixture at 1280×720, render distance 48, detail distance 24: 59.8 FPS stationary, median 16.61 ms, p95 18.48 ms, p99 21.56 ms. Image content was inspected and the run had no errors or panics. Log: `tools/build/occlusion/mountains-ground-alpine-rebuilt-cache/runtime.log`.
