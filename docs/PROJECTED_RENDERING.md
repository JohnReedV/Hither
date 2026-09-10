# Projected rendering and preparation budgets

## Geometry

Terrain's error allowance is one projected pixel, quantized to powers of two.
The vertical focal length comes from the selected world render resolution and
camera field of view. The nearby detail window retains the exact one-metre
terrain lattice. Outside it, meshing checks the height residual against the
projected allowance, and retained tiles both refine and coarsen. Skirts cover
adjacent error bands. Collision, terrain support and procedural placement do
not read visual LOD meshes.

Broadleaf foliage has two additional shared crown representations. Spatial clusters
aggregate leaves into colored volumes. Conifers retain authored branch and snow
surfaces in their distant meshes: solid crown volumes changed their snow coverage
and porosity. Reduced conifer needles and oak leaves preserve source shading normals. Small source
meshes use fewer clusters so simplification cannot increase their triangle count.
Camera selection uses projected crown extent, hysteresis and the detail-distance
limit. Grass also aggregates unresolved tuft sites instead of preserving every
site at arbitrary distance. Original detailed art remains available nearby. Cluster materials retain the
linear-light average of the source foliage texture, avoiding a brightness jump.
Settled plants skip selection work until the camera moves half a metre, settings
change, an authored mesh/transform changes, or a shared representation arrives.
New plants are always initialized, including while the camera is stationary.

## Static surface cache

A 48-page GPU clipmap stores packed linear color, world normals and snow masks
for terrain. Each page has a complete 256-to-1 mip chain; three spatial levels
cover 32, 128 and 512 metres per page. The surface allocation is approximately 32 MiB, plus 8 MiB for fine climate fields.
Pages are baked using the same static WGSL functions as live procedural shading,
then downsampled. Lookup selects and blends mips from the fragment footprint.
Finished surface shading blends in when its texels are resolved by the pixel
footprint, independently of geometry detail distance. Nearby fragments instead
reuse FP16 snow climate, mountain amount, woodland density and snowline variation
from the 12.5 cm pages. Altitude/slope retention, small snow patches, fine shading,
and player tracks stay procedural. A shared snow calculation supplies the
surface, powder normal and tracks. Cache lookup falls back near page edges rather
than clamping the climate interpolation across an unavailable neighbor. Missing
or coarse pages retain procedural coverage. Clearing-mask changes invalidate pages.

Static rock color, normals and snow coverage are baked when contact data is
installed, including both authored mesh LODs, into a separate bounded 32 MiB
allocation indexed by the existing contact allocation. Distant fragments reuse
interpolated properties; nearby fragments keep fine procedural detail. This is
a vertex surface cache rather than a per-rock texture atlas. Terrain mips also
serve distant rock/ground contact shading.

Lighting, shadow sampling, footprints and player contact effects remain dynamic.
Neither surface cache modifies collision. Terrain submits at most one page bake
per frame; rock baking follows bounded contact admission. These are work limits,
not hard GPU-time guarantees.

## Residency and preparation

Gameplay residency remains independent of render visibility. A common view-cone
and 128/32-metre region ranking orders visual work ahead of nearby hidden work.
Terrain additionally checks its actual camera frustum. Content layers no longer
have an explicit sequential schedule chain. Worker/result slots remain shared,
with rotating reservations for waiting layers.

Preparation has separate controls:

- Twelve outstanding background jobs, including completed uninstalled results.
- A 32 ms completed-worker-time threshold defers further generation admission.
- A 1.5 ms admission target combining scope time with measured downstream
  estimates for asset extraction, mesh allocation/preparation and terrain commands.
- A 4 MiB upload-payload budget for meshes and contact data.
- Existing weighted admissions and local batch limits retain fairness.

These limits are cooperative. A running job cannot be preempted. One atomic
installation can exceed the time target; the first atomic upload can progress
when its downstream estimate alone exceeds that target. Payloads still obey the
4 MiB limit. Render-world feedback includes CPU staging/driver submission, not
GPU completion time. The extraction estimate conservatively includes other asset
classes sharing the public extraction set. Terrain commands are timed when applied,
not merely when queued; other adapters retain the entity-count cap and estimate.

A rotating waiting layer reserves its first actual installation, in addition to
weighted units; unused reservations expire so abandoned work cannot block others.
Terrain retirement scans only after a window change, at most 128 queued cells and spends at most 0.5 ms on
bookkeeping and mesh removal per frame. Hierarchy retirement separately bounds
actual leaf-first despawning by count and a 0.5 ms target. Each operation remains
atomic. Return teleports revalidate queued cells against the current window.

Visible fallback replacement has priority. An admitted CPU terrain worker gets
20 ms before a duplicate GPU sample bake is eligible. Ready CPU results cancel
pending GPU requests immediately; slow workers still permit GPU fallback baking.
CPU and GPU samples are not transferred between representations. Immediate
procedural fallbacks preserve coverage throughout preparation.

## Shadows and composition

Shadow range has its own saved setting, capped by render distance. Shadow quality
controls map resolution. Directional shadow sampling uses radial camera distance
for every material, blends cascade transitions, and fades over the final 15% of
the range. Both cascade projections cover the near volume so radial selection
remains valid at oblique camera angles. Near and far cascades retain separate texel
budgets. Tree shadow proxies select their representation from light-space texel
coverage independently of camera foliage LOD. Terrain and rocks cast and receive
mesh shadows; the custom shader no longer uses a constant unshadowed factor.

Static cascade reuse is not enabled: camera movement changes the cascade
projection and vegetation can change. Reusing those maps without explicit
projection/caster invalidation would introduce stale shadows.

World and hands write linear HDR color into the same intermediate target. Each
pass clears depth independently. A final 2D compositor applies the single tone map and
optional FXAA, then writes the image once for native-resolution UI presentation.
MSAA settings apply consistently across all composing passes. Resolution changes
refresh every pass's projection in the same frame; Bevy 0.19 does not directly
observe a changed RenderTarget in camera_system.

## Measurement

Performance must be measured with matching source, art, settings, routes and
shipping compiler flags. Diagnostic builds and runs overlapping compilation are
functional checks only. See the benchmark artifacts under
`tools/build/render-upgrade/` for the frozen comparison sources and logs.
