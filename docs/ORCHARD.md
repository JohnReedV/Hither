# Detailed orchard

`src/world/orchard/mod.rs` owns botanical geometry and materials; `src/world/orchard/fruit.rs` owns fruit growth and picking. Shared mesh and ray-query primitives live in `src/rendering/geometry.rs`. `src/world/orchard/apple_tree.rs` generates the courtyard apple tree; `src/world/orchard/orange_tree.rs` generates distinct evergreen citrus; `src/world/orchard/wild.rs` streams those trees through the world. The apple tree uses a fresh seed each launch, with a single cached model supplying rendering, attachments, camera bounds and picking. Surface maps remain deterministic. No reference photos are used as game textures or included in the runtime. The castle and floor remain SDFs; wood, individual leaves and fruit share their camera/depth buffer.

For reproducible bug reports, the seed is logged at startup. Developers can replay a tree with `HITHER_TREE_SEED=7 cargo run --release`. Without that optional environment variable, the seed comes from OS-backed randomized hashing state, independently of the fruit-growth RNG. Tests use fixed seeds.

## Geometry and surfaces

Full-tree photographs inspected for this revision:

- [Nebraska Extension: mature fruiting apple tree](https://lancaster.unl.edu/pruning-established-fruit-trees/) — short visible trunk, spreading fruiting limbs, dense irregular crown and pendulous outer shoots.
- [Northwoods Tree Farm: common/wild apple](https://northwoodstreefarmllc.com/product/apple-trees/) — broad, rounded mature crown with foliage extending low around the trunk.

These are visual references, not imported assets. The courtyard version is scaled to the small castle and keeps the existing five-apple gameplay cap, so it does not copy the reference photographs' heavy fruit load.

Citrus reference: [NC State Extension, Citrus x sinensis](https://plants.ces.ncsu.edu/plants/citrus-x-sinensis/), including Peter Struwwel's tree-form photograph. The dense evergreen crown, glossy ovate blades and slender gray trunk inform the orange model; no photo is copied into the game.

- Apple: short crooked trunk, seven broad scaffold limbs that rise then bow outward, four smaller upper limbs, secondary branches and drooping fruiting twigs. The crown is wider than tall, with 6,600 individual veined leaf blades (9.5–15 cm) and no pointed center pole. Heights, reaches, bends, leaf tilt and tint vary by seed.
- Orange: a rounded evergreen crown with low branching, slimmer gray-brown trunks and roughly 5,400–7,560 broad, smooth-edged leaves. Citrus has its own darker, glossier leaf material and finer bark material. Eight world-seeded mesh variants randomize trunk bend, fork height, scaffold and secondary branch counts, crown height/width, lobes and asymmetry; placement also varies orientation and scale. Each orange tree chooses two to four fruit from 64 collision-checked terminal-twig attachments, seeded by its world cell; wild apple trees carry up to five apples. Fruit varies from 8–11.6 cm diameter before tree scaling, with short 1.8–4.5 cm stalks and small green calyces. Positions follow real twigs throughout the crown rather than a forced outer ellipsoid, so leaves can naturally hide some fruit. Body/leaf intersections and fruit overlaps are rejected; each visible stalk reaches its actual twig. Different trees have different fruit layouts, and revisiting restores the same layout with harvested fruit still removed. Fruit, stalk and calyx meshes are shared across trees; no unused fruit stalks are baked into the wood. Wild apples and oranges are pickable with E. Picking uses the rendered fruit shapes and each tree’s scale and rotation, checks reach from the player, and rejects fruit hidden by tree triangles, terrain or castle geometry. Harvested slots remain empty when chunks unload and reload during the session. A per-cell harvest mask selects cached body/stalk/calyx meshes without modifying shared unharvested trees or creating individual fruit entities. Wild fruit does not currently regrow; the courtyard tree retains its growth timer.
- Branch centerlines use Catmull–Rom interpolation and transported cross-section frames to avoid flipping/twisting the wood at bends. Child branches and leaf sprays attach to the exact sampled curves used by the renderer, not straight approximations between control points. Leaf silhouettes have serrations, curvature and raised midribs.
- Geometric bark ridges and buttress roots; procedural bark, leaf and apple maps provide base color, normals and roughness. Maps are generated once at startup, not every frame.
- One shared 7,680-triangle apple-body mesh with non-spherical shoulders and inset top/bottom dimples. Orientation varies by position; stems, calyx and a small leaf complete the fruit.
- Bevy shadow maps shade mesh surfaces. The SDF floor/castle use a cheaper wood/crown shadow approximation, including a dapple pattern; this approximation is not used for interaction. Foliage is static, not wind animated.

## Growth and attachment

The existing timer still rolls a 1% chance once per elapsed gameplay second for the entire tree while below five apples, not separately for each potential attachment point. Each successful roll can add one apple. It does not roll every frame or guarantee regrowth after 100 seconds. Pausing stops the timer. At 1% per second, 100 seconds is the mean wait for one success; long dry spells are expected.

Spawn candidates interpolate along the exact centerline segments used to render the fruiting shoots, then hang 25–38 cm below them. Candidates need space from existing fruit and surrounding geometry. A conservative sphere around the entire apple is checked against nearby triangles through the BVH, catching diagonal leaf intersections as well as wood. Invalid candidates are retried; there is no forced fallback to a floating/embedded position. Stalk endpoints reconnect to the actual branch directly above the fruit, not whichever canopy blob happens to be nearest. Positions are continuous rather than a few fixed sockets.

The same static triangle geometry feeds both rendering and BVH-accelerated ray picking. The apple body must be under the crosshair, within reach, and not hidden by wood, leaf blades or castle geometry. Conservative follow-camera bounds are generated from the new branch segments and leaf clusters, not the old fixed canopy spheres. Picking can pass through genuine gaps between leaves.

## Wild orange distribution

`src/world/orchard/forest.rs` supplies seeded stand placement for citrus and conifers.
Citrus uses 3-m candidate cells with 92% cell-width jitter and 2-m minimum trunk
spacing; conifers use 3.6-m cells and 2.2-m spacing. Local candidate priority is
independent of exploration order. A coherent 28-m clearing mask produces open
glades inside dense stands, with smaller-scale density variation and thinning at
biome edges. The same mask controls grass/litter on the forest floor. Interior
stands retain most candidate sites, not a sparse global per-square-metre roll.
Trees mix 70% mature (1.25–1.70 scale), 18% intermediate (0.90–1.18), and 12%
young (0.55–0.85) sizes. Citrus avoids snow, plains and a 10-m castle clearance;
conifers require snow and forest. These are generation rules, not growth timers.

`src/world/orchard/oak.rs` adds spreading oaks with buttress roots, curved low scaffold
arms, taller central leaders, secondary forks, terminal shoots and rounded-lobed,
folded leaves. Eight seed-dependent models are shared across all instances. Near
foliage uses 40 triangles per leaf; distant foliage preserves every leaf at two
triangles. Oaks are strictly excluded from forests and snow. The old 1-in-1,400
roll per 144 m² allowed long barren journeys, made worse by an approximately
80-m streaming range. Now each 288×288-m region selects one valid plains site
from 144 stratified, jittered candidates by lowest seeded priority. A region with
at least one suitable candidate gets one oak if it passes an independent seeded
25% retention filter (another 50% reduction from the preceding version); unsuitable regions stay
treeless. Forty-eight-meter border margins guarantee at least 96 m between
trunks, so these remain solitary landmarks, not oak forests. The original
0.95–1.12 base scale is multiplied by a seeded factor from 1 to 5, biased toward
smaller specimens; maximum size means five times the linear dimensions, not volume.
Surviving sites retain their prior positions and yaw. Each morphology varies
7–11 spreading arms, 3–5 upper leaders, 7–11 forks per arm, 7–10 terminal shoots
per fork, branching heights, curved limb bends, droop and crown asymmetry.
All meshes are shared and built once, not regenerated for each world tree.
This replaces the old approximate 0.1% canopy target with a regional
rarity rule; it does not guarantee an oak in every tiny plains patch. Giant oaks
still avoid the climate/forest ecotones and the castle.

Oak landmarks stream to a 420-m radius plus a size-scaled crown allowance using at most 25 regional entries and
shared near/far mesh assets. Detail refreshes every two meters independently
of the large region boundaries, with unchanged mesh handles left untouched.
Near detail scales with the tree, capped at 100 m to bound rendering cost; trunk
collision and camera clearance inherit the tree's scale, including 5× specimens.
Collision and streaming use the same sites; a bounded 128-entry per-thread
cache avoids repeating biome searches during movement. Cache eviction does not
change placement. Tests cover discovery, habitat, spacing and exact replay over
six seeds, negative coordinates, cache eviction, rendering an oak 250 m away,
and near-detail changes within a single region.

Winter conifers leave tundra treeless. Their 29×29 candidate window covers roughly
100 m across. Full nearby detail switches at a size-dependent 5–10 m crown-center
distance (with ±1 m hysteresis) to simplified wood, twelve-triangle
needle shoots and sixteen-triangle snow caps. Far shoots preserve a subset of
actual needle silhouettes rather than solid crossed cards. Eight near/far model pairs are built
once and reused; streaming never rebuilds tree geometry. The castle's courtyard
apple is retained regardless of biome.

The world has no fixed edge. A 7×7 window of 15-meter citrus chunks follows the player, unloading old roots and reusing the same mesh/material handles. Coordinates, yaw and scale are derived from the world seed and cell, never exploration order. Revisiting a cell in the same run restores the same tree. World seeds change between launches unless `HITHER_WORLD_SEED=<integer>` is set; the seed is logged. This is not a world-save system.

Near trees retain detailed citrus geometry; distant trees preserve every blade at four folded triangles instead of sixteen (4× fewer triangles), retaining trunk and primary scaffolds but omitting concealed fine wood. The raised midrib, authored normals and full material (including normal and roughness maps) remain at all distances. Citrus uses the same size-dependent detail range and hysteresis as conifers. World cells select from 16 pre-batched fruit layouts per morphology variant; all anchors remain on actual twigs. Each citrus tree uses four visible mesh parts (wood, leaves, fruit bodies and close-only stalk/calyx details), plus one shared shadow-only mesh. Shoots use five broader alternating leaves at all distances, reducing overlap while keeping the crown full. Far fruit uses simpler spheres. Detail refreshes after 0.5 m of movement, including vertical flight, independently of chunk boundaries. At most four entering citrus chunks or 24 conifers are spawned per frame, nearest first. Orange trunks block walking and their generated branch/crown bounds keep the follow camera out. Fruit anchors inherit each tree's scale and rotation. The orange tree's spawn rule is separate from the apple's 1% per-gameplay-second growth rule. See [performance measurements](FOREST_PERFORMANCE.md).

## Verification

- `cargo test --release`: finite geometry/normals, ray hits/misses, rotated apple seam hits, 500 valid branch-attached spawn positions, spacing and capacity checks.
- An additional 32-seed test verifies different geometry, exact replay, finite normals, bounded mesh complexity and 1,600 valid apple placements across repeated harvest/refill rounds. Upward rays confirm actual rendered branches above every fruit.
- Timed ECS tests fill an empty tree, harvest everything, regrow to five and check visible mesh slots. Tests also verify pause behavior and identical growth across different frame durations.
- Wild-tree tests sample one million cell rolls, verify spacing and negative coordinates, validate reduced-detail meshes, run actual setup, cross distant chunk boundaries and revisit the origin while checking bounded chunk counts and unchanged mesh-asset counts.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.
- `bash tools/smoke_orchard.sh`: native Bevy overview/detail screenshots in an isolated Xvfb display with temporary settings. `bash tools/smoke_avatar.sh` covers camera modes, movement, jump and pause with the new scene.
- `HITHER_SMOKE_WILD=1 HITHER_WORLD_SEED=721 HITHER_PREVIEW_BIOME=forest bash tools/smoke_orchard.sh` exercises streaming through the temperate forest. Without the preview override, seed 721 starts at a tundra castle.

Four-biome checks cover all four possible castle biomes, coherent deterministic
regions, citrus/conifer habitat restrictions, sparse plains-oak rates, finite
lobed oak geometry, 20x foliage LOD reduction, bounded streaming, mesh reuse and
exact revisits. Overlapping citrus chunks retain their original entities during
movement; only entering/exiting chunks create/despawn fruit and branches.

Native biome captures: [plains](biome-plains.png), [temperate forest](biome-forest.png),
[tundra](biome-tundra.png), [boreal forest](biome-boreal.png), [giant oak](biome-oak.png).

No FPS improvement claim is made without a controlled benchmark. The heavy geometry and BVH are cached, foliage is batched into one mesh, and all five fruit bodies share one mesh/material.

### Startup profiling

Before this revision, the release headless `streaming_builds_assets_stays_bounded_and_revisits_same_trees` test took approximately 10.2 seconds. Instrumentation attributed 6.7 seconds to orange mesh setup (fruit selection itself took under 0.1 ms per variant). Procedural meshes now derive tangents directly from their existing UV-split vertices in linear time, avoiding expensive MikkTSpace welding/grouping at coincident leaf tips. The imported avatar and its authored tangents are unchanged. The same test took 0.23 seconds after the change; this measures CPU scene setup/streaming, not GPU initialization or total application launch time. Timings for orange geometry and asset setup are logged at INFO level. Tests check finite, unit-length, orthogonal tangents and mirrored UV handedness.

Latest native captures: [courtyard apple](orchard-overview.png), [branch detail](orchard-detail.png), [citrus close-up](orange-close.png), and [wild oranges beyond the castle](orange-world.png). Historical previews of the superseded forked design are retained as `orange-design-seed-7.png` and `orange-design-seed-28.png`.

Grazing ground rays fall back to an exact plane intersection when the SDF march budget is exhausted, so the ground does not disappear underneath distant mesh trees. The same reverse-Z depth calculation handles those ground hits.

## Fallen timber

`orchard::logs` samples accepted orange, apple and lowland spruce sites with an
independent 2% roll. Oak and mountain trees do not contribute logs. Both ends
and intermediate samples of fruit-tree logs must stay in their species' patch;
steep or uneven ground and den entrances reject placement, so actual density
can be slightly below one log per fifty trees. Existing standing trees are retained.

Each species has a long tapered trunk and a shorter, thicker fractured form,
with species-matched bark maps, modeled ridges, jagged end grain, annual rings,
radial checks and exposed splinters. Per-site seeds generate optional dead limbs
and forks rather than selecting preset branch layouts. An independent 1-in-50
roll adds raised, irregular moss patches to orange logs only. Revisiting a site
recreates exactly the same geometry and moss choice.

Logs stream two 15 m chunks per frame, nearest first, using the shared render
range. Unique mesh assets are released when their chunks unload. Capsule-like
trunk collision keeps players and follow cameras out of the wood; local placement
caches avoid repeating terrain sampling during movement sweeps.

Focused checks: `cargo test world::orchard::logs::tests`. An optional native
material contact sheet is available with `cargo test log_material_contact_sheet
-- --ignored --nocapture` under an X server; close its window to finish.
