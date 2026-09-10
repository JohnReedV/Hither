# Graphics and HUD controls

Escape → Options → Graphics opens the new panel. Coordinates is beside Show FPS
in the main Options menu. Click a quality
row to cycle it; use the resolution arrows in either direction. Escape or Back
returns to the pause menu. Existing FPS, sensitivity and display-mode controls
remain in Options.

- Coordinates: tiny X/Y/Z readout below FPS, in world meters. Y is foot height;
  the castle is at the world origin. Works in spectator mode too.
- Render resolution: Native, 320×180, 426×240, 640×360, 854×480, 960×540,
  1280×720, 1366×768, 1600×900, 1920×1080, 2560×1440, 3200×1800, 3840×2160.
  These are **3D rendering** presets, independent of window/borderless/fullscreen
  mode. The scene is linearly scaled to the display; HUD/chat/menu stay at display
  resolution. Non-16:9 windows fit inside the preset bounds, preserving aspect
  ratio rather than stretching the world. Native follows window size and DPI.
  A preset above display resolution provides supersampling, at additional cost.
- Anti-aliasing: Off, FXAA (default), MSAA 2/4/8 samples. MSAA costs substantially
  more and does not anti-alias procedural shading inside polygons. The SDF depth
  shortcut is disabled with MSAA to prevent partially covered edge holes.
- Render distance: 16–1024 meters, default 48. Drag the slider or click the number
  to type an exact whole-meter value; Enter or clicking away commits it, and
  Escape cancels uncommitted typing. Values outside the range are clamped.
  This is a shared radial distance from the camera for terrain, castle, grass,
  all trees and fruit. A depth-writing sky boundary clips every mesh material
  to the same sphere as the analytic ground. Screen corners get no extra reach,
  and rotating in place does not rotate a planar cutoff through the world.
  Streamers allow for the exterior camera offset and tree canopy size.
  Higher distances increase geometry, loading time and memory.
  Changes apply while stationary as well as while moving, and persist on restart.
- Detail distance: 16–1024 meters, default 24, with a separate slider and numeric
  input. Controls grass blade coverage and detailed apple, orange, boreal and oak
  foliage. Grass uses radial distance with an irregular fade in only the final
  two meters, not broad density thinning. Matching both settings aligns their
  outer limits. Distant grass retains every tuft site using
  a simpler, area-preserving triangular blade; textured ground remains beyond it.
  Trees keep their simpler visible canopy beyond this distance, using the same
  materials. Tree thresholds allow for canopy radius and forest LOD hysteresis.
  Effective detail distance is capped by render distance without discarding the
  saved preference. Decorative understory plants remain within render distance.
- Shadow distance: 16–1024 meters, default 24, independent of detail distance.
  The effective sun-shadow range is capped by render distance. Existing settings
  migrate to their former detail distance, capped at 128 meters; the slider and
  numeric field can override it. Shadow quality still controls map resolution.
- Texture quality: High (original), Medium (half dimensions), Low (quarter
  dimensions). Changes uncompressed RGBA8 world-material textures, including
  albedo, normal and roughness maps, not UI/font textures. Mipmapped images select
  an existing mip level and retain its complete tail; compressed textures are
  left intact. Procedural trees generate color-correct and normalized-normal mip
  chains. Originals remain on the CPU
  for lossless restoration; lower settings reduce GPU texture memory, not the
  retained CPU originals. Shader-generated ground detail is independent of this.
- Shadows: Low 512, Medium 1024 (default), High 2048 pixels per cascade. Retains
  the two-cascade, simplified shadow-caster approach tested in forest profiling.

  Item lighting now shares one global local-shadow budget: Low/Medium/High admit
  1/2/4 shadowed point lights, with 512/1024/1024-pixel cube faces. See
  [lighting architecture and authoring](LIGHTING.md) for emitter controls,
  transition behavior and the spatial sky-light field. Terrain and rocks use
  the same PBR lighting as other meshes; the obsolete castle trace is inactive.
- Grass blades: On (default) or Off. Off stops new grass meshing and removes
  blade geometry; textured grass ground and snow remain. On rebuilds nearby
  chunks asynchronously. This does not affect tree density or world generation.

All controls apply live and persist in the existing settings JSON after a short
debounce and on normal Quit. Older settings files inherit the tested quality
defaults. Coordinates default off; FPS remains on by default.

The extra display-resolution UI camera and scene-image presentation allow low
resolutions without blurry menus. Earlier forest benchmark results predate the
shared render-distance system (especially extended grass coverage), and are not
performance guarantees for the new distance settings. Increasing distance loads
more geometry; shadow cascades now also end at the configured far plane.

Validation: `cargo test`, strict Clippy, and `bash tools/smoke_graphics.sh`.
The smoke test uses an isolated Xvfb display/config, cycles all AA modes,
changes render resolution live, toggles coordinates/grass, and restores Native.

## Terrain and surface performance

Terrain uses world-aligned 32 m tiles with ordinary frustum and GPU occlusion
culling. Up to eight background jobs cache elevation and normals. A shared
one-metre procedural fallback keeps newly entered tiles visible until a job is
ready, including on teleports. Explicit bounds are protected from automatic
recomputation while the fallback is displaced in the shader. Loaded tiles and outstanding jobs are bounded.
Nearby tiles keep the collision lattice; distant tiles try 2/4/8 m spacing but
fall back to finer spacing whenever the error against the one-metre lattice
exceeds 4 cm. Boundary skirts cover neighboring LOD seams. Camera movement
within a tile does not rebuild geometry. Distance-band crossings reuse meshes
that already meet the requested detail; movement only refines cached tiles,
while explicit quality changes can coarsen them.

Footprints use a one-metre spatial hash with exact radius checks. Only buckets
intersecting a fragment's footprint neighborhood are searched, and snow-free
surfaces skip the search. Procedural detail bands skip noise when their filter
weight is zero. The fullscreen castle trace is bounded by prepass scene depth;
with MSAA it uses the farthest sample so silhouette coverage stays conservative.
Grass maintains a streaming queue between cell/setting changes and uses LOD
hysteresis rather than rebuilding equivalent coarse meshes. Coarse grass consumes
its original random sequence but skips terrain evaluation for discarded blades.
Forest streaming retains its queue and only adds entering strips. Forest LOD is
selected per tree in spatially relevant chunks, and rock LOD stops while stationary.
Footprint bins change only when tracks are added/removed; exposed cave segments
and current-window candidates are cached. Shared per-instance rock contact data
removes repeated procedural vertex shading once bounded background baking finishes.

The reproducible before/after harness is `tools/benchmark_render.py`; generated
logs and immutable code/assets/binaries are kept in `tools/build/render-audit`.
