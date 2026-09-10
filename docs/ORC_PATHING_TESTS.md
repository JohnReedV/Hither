# Orc pursuit regression suite

Run `bash tools/test_orc_pathing.sh`. The runner uses optimized test code with
runtime assertions and serializes tests that share gate state. It does not
change Cargo's development or release profiles. There is no test-count quota.

The Cartesian-product terrain matrix has been removed. Each pursuit test now
has a named failure condition rather than a separate name for every combination
of distance, frame rate and coordinates. The retained checks cover:

- Starting pursuit of a distant moving target before a full route is ready.
- Following a crest smaller than the step limit without losing support.
- Keeping up route updates across multiple crests with a moving target.
- Replacing a clear local corridor with a wall detour.
- Preserving a detour when a moving target changes the desired heading.
- Returning around a wall when the target reverses direction.
- Rejecting a missing floor and rejecting an excessive continuous slope.
- Reaching a lower floor through its ramp rather than following overhead ground.
- Retaining engagement across the acquisition boundary, and reacquiring after release.
- Preserving elevation-independent engagement while respecting spectator mode.
- Running the live AI downhill until it enters valid attack range.
- Keeping a full party engaged with a faster, jumping player uphill.

The new oscillation and workload regressions separately check historical route
handoff, two-point stall detection, backward local avoidance, resumable long-edge
sweeps, bounded recovery from an embedded position, and cache invalidation
when a gate closes. Existing den, gate,
collision, floor-transition and crowd-separation regressions remain in the run.

## Chase implementation

An acquired target remains engaged out to 192 metres horizontally; acquisition
still requires 50 metres. Supported route checks follow the actual floor.
Outdoor pursuit starts on a verified one-metre corridor and renews it immediately
at the end. Obstacles and layered destinations use the full route search.

Completed searches retain their result until they can attach it safely to the
actor's current position. Historical start points are never installed merely
because the planning slice expired. Progress means reducing the best distance
to the current waypoint, so alternating between two positions triggers a replan.
Den residents keep a corridor through corners instead of reselecting their
nearest graph anchor every frame. Local avoidance leaves backward routing to
the planner; crowd separation handles overlap independently.

Long graph edges yield between quarter-metre sweeps and resume without caching
an unfinished check as blocked. Recovery tests eight candidate positions per
actor update and resumes its cursor next frame. Exact geometry queries are
shared within one AI update and discarded before the next update so gate changes
cannot reuse stale collision results. These are bounded pieces of work, not a
hard real-time guarantee: individual geometry queries and OS scheduling can
still exceed a nominal deadline.

## Measurement

The full-party test prints AI update CPU times with `--nocapture`. It includes
planning, movement, separation and the live ECS system, but omits rendering and
animated model processing. Use identical optimized builds and serial test runs
for comparison; these timings are not whole-game FPS measurements.
