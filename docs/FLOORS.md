# Shared floor system

`src/world/floor.rs` provides indexed triangle queries and rounded character-base
contact. `src/world/orcs/floors.rs` combines those surfaces with outdoor
support, fixed grates, roofs and moving doors. Players, NPC grounding,
navigation and scripted den travel consume this support data.

## Geometry is the source

The den generator marks floor and ceiling triangle ranges while constructing
the render mesh. The index reads the final vertices after variant deformation.
It does not reconstruct a tunnel from its centerline or evaluate a second ramp
height formula. All sites of a style share one immutable index, divided into
half-metre spatial bins. A bounded cache reuses exact capsule contacts for
repeated positions and radii. Camera clearance reads this same mesh.

Floor queries return no hit outside the mesh. Layer enumeration adds outdoor
ground and lids separately, so a tunnel floor and the surface above it coexist.
Support selection takes the highest reachable layer below the feet, with the
normal step tolerance for small penetration. An altitude threshold does not
select the support layer. The iron lid opens across the full ramp width; its fixed hinges remain
at the bank. Bank props are placed outside the shared exit corridor.

## Contact and motion

A point-height query alone is insufficient on curved floors: a character's
center can be clear while the edge of its base intersects the bank. Rounded
support finds the highest contact required by the capsule's lower sphere over
each nearby triangle. It checks the plane tangent and the constrained maxima
on all three edges, including vertices. This handles slopes and floor seams
without adding location-specific offsets or invisible flat floors.

Body containment requires footprint coverage, support clearance and headroom.
The ceiling belongs to the floor below it; solid earth above that ceiling is
never accepted as free space, even if the actor is already embedded there.
The existing conservative rock and shoring barriers remain solid. Their tops
can catch a falling player; isolated props are not NPC navigation destinations.
Player steps and gravity, NPC support traces and swept settling all query the
same floor heights. Scripted entry and return provide horizontal destinations
to the same supported movement trace as navigation, rather than interpolating
a separate vertical path. Moving obstacles contribute live support and
invalidate affected navigation tiles.
`src/world/orcs/floor_regions.rs` floods supported movement from the open
front edge to certify which cave-floor samples connect to the entrance. The
region cache uses each site's full world coordinates and the same grid and
boundary samples as pathfinding. Local-coordinate interpolation can round
differently far from the origin and certify an edge that runtime movement rejects. Recovery
rejects isolated patches behind rocks and steep banks, even when the capsule
fits there; a recovery destination must support ordinary walking.

Bounded overlap recovery remains a fallback for moving barriers and previously
embedded actors, rather than compensating for mismatched floor geometry.

The reusable mesh query's `rounded_support` operates on one connected floor
mesh. Separate stacked structures should provide separate meshes/layers and
combine their results through the support adapter. Coordinates in the core are
feet coordinates; the existing player-facing wrapper converts eye coordinates.

## Regression checks

Run `cargo test -- --test-threads=1`. Coverage includes:

- Triangle interiors and shared edges against the final rendered vertices of
  every den style, including ceilings and deformed floor seams.
- Capsule contact against analytical slopes and independently sampled triangle
  edges, missing support, and stacked floor selection.
- Player and navigation support agreement across all styles, with fast falls
  landing on the same visible floor instead of the ground above the tunnel.
- Both bank-contact exits: press fully into the bank, then walk forward at
  20/60/144 Hz. Blocked motion slides through checked short steps.
- Seven straight exit lanes from -1.2 m to +1.2 m at 20/60/144 Hz, with
  no jumping or steering and at most 15 cm of lateral correction.
- Existing entrance drops, moving gates, bank recovery, pursuit, scripted
  returns, camera clearance and exits at 20/60/144 Hz.

After building release, `bash tools/smoke_dens.sh` exercises each style in an
isolated Xvfb display. It never focuses or sends input to the user's desktop.
