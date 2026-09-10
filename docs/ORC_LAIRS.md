# Orc lairs

Sites retain the existing rarity and occur in every biome. A separate seeded hash
chooses one of three equally weighted styles, without changing encounter odds
(25% one orc, 50% two, 25% three). Styles remain stable when revisiting a site.

- **Barred burrow:** full-width rusted iron hatch, excavated banks, shoring, chains,
  scattered bones and stores, with a faint warm light beyond the ramp.
- **Stonejaw quarry:** bulky irregular boulder plugging a compact front mouth beneath a fixed rocky overhang, shale outcrops, mineral seams,
  picks and ore piles. Cool light reveals the excavation near the bend but does
  not illuminate the far end. The first orc uses the 7.5-second `Push` clip to
  lift the boulder overhead, carry it up the ramp and throw it aside.
  Its contact path is baked from the animated palm into `stone-contact.json`;
  release inherits that path's velocity and follows gravity, then rolls to rest.
  The boulder stays aside until the party returns and the site resets.
  Followers begin climbing independently and test the moving boulder collider;
  they do not wait for the throwing animation to finish. There is no fixed grille.
  Resting rock support follows its position; it is not a flat walkable metal hatch.
- **Rootwarren:** an earthen excavation sealed down its ramp by a woven-root
  and stitched-hide plug, with bone fasteners and draw ropes. The plug retracts
  inward into the bank; collision follows its moving volume. Exposed branching
  roots, ochre shelf fungi, buried bones and low spoil heaps dress the banks.
  No roof, standards or iron grille. Its rounded mouth
  is shared by the terrain cutout and collision. Interior dressing
  stays below grade; the rounded passage bends away into darkness.

All three use the same rough, walkable ramp and bending underground envelope.
Dressing occupies the banks outside a 3.3-metre clear ramp corridor. The
Rootwarren mouth retains rounded corners without a narrow throat. Where present, closed doors support
walking; open doors remove central surface support and remain open while the
player is underground. Geometry is cached once per style and shared by sites.

Bank rocks, spoil heaps, crates and upright shoring have conservative collision
bounds derived from their final rendered vertices, including Rootwarren's
rounded deformation. A metre grid caches nearby barriers for both player
collision and orc navigation. The underground space behind bank rocks is solid,
so bodies cannot land in hidden pockets between a rock and the excavation wall.
Props leave the central throat and the stone lifter's stance clear in both the
mesh and collision geometry.

Orcs treat the small rock tops as barriers rather than navigation platforms.
Recovery chooses nearby clear ground in the entrance-connected floor region
on the navigation grid;
a perched resident moves through clear space before swept gravity lands it.
Both scripted emergence and return sweep their motion through the barriers.
Player jumps reserve full body clearance below the roof and shoring.

Players and orcs use the [shared floor system](FLOORS.md): floor and ceiling
triangles are indexed from the final rendered mesh, and support accounts for
the rounded body footprint on curved banks. The same support feeds navigation,
gravity and scripted travel. The quarry boulder and Rootwarren plug contribute
live landing support using their collision dimensions. Recovery from existing
penetrations preserves the underground layer; camera landing motion does not
change physics support.
An occupied entrance reopens even if the player entered during closing; NPC
chase behavior alone still does not reopen it.

Regression coverage checks drops over the entrances and tunnel perimeters at
four gate phases, moving-gate clearance, room to move after landing, walking out of all three
styles at 20/60/144 Hz, and recovery from bank and floor penetrations. Run
`cargo test -- --test-threads=1` for the player cases plus shared navigation and
camera checks (NPC frame deadlines are sensitive to concurrent test load).
Additional regressions verify every authored rock blocks both body queries,
the scripted routes clear all rocks, and residents embedded along both banks
of every style recover and navigate all the way outside.
After `cargo build --release`, `bash tools/smoke_dens.sh` captures native third-person
views inside, outside, and back inside each style, then front views of pursuing
residents while the player presses against each bank, using an isolated Xvfb display.

After emergence, orcs target a non-spectating player only within 50 meters.
Chases use one shared, tiled multi-surface graph for every elevation, with
clearance-checked connections, local crowd steering and 3D obstruction-aware
attack reach. See [NPC_NAVIGATION.md](NPC_NAVIGATION.md) for architecture, budgets
and regression cases.
Otherwise they route to the front of their own entrance, request its gate open,
wait for clearance, and descend to the hidden end before despawning. A player
entering range interrupts the return; an orc already descending reverses out
along the ramp before pursuing an above-ground player. Underground intruders
interrupt both emergence and return immediately, without first marching to
the outdoor waypoint (the stone opener completes its lid push first).
Once the entire party has returned and despawned, the player has left, and
the gate has closed, the encounter rearms for the next visit. Active parties retain their home across render
distance changes. Footprint yaw uses actual post-collision travel and the snow
shader's toe-axis convention, including obstacle steering.

## Performance set

All den styles share a sealed rear tunnel, with continuously interpolated
darkening to black before the hidden end. Rear collision stops before the seal.
Understory sites exclude the entrance plus foliage clearance. Underground orcs
request the hatch open even after their scripted emergence is interrupted;
closed-hatch collision and scripted-exit clearance prevent crossing a shut gate.
Use `HITHER_DEN_BOTTOM=1 HITHER_DEN_REAR=1` for background rear-tunnel QA.

`tools/orc_outfits.py` authors distinct equipment for each variation:
- Mace bruiser: scarred oxhide jerkin, asymmetric salvaged iron ribs, stitched
  repairs, uneven hide skirt and ridged shoulder protection.
- Ironcap: forged breastplate, overlapping waist lames, oxblood split tabard
  with rough clan tallies, knee guards and reinforced helmet brow.
- Axe raider: faded moss canvas, ragged hide mantle, diagonal bandolier,
  tooth trophies and lighter one-sided shoulder armor.

All have a bent iron buckle and toggle pouch, worn baked cloth/leather textures,
rusted iron, and uncovered feet. Added details are joined by material into a
handful of skinned meshes, preserving attachment weights rather than adding a
separate draw for every stitch/rivet. Rebuild with `tools/build_orcs.py`.

`tools/orc_animation.py` authors three breathing idles (neutral, scanning,
weary), two walk performances, three attacks (swing, backhand, overhead), and
an alert reaction. The existing turn, jump, fall, landing, strafe and push clips
remain in each asset, for 18 clips total. Runtime selects varied idles/attacks,
uses the heavy gait for the ironcap variant, offsets initial gait phase per
individual, and crossfades state changes over 160 ms. Locomotion-facing turns
are smoothed separately from footprint direction, which still follows travel.
Attacks use planted feet, pelvis/spine weight transfer, a high chamber and a
shoulder-driven downward chop with the elbow folded below the hand. The haft
stays in front through acceleration, follow-through and recovery. Weapon-carry poses also
cover imported backward/sideways locomotion. Asset authoring validates weapon
vertices against the moving head and both leg capsules throughout attacks and
walking/idles, after applying the permanent finger grip. Clip frames use the
scene's 60 Hz timebase, matching the runtime animation durations.

Nine local facial deform bones add blinking, squinting, asymmetric brows,
lip curls and jaw/cheek tension. Channels are baked into all clips; no special
runtime face shader or morph pipeline is needed. Torso geometry is retained
beneath the cuirass, including continuous armpit skin. Only trouser-covered
lower-body geometry is masked. Health/damage/death logic is not introduced by
this animation update; authored jump/turn clips do not imply new AI abilities.

Background QA: use `HITHER_PREVIEW_BIOME=orcs HITHER_DEN_KIND=0|1|2` with
`tools/smoke_graphics.sh` (isolated Xvfb). `HITHER_DEN_INSPECT=1` examines a closed
entrance without triggering it; `HITHER_DEN_ENTER=1` tests descent through the
opened door. All three orc assets include the additional `Push` animation and
retain their existing skeleton, grip and locomotion clips.
