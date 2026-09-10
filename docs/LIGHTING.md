# Lighting architecture and authoring

Hither uses Bevy 0.19.1 clustered forward PBR for terrain, rocks, vegetation,
characters and first-person hands. Procedural materials supply surface properties;
Bevy supplies local-light evaluation, attenuation, shadows and exposure. The sky
uses the same sun direction as the world light. World and hand HDR passes are
composed before a restrained bloom and the existing final TonyMcMapface tone map.

## Make an item emit light

```rust
use crate::rendering::lighting::LightEmitter;

commands.entity(item).insert(LightEmitter::new(800.0));
```

Power is lumens. The component owns one child point light, follows the item's
hierarchy and cleans up when removed or despawned. It requires a transform and
visibility, so it also works on an otherwise empty item root. Changing `enabled`
to false or setting `lumens` to zero removes illumination immediately. Inventory
systems should disable unequipped emitters or hide their inventory hierarchy;
this renderer does not infer gameplay inventory state.

Optional controls:

```rust
LightEmitter {
    color: [1.0, 0.68, 0.34], // sRGB authoring color
    range: 14.0,              // metres; zero derives a bounded range from power
    radius: 0.08,             // source radius, not the influence range
    offset: [0.0, 0.25, 0.0], // local emission anchor
    priority: 1.5,            // budget importance, default 1
    glow: 1.0,               // zero preserves the original material emission
    ..LightEmitter::new(18_000.0)
}
```

The type supports serde with defaults. An item definition can deserialize
`{"lumens":800,"color":[1.0,0.68,0.34]}`. No new inventory/file-format system is
required by the renderer.

If the emitter entity has a `MeshMaterial3d<StandardMaterial>`, it receives an
isolated emissive material variant. Its shared source material is never changed,
and removal restores the original handle. Luminous surfaces do not cast an
opaque shadow over their own light; removal restores their previous caster policy.
Attach glow to the luminous part of a large item, because that mesh is excluded
from shadow casting. An emitter on a parent root illuminates the world without making every child part glow: retain authored flame/glass/ember
materials on those children. This avoids glowing wood or metal handles.

## Cost and visibility policy

One global scheduler replaces the separate den/torch shadow rules. Quality uses
the existing Shadows setting:

| Quality | Admitted local lights | Point-shadow maps | Face resolution |
| --- | ---: | ---: | ---: |
| Low | 64 | 1 | 512 |
| Medium | 128 | 2 | 1024 |
| High | 256 | 4 | 1024 |

These are capacity limits, not frame-rate guarantees. Point shadows have six
faces. Increasing the number of overlapping shadow casters can dominate cost
even with only one light. Bevy's cluster assignment is reused; there is no second
light grid or per-pixel scan of all emitters. Disabled/suppressed light children
are hidden so they do not consume cluster entries.

The score combines power, distance and authored priority. Existing shadow owners
receive hysteresis. A retiring light keeps its shadow during fade-out, and a new
owner waits for a free slot. Quality reductions can retire excess maps immediately
to respect the new limit. `EmissionShadows::Required` is the default: lights that
cannot obtain a shadow slot are suppressed, never silently made unshadowed.
`EmissionShadows::None` is only suitable for deliberately short-range accents in
open space. An offscreen source remains eligible if its influence reaches the
visible world. Glowing surfaces remain visible when direct illumination is budgeted
out. `LightingStats` exposes candidates, admitted, shadowed and suppressed counts.

Torches retain their animated flame and use one moving light following its
luminous centroid. Den lamps and the short-range orc-room accents use the same emitter component. The old broad unshadowed
den fill and separate nearest-lamp policy are removed.

## Spatial environment light

`lighting_environment.rs` supplies one streamed native irradiance volume. Its
64 x 1 x 64 ambient cubes occupy 192 KiB of RGBA16F texels. RGB stores directional
sky/ground radiance; alpha stores a conservative terrain ceiling in world metres.
A single background job rebuilds after a 16 m camera-cell or range change.
Stale results are discarded; one image handle is reused and memory does not grow
with travel. The volume includes a margin around the render radius and is shared
by world and hand views.

A version-checked patch at Bevy's shared PBR/irradiance library boundary applies
the ceiling visibility to sunlight and sky irradiance for native scene objects.
Heightfield terrain and its generated surface rocks are explicitly exterior
receivers: they bypass the approximate underground classifier, while retaining
normal sunlight and local-light shadow maps. Cave geometry keeps spatial sky
exclusion. A coarse, interpolated height field must never decide whether the
terrain that generated it is underground. Global ambient is only a small floor.
The existing radial-shadow library integration remains independent.

This field is **spatial ambient lighting**, not a dynamic multi-bounce GI solver.
Its coarse height envelope is only used for native scene objects; narrow cave mouths may retain some sky near the
surface. Detailed tree/rock/wall occlusion comes from shadow maps. Room-scale
visibility-aware bounce probes, true area emitters, PCSS, SSGI and ray-traced GI
remain separate extensions, not hidden assumptions in this baseline.

## Ownership and maintenance

- `rendering/lighting.rs`: world sun, emitter lifecycle, material variants, budgets.
- `rendering/lighting_environment.rs`: bounded environment field and checked
  Bevy shader integration.
- `sdf_scene.wgsl`: procedural surface properties feeding shared PBR; preserves
  snow, geology, contact detail, cutouts and conservative depth.
- `player/hand.rs`: matching world poses for the hand rig/camera, independent depth.
- `graphics.rs`: final composition, bloom, existing graphics controls.

Keep shader integration pinned to the engine version. Before upgrading Bevy,
verify the PBR accumulation anchors and irradiance packing. A native runtime smoke
test is necessary: Rust compilation alone does not validate WGSL variants.
Keep static material caches free of illumination so a moving emitter never
rebuilds terrain caches. Avoid unbounded lights, per-blade lights, or unrestricted
shadow maps in content-specific systems.

## Validation

`HITHER_LIGHTING_TEST=1` adds a generic glowing test item and a wall in front of
the starting camera. `dark` removes sun/ambient/sky for direct-light isolation;
numeric values from 1 through 512 add that many candidate emitters for budget
stress. These fixtures never appear in normal play.

Use `bash tools/smoke_lighting.sh item|dark|stress|forest|den` for isolated captures.
Run rendering tests plus the torch and hand tests. Test MSAA and reduced-resolution
world targets because cluster coordinates must follow the world target, not UI
resolution. Benchmark only without another game/compiler process, using identical
seeds, camera paths, resolution, assets and compiler settings. Report frame-time
percentiles and shadow overlap; do not infer a universal FPS from the light caps.

Measured frame times, test outcomes and final captures are recorded in
[LIGHTING_VALIDATION.md](LIGHTING_VALIDATION.md).
