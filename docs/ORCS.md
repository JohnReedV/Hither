# Orc dens

Seeded dens use 288-meter regions and 25% regional acceptance, matching the
current solitary-oak rarity rule. Unlike oaks, they do not filter out forests
or snowy biomes. Castle clearance is retained; forest trunks leave entrances clear.

The analytic ground has a 4.4-by-8-meter entrance over a descending earthen
ramp. It drops nearly six meters, then continues through a full-height arched
passage that bends out of sight and darkens toward the back. The ramp and tunnel
floor share the same 20-segment boundary at z = -4 m, with the tunnel blending
into its rounded section over the next 1.2 m. The natural den narrows both sides
of that shared boundary together, preventing an open floor seam. Exposed roots,
embedded rocks, timber shoring, iron straps, supplies, chains and old bones
surround a warm, flickering torch. Soil and corroded iron use
small shared color, normal and roughness maps. Fixed bars surround a central
hinged hatch with interior bolts. Players and emerged orcs can walk across the
grille at its 14 cm support height; it is no longer an invisible perimeter wall.
When the hatch is open its central support is removed: players can fall onto
the ramp, walk underground, and climb back out. The side grilles remain solid.
An occupied den keeps its hatch open; underground support never snaps a player
back onto the grille. Tunnel walls and ceiling constrain underground movement.
Each lower room has an iron-bound wooden wall torch at z = -8 m, just past the
ramp. The metre-long, uneven shaft has shared 1024×2048 color, normal and
roughness maps with bent grain, knots, fine pores, splits, worn fibres and a
blackened top. Thick hammered iron bands have rolled edges and raised rivets;
the bracket holds a charred, cord-wrapped fuel basket with glowing coals.

The flame is a view-independent ray-marched volume with five curling tongues,
upward-moving turbulence, cobalt-blue combustion at the base, golden-yellow
wisps and rising embers. Two warm lights follow the same animated flame paths,
with a combined nominal 92,000 lumens over 11.5 m, plus a small blue base light
and a short-range warm bounce light that reveals the grip beneath the basket.
Their brightness follows the combined flame heights. Only a torch within 18 m
of the camera enables its main shadow map; other flame lights remain unshadowed.
The shared fire clock pauses with gameplay, and world-position phase offsets
keep separate dens from flickering in lockstep. Meshes, textures and the flame
material are shared; no per-frame geometry or particle entities are allocated.

Orcs acquire a player within 50 m horizontally and retain an active pursuit up
to 192 m. Elevation changes and crossing the acquisition boundary do not cancel
chasing. Spectators remain excluded. Supported shortcuts follow hills and
valleys instead of requiring a planar floor. Distant outdoor pursuits start on
a verified 1 m corridor, then revalidate or route around obstacles; layered
destinations and existing detours continue through the full navigator.
See [the pathing test suite](ORC_PATHING_TESTS.md) for the focused regression
coverage and its runner.

Orc route searches and movement share the same step and slope checks. A steep
local surface normal is accepted as small ground roughness only when sampled
support across the body footprint fits within the 20 cm step allowance. This
lets orcs walk over low ridges at any frame rate while retaining body clearance,
ledge limits, and rejection of sustained steep slopes.
If an orc's body has a shallow wall overlap, settling searches for a correction
within 12 cm and sweeps a capsule expanding back to full size along that move.
Recovery stays within one step of the original floor and rejects deep embedding.
An actor already on a steep bank can move continuously downhill to leave it;
this does not permit climbing or entering steep terrain from walkable ground.
Unfinished routes retain valid moving targets, but a changed return queue slot
replaces an obsolete destination if it lacks body clearance or floor support.

Underground collision keeps entrance walls separate from the room's rounded
support cap. The entire body stays inside the passage's horizontal sections,
with continuous clearance through the natural throat. Horizontal player movement
rejects bank penetration deeper than the landing tolerance, including after a
step changes which side of a lid supplies support. Camera clearance additionally
checks cached triangles from the rendered room floor and roof across its footprint,
so an orbit camera cannot slip beneath a raised bank or above the arch.

Player landing queries use physical feet rather than camera bob. Descending
players recover floor penetrations within the same 20 cm walking step allowance,
so entering a rising ramp while airborne cannot leave them falling below its
floor. Regression coverage samples all three dens on a 20 cm grid with closed,
partially opened, and fully opened lids, plus jumping ramp crossings at three
frame rates.

Crowd steps use each resident's latest position and enforce physical body clearance.
A separate four-pass contact solver separates overlapping residents even while idle,
attacking, or waiting for a route. Stable entity-based directions resolve exact
coincidence. Corrections sweep the full body along supported ground, including
slopes, and try sideways movement beside walls. The stone carrier remains a
kinematic obstacle during the lift.

After the initial release, non-rock lids reopen only for a resident with no
player target arriving near the return mouth or already re-entering. An underground player keeps an escape route: an occupied entrance reopens even
if closing had started. A pursuing orc alone does not reopen it. Reacquiring a
target cancels an NPC return request when no player is underground. Bank rocks
and shoring share solid barriers with player physics and navigation; see
[ORC_LAIRS.md](ORC_LAIRS.md) for clearance and recovery checks.
Returning residents approach the mouth, then route to the interior once within
2.5 m. Crowd contact resolves the shared passage; fixed queue slots are avoided
because they can reverse the order of bodies already crossing the lip. Regression
coverage requires the whole party to enter, despawn, and let every den close
and rearm from both the lip and farther outside at 30 and 144 FPS.


Entry, pursuit, and return all use the supported navigation graph. There is no
timed centerline emergence or return script to stall against elevation changes.
Only the stone carrier follows the mandatory lid-lifting animation. Chase
destinations use the highest clear supporting floor below the player; attacks
still measure the actual player position.

Connectivity floods are prepared on the asynchronous compute pool within 96 m
of a den. Physics never waits for a cold flood; ordinary support and swept body
collision remain active until its connectivity result is available. The prepared
graph stores verified floor connections and shares a reverse route for the party's
destination. Den pursuit uses these connections directly, avoiding expensive
searches across the outdoor ground above a tunnel. Live sweeps still check moving
gates and every crowd step. Outdoor travel uses the general world navigator.
Shared floor and barrier geometry is warmed during startup. Gate animation only invalidates
routes at topology transitions and animation endpoints, allowing searches to
finish while a lid moves. Locomotion still tests the gate's current pose.

Optional route smoothing retries every 150 ms instead of retracing the same
corners every frame. New routes and reached waypoints trigger immediate smoothing;
geometry changes still invalidate and replan immediately. Every movement step
retains its swept collision, elevation and crowd checks. Across level or descending
samples, the previous body pose is already verified; only upward steps need an
additional raised-body clearance query.

Chase planning shares a 1 ms per-frame time budget in addition to the node budget.
They yield between graph edges and retain unfinished expansion cursors. Verified
directed edges are cached by agent shape and cleared on geometry invalidation;
the cache is bounded to 65,536 entries. Once the planning deadline is reached,
residents continue following valid existing routes and defer new planning and
smoothing. Local avoidance scores candidates before sweeping their bodies, and
only sweeps candidates that can still win. Its extra lookahead is limited to
40 cm rather than rechecking the entire remaining path on every movement step. These
changes reduce repeated work when a player switches sides around a barrier.
Run the explicit CPU benchmark with
`cargo test barrier_switching_chase_benchmark -- --ignored --nocapture`; it reports
median, 95th percentile, and maximum chase CPU time for three residents.



A nonspectating player within ten meters alerts a den. Each encounter rolls:

- 25%: one orc
- 50%: two orcs
- 25%: three orcs

Each orc independently has a one-third chance of being the mace brute,
ironcap axeman or crested raider. Party randomness is separate from the den
placement roll. Dens trigger once per application session; leaving and returning
does not create duplicate parties. This memory is not yet saved across launches.

The entire party spawns in one update when the den is alerted. Orcs ascend as the entrance opens,
pursue the player with collision and local separation, and swing their weapons
at close range. Spectators do not alert dens or attract pursuit. Pause freezes
AI and animation. Health, damage, loot and sound effects are not implemented.

The three embedded GLBs use the bundled CC0 MakeHuman anatomical base and
Hither Avatar v1 skeleton/locomotion clips, plus three attack clips. They do not use
the player's portrait overlay. Authoring: `tools/build_orcs.py`; isolated asset
preview: `tools/preview_orcs.py`. Source attribution follows
`docs/HITHER_AVATAR_V1.md` and the existing MakeHuman assets.

The three variations share the goblin's anatomical surface approach: subdivided
skin with sculpted brow/cheek folds, UV-painted warm cartilage and lips, subdued
olive tones, healed scars, pores, and embedded albedo/roughness/normal maps.
`tools/orc_surfaces.py` also authors woven canvas, cracked hide, and dark hammered
steel with localized oxide and variable metalness. Shoulder plates and the helmet
are open shells with thickness; garments have folds and visible rims. Tusks are
continuous curved meshes, and mace flanges and axe edges have forged bevels.
The Bruiser, Ironcap, and Raider retain their individual equipment and palettes.

Rebuild and verify the shipped assets with:

```sh
tools/.venv/bin/python -u tools/build_orcs.py
tools/.venv/bin/python tools/check_orcs.py
HITHER_ORC_FACES=1 tools/.venv/bin/python tools/preview_orcs.py
HITHER_ORC_CLIP=Walk tools/.venv/bin/python tools/preview_orcs.py
```

The validator checks all three GLBs for embedded PBR textures, the 62-joint rig,
18 animation clips, finite animation samples, and normalized skin weights.

All three orcs use brown irises. Attacks last 0.8 seconds (sweep), 0.9 seconds
(backhand), or 1.0 second (overhead), with a brief windup, quick strike and
continuous recovery. Attacks blend in over 60 ms and play once; the next swing
is selected when the actual clip finishes rather than by a separate timer.
During recovery, the free left hand reaches forward and rakes across with curled,
spread fingers, then returns to its guard. This additive claw scratch changes
only left-arm/hand animation channels; weapon swings and clip durations remain
unchanged.

The stone carrier opens and rotates the left palm upward during the lift. The
rock's center follows a baked point on the palm surface, with its flat bearing
face 1.40 m below the center resting on that point. The supporting arm stays high
through release, and the airborne stone inherits the resulting contact velocity.
`tools/preview_orc_carry.py` checks and renders the exported hand against the
runtime rock mesh; its module header describes the mesh-export command.

All variants have bare anatomical feet and long dirty toenails. Walking orcs
hold their tools with curled fingers baked into every animation, including attacks.
Tool hafts are positioned inside the curled hand rather than at the wrist.
`tools/preview_orc_grip.py` renders an isolated animated-hand close-up for QA.
Walking orcs
flatten nearby grass and leave alternating barefoot prints in snow for 30 seconds.
Orc prints have broad forefeet, separate toes and claw furrows, with no boot lugs.
The shared footprint pool is bounded to 512; grass deformation uses the nearest
16 ground-level orcs to keep vertex-shader work bounded.

Meshes/materials are shared, dens stream with render range, and distant actors
are removed. The secondary interior fill lights do not cast extra shadow maps.

Background-only QA: set `HITHER_PREVIEW_BIOME=orcs` with
`HITHER_GRAPHICS_PREVIEW=1 bash tools/smoke_graphics.sh`. Add
`HITHER_DEN_INSPECT=1` to inspect the closed grate as a spectator. This uses the
private Xvfb display, never the user's desktop.

Torch close-up QA: `bash tools/smoke_torch.sh` (after a release build) captures
two moving frames and two paused frames on private Xvfb display :96. The
`HITHER_TORCH_INSPECT=1 HITHER_PREVIEW_BIOME=orcs` startup view is opt-in only.
