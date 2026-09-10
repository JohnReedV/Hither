# Hither SDF Field Lab

A procedural world built with Rust, Bevy, and WGSL. The spawn castle and courtyard apple tree have been removed; wild trees and fruit remain throughout the world. Lighting uses shared PBR shading, shadowed item emission, directional sky irradiance, ambient occlusion, restrained bloom, and HDR tone mapping.

The lighting rework unifies terrain, rocks, vegetation and characters under Bevy
PBR, with reusable item emitters, a global local-shadow budget, spatial sky
irradiance, world-lit first-person hands and restrained HDR bloom. See
[lighting architecture and authoring](docs/LIGHTING.md) for the API, limits and
reproducible validation scenes.

## Run

```bash
cargo run
```

Both `cargo run` and `cargo run --release` prioritize runtime performance with
level-3 optimization, full link-time optimization (LTO), one codegen unit, and
incremental compilation disabled. Builds, especially the first Bevy build and
final link, can take substantially longer. Both profiles omit debug information
and disable debug assertions and integer overflow checks; `cargo test` keeps
those checks enabled. Actual performance depends on the workload and hardware.

## Project structure

See [the architecture guide](docs/ARCHITECTURE.md) for module ownership, scheduling
contracts and validation commands, and [the tools guide](tools/README.md) for
authoring, smoke tests and profiling. `src/main.rs` is only the executable entry
point; `src/app/` composes the player, world, rendering, UI and diagnostics modules.

## Controls

- `W`, `A`, `S`, `D`: move on the ground plane
- `Space`: jump; release early for a shorter jump
- `E`: pick an apple directly under the center crosshair, within 3.5 meters of the player; leaves, branches and walls block picking
- `T`: open chat; `Enter` sends locally, `Esc` cancels without opening Pause. Backspace/Delete, arrows, Home and End edit the message.
- `/`: open chat with `/` already entered, ready for a command.
- `/teleport <x> <y> <z>` (alias `/tp <x> <y> <z>`): move the player or spectator to absolute world coordinates (Y is eye height). Accepts negative and decimal numbers, for example `/teleport 100 25 -50`. Clears jump/fall motion; normal gravity and collision handling resume in player mode.
- `Up` / `Down` while typing: recall older/newer sent messages and commands. Down past the newest restores your unfinished draft. The last 100 submissions are kept for the current session; recalled text can be edited before Enter sends it.
- Mouse wheel over the open chat box: scroll the message log up/down, including wrapped messages. Closing chat returns the log to the newest messages.
- Command suggestions appear above the chat input as you type (`/s` shows “Spectate”). `Tab` fills the bottom suggestion; repeated Tab cycles matching commands or `/locate`, `/tp`, and `/teleport` targets. Type a space after any of these location commands to see target suggestions. Enter executes. Suggestions, completion and execution share one command registry.
- `/tp <target>` (also `/teleport <target>`): teleport to the nearest location using any `/locate` target or alias, for example `/tp goblin_den`. Searches run in the background from your position when submitted and place your feet at the destination. A coordinate teleport supersedes a pending search.
- `/locate <target>`: report the nearest target's X/Y/Z coordinates and horizontal distance in chat. Targets: `goblin_den`, `orc_den`, `plains`, `temperate_forest` (`forest`), `tundra`, `boreal_forest` (`boreal`), and `mountains` (`mountain`). Searches use your position when submitted and the current world seed, including unloaded regions. Dens return their center (goblins) or surface entrance (orcs), searching up to 32,768 m. Biomes return the nearest matching sample on a 2 m grid within 4,096 m, or your current X/Z if already there; transitions use the dominant biome. Searches run in the background, one at a time; missing or invalid targets show usage.
- `/spectate`: toggle spectator mode. `WASD` flies relative to the camera at 20 m/s,
  `Space` rises, `Left Shift` descends, and holding left mouse doubles speed to
  40 m/s. Flight ignores walls, trees and gravity; diagonal movement is normalized.
- `F3`: cycle First Person → Third Person (rear) → Second Person (front) → First Person
- Move the mouse: look around with the pointer locked to the camera
- `Esc`: pause or close the current menu; from Options it returns to Pause first
- `Resume`: return to the world and recapture the pointer

The pause menu's Options screen provides sliders and exact text entry for the 1–1000 FPS cap and 0.10–5.00 mouse-sensitivity scale. Press `Enter` to commit typed input. You can also select Windowed, Borderless, or Fullscreen display mode and toggle a tiny live FPS counter in the top-right corner.

All options are saved automatically to `~/.config/hither-sdf/settings.json` and restored the next time the game starts.

Coordinates can be toggled beside Show FPS. The **Graphics** submenu adds render
resolutions from 320×180 through 4K (plus Native), Off/FXAA/MSAA anti-aliasing,
texture and shadow quality, and a grass-blade toggle. Menus remain sharp at low
render resolutions. See [graphics controls](docs/GRAPHICS.md) for details.

Render Distance in Graphics uses a slider and exact meter input (16–1024 m,
default 48 m). Trees, fruit, castle, terrain and grass share the same radial
camera cutoff; screen edges get no extra reach. Distant grass uses reduced
geometry, with an irregular transition in the final two meters of detail range.

Chat is local-only for now: submitted messages appear as `<You>` in the bottom-left history for 12 seconds and reappear when chat opens. Messages are limited to 256 characters, with the last 100 kept in memory (the newest six shown). Nothing is sent over a network or saved to disk. `ChatSubmitted` is the outgoing-message boundary for future multiplayer support. Chat releases the mouse and blocks movement, jumping, apple picking and view switching while typing; the world and gravity continue. Sending or cancelling recaptures the mouse. The input and history use bundled Almendra Regular, a readable manuscript-style face in warm ivory, without an instructional header; see [font attribution and license](assets/fonts/README.md).

Slash commands are handled locally, not sent as chat messages. Spectator mode uses
an invisible, first-person free camera, with no apple harvesting, footprints or
grass trampling. Opening chat or pausing stops flight; the world still runs while
chatting. `/spectate` again restores normal play at the current location, moving
to a nearby clear spot if inside an obstacle, or falling normally if above ground.
Your previous F3 view is restored. Spectator mode is not saved between launches.

The tree checks for growth once per second whenever it holds fewer than five apples. Each check has a 1% chance to regrow one apple, hanging below a random point on an actual fruiting branch. Picking removes it and records the harvest internally, leaving room for regrowth. The top-left debug overlay and its counters are no longer displayed; the crosshair, optional FPS counter, chat and pause menu remain. Pausing stops growth. One success takes 100 seconds on average, but there is no guaranteed deadline; a few minutes without a new apple is normal.

Each launch grows a different tree: curved main limbs split into secondary and occasional tertiary branches, with randomized leafy shoots, trunk shape and roots. Raised bark ridges and thousands of individually modeled curved/serrated leaves provide close-up detail. Apple spawns always follow that tree’s actual branch geometry. Apples have lobed shoulders, stem/bottom dimples, curved stalks, a calyx and a leaf. Procedural base-color, normal and roughness maps supply bark fissures, leaf veins and mottled red/gold apple skin with fine speckles. Meshes/materials are built once and shared; fruiting stalks rebuild only when their apple slot changes. Picking uses accelerated ray/triangle tests against the same fruit and tree geometry shown on screen, including gaps between leaves. See [orchard implementation notes](docs/ORCHARD.md).

Third- and second-person views render a real skinned human mesh: approximately 108,000 triangles, 53 joints, and separate skin, hair, eyes, clothing and footwear materials. The outfit is an open navy/red plaid jacket with beige fleece trim (no hood), dark gray crewneck T-shirt, classic blue denim jeans and dark hiking boots. The face is an artistic approximation fitted to portrait landmarks, with portrait-derived facial detail baked into a standard 4096² skin texture. The ears are compact, the forehead hairline is fitted 3D geometry rather than detached painted strands, and the undershirt is tucked beneath the jacket surface and into the waistband. The rear jacket has clearance over the denim hips. Ten skeletal clips cover idle, walking, strafing, jump/fall/landing and left/right turn-in-place steps. Walks have planted support feet, weight transfer and opposing arm swing; jumping has gathering, extension and impact recovery. Looking around while stationary makes the body follow through stepping turns, not instant rotation. Animations blend with movement and freeze when paused. The follow camera pulls inward around obstacles; the avatar shares the SDF world's depth buffer.

## Trees and world generation

Five biomes use independent seeded climate, woodland and mountain noise fields, forming
irregular regions rather than fixed bands:

- **Plains:** nearly continuous grass, with rare giant solitary oaks. Each suitable
  288-m region has a 25% chance of retaining its seeded landmark, with at least 96 m between oak trunks.
  Regions without suitable open, snow-free terrain remain treeless; oaks are
  visible from 420 m plus their crown allowance so they can be discovered across open plains.
  Oaks range from their original scale to five times larger, with eight seeded
  branch-topology variants and varied crowns, forks and spreading arms.
- **Temperate forest:** close-growing orange stands, mature overlapping crowns,
  saplings and grassy clearings. No oaks.
- **Tundra:** open, treeless snow with wind-crust texture and persistent tracks.
- **Boreal forest:** denser, irregular snow-laden conifers.
- **Mountains:** independent alpine ranges with regional profiles ranging from
  broader eroded massifs to sharper ridges. Uneven crowns, subsidiary spurs and
  gullies break up the skyline. Mostly continuous weathered stone gives way to
  occasional fractured outcrops, with broken mineral exposures rather than
  uniform contour stripes. Thirty procedural rock models span granite boulders,
  stacked ledges, broken crags, shale talus, weathered tors and blade outcrops.
  Seeded colonies follow slope and elevation, leaving stretches of open ground;
  alpine cushions, tussocks, lichen and slope-dependent snow add smaller detail.
  Five variants per family vary the appearance. Rocks and terrain share the same
  world-space geology, lighting, snow and tone mapping. Irregular dirt/snow contact
  bands and smaller fallen fragments blend the bases into the surrounding ground.
  Nearby rocks use detailed meshes; distant outcrops retain simpler silhouettes.
  Meshes are shared across instances, chunks stream with bounded work, and small
  debris only loads nearby. Rock collision, camera obstruction and ledge support
  query the same detailed triangles independently of render distance or streaming.
  Foothills blend into neighboring biomes and preserve castle and cave clearings.

The castle stays at the coordinate origin, but its biome is not forced: each seed
can start it in any of the four biomes or a transition. Woodland density fades at
forest edges. Climate borders blend over a narrow interval (half the original
blend width). The border progresses through dormant straw and frosted tips into
coherent patches of snow. Grass gets sparser/shorter and is buried using the same
coverage mask as the ground shader; no green blades remain on white ground.
Orange trees and conifers each use eight world-seeded detailed models. Oaks use
four, with broad crooked scaffold limbs, smaller forks, buttress roots and folded
lobed leaves. Oaks occur only as rare giant plains landmarks. Forest sites have
strongly jittered positions, minimum trunk spacing and coherent 28-m clearing
patterns, with grass/litter following the same stand mask. Most trees are mature,
interspersed with smaller young trees. Placement, rotation and scale are deterministic.
Streaming considers at most 841 conifer cells, 25 oak regions and 49 citrus chunks;
all geometry is reused. Distant oak/citrus leaves keep every blade with 20x/4x
fewer triangles; distant conifers simplify shoots, wood and snow surfaces.
Castle/gate clearance and trunk/camera
collision apply in every biome.

Orange morphology varies trunk bend/fork height, scaffold counts, secondary branch
counts, crown lobes, width/height and asymmetry. Conifers vary height, skirt height,
branch tiers/counts, droop, lateral shoots, crown asymmetry and snow loading.
The bounded morphology banks are generated concurrently once per world, with shared
meshes thereafter. Fruit anchors and clearance checks follow each generated citrus
model; changing the world seed produces a new set of shapes.

Snow has distance-filtered powder grain, wind-crust color and bump detail.
Walking leaves mirrored left/right hiking-boot impressions with rounded toes,
medial arches, heels, chevron lugs and slight toe-out. Their pressure walls,
compacted granular interiors, partially snow-filled tread and broken powder rims
vary in world space instead of repeating an identical stamp. Stationary camera turns
also stamp the left/right foot at the actual turn-animation contact phases, including
in first person; small aim corrections without a step leave no mark.
Jumping stops the trail and
landing stamps both feet. Tracks last 30 seconds of unpaused gameplay, fading
during the last three seconds. The 512-entry footprint buffer is bounded, and
fully snowy grass cells never submit empty meshes to the renderer.
These are visual impressions, not changes to the collision surface.

The infinite ground is natural grass instead of checkerboard tiles: softly varied green/olive ground cover, fine fibres and soil, a worn entrance path, and exposed tree roots. Nearby curved 3D blades have varied heights, lean and occasional dry tips, with subtle GPU wind and bending around the player's feet. They receive the same PBR lighting and tree shadows as the character and orchard. Wind pauses with the game. Both forest biomes have seeded rolling terrain, with occasional larger, elongated hills inspired by the Appalachians. Trees, understory, grass, player support and camera clearance follow the same elevation. Relief fades into open biomes, and level clearings preserve the castle and cave entrances.

Grass streams in deterministic 4-meter chunks covering the radial detail range, with at most four mesh jobs in flight. One shared material drives all blades; no per-blade entities or CPU animation. Far meshes simplify each tuft to a broader triangular blade without dropping tuft locations. Blades fade irregularly over the last two meters; matching detail and render distance aligns their outer limits. Old meshes are released and CPU vertex copies are discarded after upload. Higher distances require more loading time and memory. No external grass assets or startup texture generation are needed. For isolated distance comparisons, use `HITHER_WORLD_SEED=721 HITHER_PREVIEW_BIOME=plains HITHER_GRASS_DISTANCE_TEST=1 bash tools/smoke_graphics.sh`.

The courtyard apple tree follows mature orchard-tree references: a short trunk, broad low crown, bowed scaffold limbs and smaller fruiting branches. Its shape varies each launch, and apples attach beneath its actual branches. The 1% growth check is for the **entire apple tree**, not each fruiting spot.

Orange trees have a separate citrus model: a dense rounded evergreen crown, low branching, broad smooth-edged dark glossy leaves, slimmer gray-brown trunks and round pitted fruit. Temperate forests use jittered 3-m candidate cells with 2-m minimum trunk spacing, dense stands and coherent clearings instead of independent sparse per-metre rolls. They never populate open plains. Nearby world sections stream in and out; revisiting restores the same trees. Orange fruit is decorative; apple harvesting and regrowth are unchanged.

Dense forests use shared near/far meshes and bounded, nearest-first streaming.
Close trees keep detailed foliage and wood; distant citrus keeps every leaf blade
with a simplified folded mesh and the same material, normal map and roughness map
at every distance. Concealed fine twigs are omitted. Fruit bodies and stalks
are batched per tree, not separate entities per orange. Detail refreshes during
movement, including spectator flight, with hysteresis to prevent rapid switching.
Forest density, draw distance and procedural tree placement are unchanged.
Forests use shared lightweight shadow-only meshes, two shadow cascades and a
depth prepass to avoid shading hidden foliage. Grass meshes build in a bounded
background queue; FXAA keeps edges smooth without 4x multisampling.
See [forest optimization and benchmark instructions](docs/FOREST_PERFORMANCE.md).

For reproducible testing, set `HITHER_WORLD_SEED` for biomes, vegetation placement,
grass variation and the courtyard tree; `HITHER_TREE_SEED` optionally overrides
the courtyard tree alone. Seed 2 starts the castle in grassland; seed 721 starts
it in tundra. The seed, castle biome and snow coverage are printed at startup.
Without an override the world changes on the next launch; this is not yet a
saved-world system. See [reference photos, generation details and tests](docs/ORCHARD.md).

For visual QA only, `HITHER_PREVIEW_BIOME=plains|forest|tundra|boreal|mountains|oak` starts at
a representative generated location (choose one value). Normal launches still
start inside the castle. For example:

```sh
HITHER_WORLD_SEED=721 HITHER_PREVIEW_BIOME=oak cargo run --release
```

Native winter/footprint smoke test (isolated window and settings):
`HITHER_WORLD_SEED=721 HITHER_SMOKE_WILD=1 HITHER_SMOKE_GRASS=1 HITHER_SMOKE_SNOW=1 bash tools/smoke_orchard.sh`.
It captures the trail and the same view after 31 seconds, then continues streaming
and checks the runtime log for renderer errors.

## Character skins

The standard is **Hither Avatar v1**, a self-contained `<name>.hither-avatar.glb` file containing the model, embedded PBR textures, skeleton, eight named animations and versioned metadata. The bundled character uses this exact format. Custom avatars may have their own topology and UV layout; cosmetic changes never alter the player's collision shape or jump physics.

Put a custom file under `assets/avatars/`, then select it at startup:

```bash
cargo run --release -- --validate-avatar assets/avatars/mine.hither-avatar.glb
HITHER_AVATAR=avatars/mine.hither-avatar.glb cargo run --release
```

Invalid custom files fall back to the bundled avatar. There is no skin picker UI yet. See the [file standard and Blender workflow](docs/HITHER_AVATAR_V1.md), the packed [editable template](assets/avatars/source/default.blend), and [asset credits](docs/AVATAR_ASSETS.md). The old six-by-six PNG atlas is retired.

## Extending the scene

Add SDF functions and combine them in `map_scene` in `assets/shaders/sdf_scene.wgsl`. Each object returns a distance and material ID; add its color in `material_color`.

To explore the alpine biome, launch with `HITHER_WORLD_SEED=721 HITHER_PREVIEW_BIOME=mountains cargo run --release`. Seed `3` provides another mountain preview. Set Render Distance to 512 m in Graphics for mountain panoramas; `/spectate` lets you fly through the valleys.

Snow melts into coherent patches across the foothill apron, with grass burial and footprints using the same surface coverage as the terrain. Mountain interiors accumulate alpine snow gradually above 200 m; exposed slopes retain less snow. Lowland snow can extend into the transition below that altitude, avoiding a hard biome boundary or a bare strip before the grass returns.

Walking and standing are limited to terrain slopes of 45 degrees or less. Steeper mountain faces block uphill walking and cause the player to slide downhill on contact. They cannot restore jump eligibility, so repeated jumps cannot climb a face; jumps that clear it can still land on gentler terrain.

Hidden native mountain QA: `bash tools/smoke_mountains.sh` uses an isolated virtual display and temporary settings, checks renderer errors, and saves `tools/build/mountain-valley.png` without touching the desktop.

Alpine ground keeps layered meadow color and tussock shading beyond the grass-blade
detail radius. Warped simplex fields break up grass, dry patches and slope-driven
rock exposure; altitude gradually thins the meadow instead of turning whole
valley floors into soil. Stone uses triplanar mineral relief and shares its
material with outcrops. Surface detail filters by projected pixel size, keeping
resolved texture at long range and converging to average cover as it shrinks.
This changes surface shading without adding terrain or vegetation geometry.

Vegetation stays upright on slopes. Trees and plants sample their root footprint against the rendered terrain, burying their bases on the downhill side; cliff-edge footprints are rejected in every biome. Grass feet also follow the surface at both detail levels.

## Goblin mobs

Small, skinned Mireling goblins live in mountain dens. Their reference-inspired anatomical model, dedicated animations, and one-third player height are documented in [GOBLINS.md](docs/GOBLINS.md). Use `HITHER_GOBLIN_PREVIEW=1 cargo run --release` for a close spectator preview.
