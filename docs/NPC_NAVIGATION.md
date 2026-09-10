# Shared multi-surface navigation

`src/world/navigation.rs` owns routing, tile caching, support tracing and path following.
`src/world/orcs/navigation.rs` adapts the shared floor and world collision geometry and
handles crowd steering. There is no surface/underground mode in the planner or
in ordinary orc locomotion, and no hand-authored cave corridor in pathfinding.

## Geometry contract

`Geometry::floors(xz, agent)` returns **all** supporting floors at that horizontal
position. They can overlap vertically at any elevation. `body_clear` checks body
radius and ceiling clearance. `slope` can supply an analytic surface gradient;
its default uses local one-sided floor derivatives so normal stairs remain
traversable. Geometry producers must supply actual support and body clearance,
including their walls and openings, rather than a navigation-specific shortcut.
`support_boundaries` supplies segment fractions at analytic ledges and surface
transitions; the walker checks the boundary and both sides so short discontinuities are not
skipped between regular samples.

Nodes identify a globally aligned horizontal sample and an actual floor height,
not a surface/cave label or a floor index that changes when a door opens. A node
can connect only through a supported, clearance-checked walk. Starting or ending
above another floor does not create a connection through the ceiling.

The world adapter enumerates terrain/root tops, den roofs and lids, ramps,
and tunnel floors. Den support comes from the final rendered triangles through
[the shared floor system](FLOORS.md), including rounded character-base contact.
Player physics and NPC grounding use the same heights. The open ramp replaces
lid support across its actual opening; the iron lid opens across the ramp width.
A future mountain, bridge or cave generator must contribute its supporting
surfaces and colliders through this same contract.

## Routing and streaming

- Fine graph spacing is 0.25 m to retain nodes in narrow, off-grid passages. Aligned coarse junctions add checked 4 m and 16 m
  connections. These accelerate open-space travel; fine connections remain
  available around obstacles, switchbacks and entrances. This is a multiresolution
  sampled surface graph, not a Recast polygon navmesh or a precomputed region graph.
- Every edge, shortcut and movement step uses the same support trace. Samples are
  at most 0.08 m apart, with additional samples at and on both sides of analytic ramp
  boundaries. Body radius, headroom, maximum step and slope constrain traversal.
  Actor goal offsets are projected to nearby support once; internal graph edges
  must finish on their actual destination floor (5 mm numerical tolerance).
  Local step checks and surface gradients replace a conflicting whole-segment
  rise cap, so a slope followed by a step remains traversable as the body advances.
  Long shortcuts require approximately planar support, preserving detailed
  waypoints through curved ramps and stairs.
- Lazy 16 m tiles are shared across orcs. Up to 256 tiles are cached; least recently
  used tiles are evicted. Negative world coordinates use Euclidean tile division.
  Cached columns distinguish agent clearance profiles.
- Gate state or gate motion invalidates tiles around that geometry. Seed changes
  reset the cache. Other future geometry edits must call `Graph::invalidate` with
  their bounds expanded by body/edge clearance. Active queries and paths restart
  on a geometry revision; this restart is currently conservative across agents,
  even though cached tile eviction is local.
- A* persists across frames with 48 queue pops per agent and 192 shared per frame.
  The first serviced agent rotates each frame to avoid starvation. There is no
  80 m radius limit. A query is limited to 65,536 discovered nodes. These are work
  and memory limits, not a guaranteed wall-clock budget.
- Results distinguish searching, complete, unreachable, budget exceeded and
  invalidated. Incomplete searches do not masquerade as successful partial paths.
  Moving targets do not continually cancel an unfinished search; an agent can
  follow its still-valid previous route while computing a replacement. A proven
  direct route to a changed target replaces an unfinished query immediately.

## Locomotion and gameplay

Orcs use one `Navigator::steer` request with their real destination and one
`walk`-based movement function. A nearby waypoint is consumed only if the next
segment is reachable from the actual body position and the body is within 2 cm
of the turn (the final destination keeps its 15 cm arrival tolerance). This preserves necessary
turns at ramp lips and corners instead of repeatedly replanning a route whose
first necessary turn was discarded. Candidate crowd velocities favor preserving a
traversable connection to the current waypoint after movement, including coordinate
rounding. Every candidate must pass a local body/support sweep; a safe escape step
can trigger corridor replanning on the next frame. Scoring rewards reducing the
remaining distance to that waypoint. Separation is never added after validation.
Stable entity order gives residents right of way, with a larger yielding distance
for the other resident. Full-circle velocity sampling lets them back away when
they converge on a narrow mouth. This is local avoidance, not a full reciprocal crowd solver.

Before routing, gravity settles residents onto the next supporting floor if a lid
opens beneath them. Vertical movement is swept against collision and stops at the
first floor, preserving distinct levels. Unsupported bodies do not repeatedly
request impossible walks from their former support height. The navigator keeps
supplying the corridor waypoint to collision-checked avoidance; it does not
restart merely because a new centerline trace disagrees at an intermediate pose.
Geometry changes and sustained lack of net progress still trigger replanning.
Progress uses a fixed position anchor, so alternating steps within half a body
radius count as a stall rather than continually resetting recovery.

Grounding also repairs small floor penetration after scripted emergence or
coordinate rounding: it selects the nearest support within the normal step
allowance and checks the vertical correction against body collision. A floor
slightly above the feet is not discarded as though the resident were falling
in open air. Obstructed corrections wait rather than crossing the obstacle.
Combat still requires 3D proximity and intervening clearance. Authored gate-opening, emergence and
retreat animations remain encounter choreography.

## Verification and boundaries

Tests cover three stacked floors connected by ramps on opposite ends, repeated
at -10,000, zero and +10,000 elevation; closing the only connection; routes beyond
80 m and across negative tiles; local invalidation; agent-size cache separation;
zero search budget; concave obstacle escape; slope and stair behavior at different
movement step sizes; and cached-path recovery after a door reopens. Actual-world
cases descend and return through all three den styles, stop at closed hatches,
walk above tunnels and cross a closed lid. Regression coverage also exercises
off-center ramp starts across multiple den placements, entry and exit at 30 and
144 Hz, and a tight stepped corner at 30, 60, 144 and 240 Hz. The rest of the game tests cover
player movement, encounter and animation behavior. Additional regressions cover
lid support disappearing, thin passages between grid columns, immediate recovery
from stale searches, and three-orc pursuit through repeated den entry and exit.
Every frame of scripted emergence is tested as a grounding handoff in all three
styles. An encounter-system test interrupts three residents before emergence
finishes and follows a player inside and back outside, including live gate state.

Finite horizontal and edge-sampling resolution remains a limitation: features
narrower than these resolutions need finer sampling or a different geometry
backend. The graph does not impose an elevation band, but positions still use
the game's `f32` coordinates and retain their precision limits. Walking is the
supported movement capability; flight, swimming, ladders and jumps require
explicit traversal actions when those mechanics exist. Huge labyrinthine routes
can exhaust the query budget; a polygon/region hierarchy can later replace the
sampled graph behind the same geometry and navigation boundary.
