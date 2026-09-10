//! World geometry adapter and crowd steering for the shared navigation graph.
use super::*;
use crate::world::navigation::{self as nav, Agent, Geometry};
pub(super) use nav::Navigator;

#[derive(Default)]
struct QueryCache {
    enabled: bool,
    floors: HashMap<([u32; 2], [u32; 4]), Vec<f32>>,
    bodies: HashMap<([u32; 3], [u32; 4]), bool>,
}
thread_local! {
    static QUERIES: std::cell::RefCell<QueryCache> = Default::default();
}
fn shape_key(a: Agent) -> [u32; 4] {
    [
        a.radius.to_bits(),
        a.height.to_bits(),
        a.step.to_bits(),
        a.slope.to_bits(),
    ]
}
/// Exact queries are reusable only during this synchronous AI update. Starting
/// a new frame discards results, including results for moving gates and seeds.
pub(crate) struct FrameQueries;
impl FrameQueries {
    pub fn begin() -> Self {
        QUERIES.with_borrow_mut(|cache| {
            cache.floors.clear();
            cache.bodies.clear();
            cache.enabled = true;
        });
        Self
    }
}
impl Drop for FrameQueries {
    fn drop(&mut self) {
        QUERIES.with_borrow_mut(|cache| cache.enabled = false);
    }
}

pub(crate) struct World;
impl Geometry for World {
    fn floors(&self, p: Vec2, agent: Agent) -> Vec<f32> {
        let key = ([p.x.to_bits(), p.y.to_bits()], shape_key(agent));
        if let Some(value) =
            QUERIES.with_borrow(|c| c.enabled.then(|| c.floors.get(&key).cloned()).flatten())
        {
            return value;
        }
        let value = RawWorld.floors(p, agent);
        QUERIES.with_borrow_mut(|c| {
            if c.enabled {
                c.floors.insert(key, value.clone());
            }
        });
        value
    }
    fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
        let key = (
            [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()],
            shape_key(agent),
        );
        if let Some(value) =
            QUERIES.with_borrow(|c| c.enabled.then(|| c.bodies.get(&key).copied()).flatten())
        {
            return value;
        }
        let value = RawWorld.body_clear(p, agent);
        QUERIES.with_borrow_mut(|c| {
            if c.enabled {
                c.bodies.insert(key, value);
            }
        });
        value
    }
    fn support_boundaries(&self, a: Vec2, b: Vec2, agent: Agent) -> Vec<f32> {
        RawWorld.support_boundaries(a, b, agent)
    }
}
struct RawWorld;
impl Geometry for RawWorld {
    fn floors(&self, p: Vec2, agent: Agent) -> Vec<f32> {
        if let Some(mut levels) = crate::world::goblin_dens::levels(p, agent.radius) {
            if !crate::world::goblin_dens::surface_open(p) {
                levels.push(crate::world::terrain::height(p));
            }
            return levels;
        }
        let eye = Vec3::new(p.x, crate::player::movement::PLAYER_EYE_HEIGHT, p.y);
        // Enumerate the actual structural surfaces even beneath obstacles.
        // Clearance rejects covered floors; deleting them here makes slope
        // probes jump to an overhead lid and invent a steep bank beside rocks.
        floors::levels(p, agent.radius).unwrap_or_else(|| {
            vec![crate::world::orchard::oak::support_height(
                eye,
                agent.radius,
            )]
        })
    }
    fn support_boundaries(&self, a: Vec2, b: Vec2, agent: Agent) -> Vec<f32> {
        let cell = (a / CELL).floor().as_ivec2();
        let Some(home) = site(cell) else {
            return Vec::new();
        };
        den_boundaries(a, b, agent, den_kind(cell), home)
    }
    fn body_clear(&self, feet: Vec3, agent: Agent) -> bool {
        if let Some(clear) = crate::world::goblin_dens::body_clear(feet, agent.radius, agent.height)
        {
            return clear;
        }
        !crate::player::movement::character_collides(
            feet + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
            agent.radius,
        ) && ceiling_at(feet).is_none_or(|top| feet.y + agent.height <= top)
    }
}

// Explicit geometry identity keeps background preparation independent of
// changing world seeds and moving gates.
pub(super) fn den_boundaries(a: Vec2, b: Vec2, agent: Agent, kind: usize, home: Vec3) -> Vec<f32> {
    let a = a - home.xz();
    let b = b - home.xz();
    let d = b - a;
    let mut result = Vec::new();
    if a.length_squared().min(b.length_squared()) < 24. * 24. {
        result.extend(collision::barriers(kind).boundaries(a, b, agent.radius));
    }
    if d.y.abs() > f32::EPSILON {
        for z in [-4., 4.] {
            result.push((z - a.y) / d.y);
        }
    }
    let natural = kind == 2;
    let half_width = 2.08 - agent.radius;
    if d.x.abs() > f32::EPSILON {
        for x in [-half_width, half_width] {
            result.push((x - a.x) / d.x);
        }
    }
    if natural && d.length_squared() > f32::EPSILON {
        // Rounded burrow banks: the same 0.4 m arcs used by burrow_width.
        let inset = 1.8 - 0.12 - agent.radius;
        for x in [-inset, inset] {
            for z in [-3.6, 3.6] {
                let q = a - Vec2::new(x, z);
                let aa = d.length_squared();
                let bb = 2. * q.dot(d);
                let cc = q.length_squared() - 0.4_f32.powi(2);
                let disc = bb * bb - 4. * aa * cc;
                if disc >= 0. {
                    let root = disc.sqrt();
                    result.extend([(-bb - root) / (2. * aa), (-bb + root) / (2. * aa)]);
                }
            }
        }
    }
    result
}

/// Navigation treats bank rocks as barriers, not tiny isolated platforms.
/// Release a landed/perched or embedded resident into nearby clear space,
/// then let swept gravity settle it onto the walkable ground.
#[derive(Default)]
struct Recovery {
    origin: Option<Vec3>,
    cursor: usize,
    cooldown: f32,
}
#[cfg(test)]
pub(super) fn settle(agent: Agent, feet: &mut Vec3, velocity: &mut f32, dt: f32) -> bool {
    settle_with_recovery(
        agent,
        feet,
        velocity,
        dt,
        &mut Recovery::default(),
        usize::MAX,
    )
}
fn settle_with_recovery(
    agent: Agent,
    feet: &mut Vec3,
    velocity: &mut f32,
    dt: f32,
    recovery: &mut Recovery,
    attempts: usize,
) -> bool {
    let eye = *feet + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
    let clear = World.body_clear(*feet, agent);
    let connected = floor_regions::connected(*feet);
    let floors: Vec<_> = World
        .floors(feet.xz(), agent)
        .into_iter()
        .filter(|y| World.body_clear(feet.with_y(*y), agent))
        .collect();
    let standing = floors.iter().any(|floor| (*floor - feet.y).abs() <= 0.201);
    let perched = !standing
        && surface_height(eye, agent.radius).is_some_and(|floor| (feet.y - floor).abs() < 0.10);
    let buried = floors
        .iter()
        .filter(|floor| feet.y >= -0.15 || **floor < 0.)
        .min_by(|a, b| a.total_cmp(b))
        .is_some_and(|floor| *floor > feet.y + 0.201);
    if !clear || perched || buried || !connected {
        if recovery.origin != Some(*feet) {
            *recovery = Recovery {
                origin: Some(*feet),
                ..Default::default()
            };
        }
        recovery.cooldown -= dt;
        if recovery.cooldown > 0. {
            return false;
        }
        let start = *feet;
        let end = recovery.cursor.saturating_add(attempts).min(1 + 60 * 32);
        'search: while recovery.cursor < end {
            let index = recovery.cursor;
            recovery.cursor += 1;
            let (ring, direction) = if index == 0 {
                (0, 0)
            } else {
                (1 + (index - 1) / 32, (index - 1) % 32)
            };
            let distance = ring as f32 * 0.05;
            let angle = direction as f32 * TAU / 32.;
            let at = start.xz() + Vec2::new(angle.cos(), angle.sin()) * distance;
            for y in World.floors(at, agent) {
                if start.y < -0.15 && y >= 0. {
                    continue;
                }
                let ground = Vec3::new(at.x, y, at.y);
                let candidate = ground.with_y(start.y.max(y));
                if !World.body_clear(ground, agent)
                    || !World.body_clear(candidate, agent)
                    || !floor_regions::connected(ground)
                {
                    continue;
                }
                // A merely clear sliver between a rock and the wall can
                // still have no navigation connection. Recover onto a
                // floor that reaches the graph's actual quarter-metre grid.
                let grid = (ground.xz() / nav::SPACING).round();
                if !(-1..=1).any(|x| {
                    (-1..=1).any(|z| {
                        let at = (grid + Vec2::new(x as f32, z as f32)) * nav::SPACING;
                        at.distance(ground.xz()) > 0.05
                            && nav::walk(&World, agent, ground, at).is_some()
                    })
                }) {
                    continue;
                }
                let horizontal_steps = (distance / 0.04).ceil().max(1.) as usize;
                if clear
                    && connected
                    && !(1..=horizontal_steps).all(|i| {
                        World.body_clear(
                            start.lerp(candidate, i as f32 / horizontal_steps as f32),
                            agent,
                        )
                    })
                {
                    continue;
                }
                let fall_steps = ((candidate.y - y) / 0.04).ceil().max(1.) as usize;
                if !(0..=fall_steps).all(|i| {
                    World.body_clear(candidate.lerp(ground, i as f32 / fall_steps as f32), agent)
                }) {
                    continue;
                }
                *feet = candidate;
                *velocity = 0.;
                recovery.origin = None;
                break 'search;
            }
        }
        if recovery.origin.is_some() {
            if recovery.cursor > 60 * 32 {
                recovery.cursor = 0;
                recovery.cooldown = 0.5;
            }
            *velocity = 0.;
            return false;
        }
    } else {
        *recovery = Recovery::default();
    }
    nav::settle(&World, agent, feet, velocity, dt)
}

#[derive(Default)]
pub(super) struct Navigation {
    pub graph: nav::Graph,
    pub cursor: usize,
    recovery: HashMap<Entity, Recovery>,
    followers: HashMap<Entity, floor_regions::Follower>,
    gates: HashMap<IVec2, (bool, u32)>,
    seed: Option<u64>,
}
impl Navigation {
    pub fn floor_waypoint(
        &mut self,
        entity: Entity,
        from: Vec3,
        target: Vec3,
        dt: f32,
    ) -> Option<Vec3> {
        self.followers
            .entry(entity)
            .or_default()
            .steer(from, target, dt)
    }
    pub fn retain_actors(&mut self, actors: &[(Entity, Vec3)]) {
        self.followers
            .retain(|id, _| actors.iter().any(|(e, _)| e == id));
        self.recovery
            .retain(|id, _| actors.iter().any(|(e, _)| e == id));
    }
    pub fn settle(&mut self, entity: Entity, feet: &mut Vec3, velocity: &mut f32, dt: f32) -> bool {
        settle_with_recovery(
            Agent::default(),
            feet,
            velocity,
            dt,
            self.recovery.entry(entity).or_default(),
            8,
        )
    }
    pub fn refresh(&mut self) {
        let seed = crate::world::biome::world_seed();
        if self.seed != Some(seed) {
            self.graph.reset();
            self.followers.clear();
            self.recovery.clear();
            self.gates.clear();
            self.seed = Some(seed);
        }
        let open = open_gates().read().unwrap().clone();
        let phases = gate_openness().read().unwrap().clone();
        let mut current = HashMap::new();
        for cell in open.iter().chain(phases.keys()) {
            current.insert(
                *cell,
                (
                    open.contains(cell),
                    // Moving gates are swept by locomotion every frame. Only
                    // topology transitions restart searches, not every animation tick.
                    match phases.get(cell).copied().unwrap_or(0.) {
                        p if p <= 0. => 0,
                        p if p >= 1. => 2,
                        _ => 1,
                    },
                ),
            );
        }
        for cell in self.gates.keys().chain(current.keys()) {
            if self.gates.get(cell) != current.get(cell)
                && let Some(home) = site(*cell)
            {
                self.followers.clear();
                self.graph
                    .invalidate(home.xz() - Vec2::splat(24.), home.xz() + Vec2::splat(24.));
            }
        }
        self.gates = current;
    }
}

/// Chase the support below an airborne player, not an unreachable point in
/// midair. Enumerating actual support retains the correct floor in a den with
/// overhead ground; attack range continues to use the player's real position.
pub(super) fn chase_goal(feet: Vec3) -> Vec3 {
    let agent = Agent::default();
    World
        .floors(feet.xz(), agent)
        .into_iter()
        .filter(|y| y.is_finite() && *y <= feet.y + agent.step + 0.001)
        .map(|y| feet.with_y(y))
        .filter(|p| World.body_clear(*p, agent))
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap_or(feet)
}

pub(super) fn attack_reachable(p: Vec3, target: Vec3) -> bool {
    if p.distance(target) > 1.65 || (p.y - target.y).abs() > 0.65 {
        return false;
    }
    // Reject a wall, hatch or steep floor between the two bodies.
    for i in 1..=6 {
        let q = p.lerp(target, i as f32 / 7.);
        if crate::player::movement::player_collides(
            q + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
        ) {
            return false;
        }
        let floor = crate::player::movement::world_support_height(
            q + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
            0.28,
        );
        if (floor - q.y).abs() > 0.45 {
            return false;
        }
    }
    true
}
pub(super) fn step(
    p: Vec3,
    toward: Vec3,
    dt: f32,
    neighbors: &[(Entity, Vec3)],
    owner: Entity,
) -> Vec3 {
    step_on(&World, p, toward, dt, neighbors, owner)
}

pub(super) fn step_on(
    world: &impl Geometry,
    p: Vec3,
    toward: Vec3,
    dt: f32,
    neighbors: &[(Entity, Vec3)],
    owner: Entity,
) -> Vec3 {
    let agent = Agent::default();
    let desired = (toward - p).with_y(0.).normalize_or_zero();
    let stride = dt * 1.55 * super::WALK_SPEED_MULTIPLIER;
    let mut best = p;
    let mut best_score = -f32::INFINITY;
    for angle in [
        0_f32,
        0.35,
        -0.35,
        0.7,
        -0.7,
        1.1,
        -1.1,
        std::f32::consts::FRAC_PI_2,
        -std::f32::consts::FRAC_PI_2,
    ] {
        let direction = Quat::from_rotation_y(angle) * desired;
        // Stop the forward step at its waypoint, but leave avoidance room to
        // move sideways even when that waypoint is extremely close. Backward
        // travel belongs to the route planner, not reactive avoidance.
        let length = if angle == 0. {
            stride.min(p.xz().distance(toward.xz()))
        } else {
            stride
        };
        let candidate = p + direction * length;
        if candidate.xz() == p.xz() {
            continue;
        }
        let mut penalty = 0.;
        let mut blocked = false;
        let mut overlap_before = 0.;
        let mut overlap_after = 0.;
        for (id, other) in neighbors {
            if *id == owner || (other.y - p.y).abs() > agent.height {
                continue;
            }
            let now = p.xz().distance(other.xz());
            let after = candidate.xz().distance(other.xz());
            let separation = agent.radius * 2.;
            // Physical clearance is identical for everyone. Extra yielding
            // distance is a preference, never an invisible wall in a passage.
            if now >= separation && after < separation {
                blocked = true;
                break;
            }
            overlap_before += (separation - now).max(0.).powi(2);
            overlap_after += (separation - after).max(0.).powi(2);
            let future = candidate + direction * 0.4;
            penalty += (0.9 - future.xz().distance(other.xz())).max(0.) * 0.7;
        }
        // Scripted emergence can hand off overlapping bodies. Reduce total
        // penetration instead of demanding movement away from every neighbor
        // at once (which pins the resident between two other bodies).
        if blocked || (overlap_before > 0. && overlap_after >= overlap_before) {
            continue;
        }
        // Reward remaining-distance reduction, not displacement along the
        // heading. The latter rewards overshooting close waypoints when world
        // coordinates quantize a tiny movement to a larger floating-point step.
        let progress = p.xz().distance(toward.xz()) - candidate.xz().distance(toward.xz());
        let recovery = (overlap_before - overlap_after) / stride.max(0.001);
        let score = progress / stride.max(0.001) + recovery * 8. - penalty - angle.abs() * 0.08;
        // Corridor visibility can only lower this score. Avoid tracing a long
        // route for candidates that cannot beat the best safe step anyway.
        if score <= best_score {
            continue;
        }
        // Score in the horizontal plane first; only promising candidates need
        // an actual terrain/body sweep. Geometry still decides every move.
        let Some(candidate) = nav::walk(world, agent, p, candidate.xz()) else {
            continue;
        };
        // Avoidance is local. Rechecking the entire remaining path here made
        // even a millimeter step cost hundreds of terrain/collision samples.
        let ahead = toward.xz() - candidate.xz();
        let lookahead = candidate.xz() + ahead.normalize_or_zero() * ahead.length().min(0.4);
        if nav::walk(world, agent, candidate, lookahead).is_none() {
            // A small score penalty still allowed a detour to leave the valid
            // corridor and strand the body beside a rock or on another layer.
            continue;
        }
        // The local sweep proves this step is safe. The navigator validates
        // the remaining corridor from the new position on the next frame.
        if score > best_score {
            best_score = score;
            best = candidate;
        }
    }
    best
}

/// Resolve physical crowd overlap independently of intent, including idle and
/// attacking bodies. Kinematic actors participate as obstacles but do not move.
/// Every correction follows support and sweeps static geometry at full radius.
pub(super) fn separate(
    world: &impl Geometry,
    agent: Agent,
    bodies: &mut [(Entity, Vec3, bool)],
    dt: f32,
) {
    let limit = (dt * 3.).min(0.15) / 4.;
    for _ in 0..4 {
        for i in 0..bodies.len() {
            for j in i + 1..bodies.len() {
                let (a, b) = (bodies[i].1, bodies[j].1);
                if (a.y - b.y).abs() >= agent.height {
                    continue;
                }
                let delta = a.xz() - b.xz();
                let distance = delta.length();
                let penetration = agent.radius * 2. + 0.002 - distance;
                if penetration <= 0. {
                    continue;
                }
                // Stable opposite directions also exist at exact coincidence.
                let direction = if distance > 0.0001 {
                    delta / distance
                } else {
                    let low = bodies[i].0.min(bodies[j].0).to_bits();
                    let high = bodies[i].0.max(bodies[j].0).to_bits();
                    let angle = (mix(low ^ high.rotate_left(23)) % 4096) as f32 * TAU / 4096.;
                    Vec2::new(angle.cos(), angle.sin())
                        * if bodies[i].0 < bodies[j].0 { 1. } else { -1. }
                };
                for (index, other, sign) in [(i, j, 1.), (j, i, -1.)] {
                    if !bodies[index].2 {
                        continue;
                    }
                    let start = bodies[index].1;
                    let amount = penetration.min(limit);
                    // Sideways alternatives let a wall-bound body yield along
                    // the passage instead of pressing harder into the wall.
                    for angle in [0_f32, 0.7, -0.7, 1.2, -1.2] {
                        let axis = Vec2::from_angle(angle).rotate(direction * sign);
                        if let Some(end) =
                            nav::walk(world, agent, start, start.xz() + axis * amount)
                            && end.xz().distance_squared(bodies[other].1.xz())
                                > start.xz().distance_squared(bodies[other].1.xz())
                        {
                            bodies[index].1 = end;
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct CrowdSlope;
    impl Geometry for CrowdSlope {
        fn floors(&self, at: Vec2, _: Agent) -> Vec<f32> {
            vec![at.y * 0.6]
        }
        fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
            p.x.abs() + agent.radius < 1. && p.y >= p.z * 0.6 - 0.001
        }
    }

    #[test]
    fn cached_collision_observes_a_gate_closing_on_the_next_update() {
        let (cell, home) = den(0, 139);
        let p = home + Vec3::new(0., -0.5, 0.);
        let agent = Agent::default();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        {
            let _frame = FrameQueries::begin();
            assert!(
                World.body_clear(p, agent),
                "fixture must be clear through the open lid"
            );
            assert!(World.body_clear(p, agent));
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().insert(cell, 0.);
        {
            let _frame = FrameQueries::begin();
            assert!(
                !World.body_clear(p, agent),
                "stale cached clearance ignored the closing lid"
            );
        }
        gate_openness().write().unwrap().remove(&cell);
    }

    #[test]
    fn embedded_recovery_resumes_instead_of_scanning_every_ring_in_one_frame() {
        let (cell, home) = den(2, 139);
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        let mut p = home + Vec3::new(2.08, -2., 0.);
        let mut velocity = 0.;
        let mut recovery = Recovery::default();
        assert!(!settle_with_recovery(
            Agent::default(),
            &mut p,
            &mut velocity,
            1. / 60.,
            &mut recovery,
            8
        ));
        assert_eq!(
            recovery.cursor, 8,
            "recovery exceeded its candidate allowance"
        );
        for _ in 0..240 {
            let old = recovery.cursor;
            if settle_with_recovery(
                Agent::default(),
                &mut p,
                &mut velocity,
                1. / 60.,
                &mut recovery,
                8,
            ) {
                assert!(World.body_clear(p, Agent::default()));
                assert!(floor_regions::connected(p));
                open_gates().write().unwrap().remove(&cell);
                gate_openness().write().unwrap().remove(&cell);
                return;
            }
            assert!(recovery.cursor <= old + 8);
        }
        panic!("incremental recovery never found connected floor: {p:?}");
    }

    #[test]
    fn rootwarren_lip_return_regression() {
        let (cell, home) = den(2, 131);
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        chase_at_dt(
            home + Vec3::new(0.0625, GRATE_HEIGHT, 4.078125),
            home + emergence_position(0.),
            1. / 144.,
        );
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }

    #[test]
    fn coincident_idle_crowd_separates_on_slopes_without_crossing_walls() {
        for dt in [1. / 30., 1. / 60., 1. / 144., 1. / 240.] {
            let mut bodies: Vec<_> = (0..3)
                .map(|i| {
                    (
                        Entity::from_raw_u32(i).unwrap(),
                        Vec3::new(0.7, 0., 0.),
                        true,
                    )
                })
                .collect();
            for _ in 0..(3. / dt) as usize {
                separate(&CrowdSlope, Agent::default(), &mut bodies, dt);
                for (_, p, _) in &bodies {
                    assert!(CrowdSlope.body_clear(*p, Agent::default()));
                    assert!((p.y - 0.6 * p.z).abs() < 0.001);
                }
            }
            for i in 0..3 {
                for j in i + 1..3 {
                    assert!(
                        bodies[i].1.xz().distance(bodies[j].1.xz()) >= 0.55,
                        "dt={dt}, bodies={bodies:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn crowd_respects_separate_floors_and_kinematic_carriers() {
        let id = |i| Entity::from_raw_u32(i).unwrap();
        let mut bodies = [
            (id(0), Vec3::ZERO, false),
            (id(1), Vec3::ZERO, true),
            (id(2), Vec3::Y * 4., true),
        ];
        for _ in 0..120 {
            separate(&CrowdSlope, Agent::default(), &mut bodies, 1. / 60.);
        }
        assert_eq!(bodies[0].1, Vec3::ZERO);
        assert_eq!(bodies[2].1, Vec3::Y * 4.);
        assert!(bodies[1].1.xz().length() >= 0.55);
    }

    #[test]
    fn moving_lids_do_not_restart_searches_every_frame() {
        let (cell, _) = den(1, 193);
        let mut navigation = Navigation::default();
        gate_openness().write().unwrap().insert(cell, 0.1);
        navigation.refresh();
        let revision = navigation.graph.revision;
        for frame in 11..95 {
            gate_openness()
                .write()
                .unwrap()
                .insert(cell, frame as f32 / 100.);
            navigation.refresh();
            assert_eq!(navigation.graph.revision, revision);
        }
        gate_openness().write().unwrap().insert(cell, 1.);
        navigation.refresh();
        assert!(navigation.graph.revision > revision);
        gate_openness().write().unwrap().remove(&cell);
    }

    #[test]
    fn residents_recover_from_rock_banks_and_route_out_in_every_style() {
        for kind in 0..3 {
            let (cell, home) = den(kind, 162);
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            for side in [-1., 1.] {
                for z in [-2., 0., 2.] {
                    let rock = collision::barriers(kind)
                        .boxes
                        .iter()
                        .filter(|b| {
                            (b.min.x + b.max.x).signum() == side
                                && b.min.x.abs().min(b.max.x.abs()) > 0.5
                                && b.max.y < 0.5
                                && b.min.z > -4.
                        })
                        .min_by(|a, b| {
                            ((a.min.z + a.max.z) * 0.5 - z)
                                .abs()
                                .total_cmp(&((b.min.z + b.max.z) * 0.5 - z).abs())
                        })
                        .unwrap();
                    let mut p = home + (rock.min + rock.max) * 0.5 - Vec3::Y * 0.3;
                    assert!(!World.body_clear(p, Agent::default()));
                    let mut velocity = 0.;
                    let mut graph = nav::Graph::default();
                    let mut navigator = Navigator::pursuit();
                    let target = home + Vec3::new(0., GRATE_HEIGHT, 5.);
                    let owner = Entity::PLACEHOLDER;
                    for _ in 0..1200 {
                        if settle(Agent::default(), &mut p, &mut velocity, 0.05)
                            && let Some(toward) = navigator.steer(
                                &mut graph,
                                &World,
                                Agent::default(),
                                p,
                                target,
                                0.05,
                                &mut 192,
                            )
                        {
                            p = step(p, toward, 0.05, &[], owner);
                        }
                        assert!(p.y > -8., "resident fell out of den {kind}");
                        if p.distance(target) < 0.5 {
                            break;
                        }
                    }
                    assert!(
                        World.body_clear(p, Agent::default()) && p.distance(target) < 0.5,
                        "resident still trapped: kind={kind} side={side} z={z} at={:?} rock={rock:?}",
                        p - home
                    );
                }
            }
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
    }
    fn den(kind: usize, row: i32) -> (IVec2, Vec3) {
        (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, row);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|home| (cell, home))
            })
            .unwrap()
    }
    fn chase(from: Vec3, target: Vec3) -> Vec3 {
        chase_at_dt(from, target, 1. / 60.)
    }
    fn chase_at_dt(from: Vec3, target: Vec3, dt: f32) -> Vec3 {
        let mut p = from;
        let mut navigator = Navigator::pursuit();
        let mut graph = nav::Graph::default();
        let mut progress = p;
        let mut last_next = None;
        let mut waiting = 0.;
        let mut velocity = 0.;
        for _ in 0..(50. / dt) as usize {
            if !settle(Agent::default(), &mut p, &mut velocity, dt) {
                continue;
            }
            if let Some(next) = navigator.steer(
                &mut graph,
                &World,
                Agent::default(),
                p,
                target,
                dt,
                &mut 192,
            ) {
                last_next = Some(next);
                p = step(p, next, dt, &[], Entity::PLACEHOLDER);
            }
            if p.distance(progress) > 1. {
                progress = p;
                waiting = 0.;
            } else {
                waiting += dt;
            }
            if waiting > 5. {
                break;
            }
            if p.distance(target) < 0.3 {
                return p;
            }
        }
        panic!(
            "failed chase from {from:?} to {target:?}; stopped at {p:?}, {:?}; next {last_next:?}; next walk {:?}; start floors {:?}, target floors {:?}, target clear {}, target slope {}",
            navigator.status,
            last_next.and_then(|q| nav::walk(&World, Agent::default(), p, q.xz())),
            World.floors(p.xz(), Agent::default()),
            World.floors(target.xz(), Agent::default()),
            World.body_clear(target, Agent::default()),
            World.slope(target, Agent::default())
        );
    }
    #[test]
    fn all_den_styles_route_down_and_back_up_without_layer_switches() {
        for kind in 0..3 {
            let (cell, home) = den(kind, 531);
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            let start = home + Vec3::new(0., GRATE_HEIGHT, 4.7);
            let end = home + Vec3::new(3., tunnel_floor(Vec2::new(3., -13.)).unwrap(), -13.);
            let p = chase(start, end);
            chase(p, start);
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
    }
    #[test]
    fn pursuit_recovers_when_lid_support_disappears() {
        let (cell, home) = den(0, 71);
        let mut p = home + Vec3::new(0., GRATE_HEIGHT, 0.);
        let mut navigation = Navigation::default();
        let mut navigator = Navigator::pursuit();
        navigation.refresh();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        navigation.refresh();
        let goal = home + Vec3::new(0., ramp_height(0., -3.), -3.);
        let mut velocity = 0.;
        for _ in 0..1800 {
            if !settle(Agent::default(), &mut p, &mut velocity, 1. / 60.) {
                continue;
            }
            if let Some(next) = navigator.steer(
                &mut navigation.graph,
                &World,
                Agent::default(),
                p,
                goal,
                1. / 60.,
                &mut 192,
            ) {
                p = step(p, next, 1. / 60., &[], Entity::PLACEHOLDER);
            }
            if p.distance(goal) < 0.3 {
                break;
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
        assert!(
            p.distance(goal) < 0.3,
            "stuck after lid opened: {:?}",
            p - home
        );
    }

    #[test]
    #[ignore = "explicit chase performance benchmark"]
    fn barrier_switching_chase_benchmark() {
        let mut graph = nav::Graph::default();
        let mut navigators = std::array::from_fn::<_, 3, _>(|_| Navigator::pursuit());
        let mut actors = std::array::from_fn::<_, 3, _>(|i| {
            (
                Entity::from_raw_u32(i as u32).unwrap(),
                Vec3::new(-6., 0., i as f32 * 0.7),
            )
        });
        let mut travelled = [0.; 3];
        let mut timings = Vec::new();
        for frame in 0..240 {
            let start = std::time::Instant::now();
            let _queries = FrameQueries::begin();
            graph.begin_frame();
            let goal = Vec3::new(if frame / 40 % 2 == 0 { 6. } else { -6. }, 0., -3.);
            let mut budget = 192;
            for i in (frame..frame + 3).map(|i| i % 3) {
                if let Some(next) = navigators[i].steer(
                    &mut graph,
                    &World,
                    Agent::default(),
                    actors[i].1,
                    goal,
                    1. / 60.,
                    &mut budget,
                ) {
                    let before = actors[i].1;
                    actors[i].1 = step(actors[i].1, next, 1. / 60., &actors, actors[i].0);
                    travelled[i] += actors[i].1.distance(before);
                }
            }
            timings.push(start.elapsed().as_secs_f64() * 1000.);
        }
        assert!(
            travelled.iter().all(|d| *d > 1.),
            "benchmark must keep all residents moving: {travelled:?}"
        );
        timings.sort_by(f64::total_cmp);
        eprintln!(
            "chase CPU ms: median={:.3}, p95={:.3}, max={:.3}",
            timings[120], timings[228], timings[239]
        );
    }

    #[test]
    fn shallow_pit_wall_overlap_recovers_in_every_den() {
        for kind in 0..3 {
            let (cell, home) = den(kind, 139);
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            let agent = Agent::default();
            for z in [-3., 0., 3.] {
                let width = if kind == 2 {
                    burrow_width(z) - 0.12
                } else {
                    2.08
                };
                for side in [-1., 1.] {
                    for penetration in [0.01, 0.05, 0.09] {
                        let x = side * (width - agent.radius + penetration);
                        let ramp_x = if kind == 2 {
                            x * 2.2 / burrow_width(z)
                        } else {
                            x
                        };
                        let mut p = home + Vec3::new(x, ramp_height(ramp_x, z), z);
                        let start = p;
                        assert!(
                            !World.body_clear(p, agent),
                            "fixture must overlap: kind={kind} z={z} side={side}"
                        );
                        let mut velocity = 0.;
                        assert!(
                            (0..120).any(|_| settle(agent, &mut p, &mut velocity, 1. / 60.)),
                            "wall recovery failed: kind={kind} z={z} side={side} penetration={penetration}"
                        );
                        assert!(World.body_clear(p, agent));
                        // Recovery is bounded by its three-metre search and
                        // must reach useful floor, not merely a clear bank sliver.
                        assert!(p.xz().distance(start.xz()) <= 3.001);
                        assert!((p.y - start.y).abs() <= 1.2);
                        let at = home.xz() + Vec2::new(0., z);
                        let y = floors::mesh(kind)
                            .rounded_support(at - home.xz(), agent.radius)
                            .unwrap();
                        chase(p, Vec3::new(at.x, y, at.y));
                    }
                }
            }
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
    }

    #[test]
    fn airborne_chase_goal_stays_on_the_floor_below_the_player() {
        let (cell, home) = den(0, 129);
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        for z in [1., -7.] {
            let floor = floors::mesh(0)
                .rounded_support(Vec2::new(0., z), Agent::default().radius)
                .unwrap();
            for lift in [0., 0.1, 0.5, 1.5] {
                let goal = chase_goal(home + Vec3::new(0., floor + lift, z));
                assert!((goal.y - floor).abs() < 0.005, "midair target: {goal:?}");
            }
        }
        // The tunnel's overlying ground remains a distinct reachable floor.
        assert!(chase_goal(home + Vec3::new(0., 2., -7.)).y.abs() < 0.005);
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }

    #[test]
    fn overlapping_residents_separate_and_follow_without_pinning_the_middle() {
        let (_, home) = den(0, 126);
        for dt in [1. / 144., 1. / 30.] {
            for spacing in [0., 0.1, 0.55] {
                let mut actors = std::array::from_fn::<_, 3, _>(|i| {
                    (
                        Entity::from_raw_u32(i as u32).unwrap(),
                        home + Vec3::new(0., 0., 12. + i as f32 * spacing),
                    )
                });
                let goal = home + Vec3::new(0., 0., 22.);
                for frame in 0..(12. / dt) as usize {
                    for i in (frame..frame + 3).map(|i| i % 3) {
                        if actors[i].1.distance(goal) > 1. {
                            actors[i].1 = step(actors[i].1, goal, dt, &actors, actors[i].0);
                        }
                    }
                }
                assert!(
                    actors.iter().all(|(_, p)| p.distance(goal) < 2.3),
                    "crowd pinned: dt={dt} spacing={spacing} actors={:?}",
                    actors.map(|(_, p)| p - home)
                );
                for i in 0..3 {
                    for j in i + 1..3 {
                        assert!(
                            actors[i].1.xz().distance(actors[j].1.xz()) >= 0.55,
                            "residents still overlap: dt={dt} spacing={spacing}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn group_keeps_chasing_when_target_enters_and_exits_repeatedly() {
        for kind in 0..3 {
            let (cell, home) = den(kind, 72);
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            let mut graph = nav::Graph::default();
            let mut navigators = std::array::from_fn::<_, 3, _>(|_| Navigator::pursuit());
            let mut actors = std::array::from_fn::<_, 3, _>(|i| {
                (
                    Entity::from_raw_u32(i as u32).unwrap(),
                    home + Vec3::new((i as f32 - 1.) * 0.65, GRATE_HEIGHT, 4.7),
                )
            });
            let inside = home + Vec3::new(1., tunnel_floor(Vec2::new(1., -10.)).unwrap(), -10.);
            let outside = home + Vec3::new(0., 0., 9.);
            // A stopped attacker can queue two residents at body clearance. Both goals are far enough beyond the mouth that all
            // residents must cross it even when they finish in that queue.
            for goal in [inside, outside, inside, outside] {
                for frame in 0..2400 {
                    let mut budget = 192;
                    for i in (frame..frame + 3).map(|i| i % 3) {
                        if attack_reachable(actors[i].1, goal) {
                            continue;
                        }
                        if let Some(next) = navigators[i].steer(
                            &mut graph,
                            &World,
                            Agent::default(),
                            actors[i].1,
                            goal,
                            1. / 30.,
                            &mut budget,
                        ) {
                            actors[i].1 = step(actors[i].1, next, 1. / 30., &actors, actors[i].0);
                        }
                    }
                    if actors.iter().all(|(_, p)| p.distance(goal) < 3.3) {
                        break;
                    }
                }
                assert!(
                    actors.iter().all(|(_, p)| p.distance(goal) < 3.3),
                    "kind {kind}, goal {:?}, group {:?}",
                    goal - home,
                    actors.map(|(_, p)| p - home)
                );
            }
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
    }

    #[test]
    fn interrupted_emergence_settles_at_every_phase_in_all_dens() {
        for kind in 0..3 {
            let (cell, home) = den(kind, 84);
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            for frame in 0..=450 {
                let start = home + emergence_position(frame as f32 / 60.);
                let mut p = start;
                let mut velocity = 0.;
                let grounded =
                    (0..120).any(|_| settle(Agent::default(), &mut p, &mut velocity, 1. / 60.));
                assert!(
                    grounded,
                    "kind {kind}, frame {frame}, handoff {:?}, stopped {:?}, floors {:?}",
                    start - home,
                    p - home,
                    World.floors(start.xz(), Agent::default())
                );
            }
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
    }

    #[test]
    fn third_den_top_recovers_small_floor_penetrations_and_keeps_chasing() {
        let (cell, home) = den(2, 86);
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        let inside = home + Vec3::new(1., tunnel_floor(Vec2::new(1., -10.)).unwrap(), -10.);
        let outside = home + Vec3::new(0., 0., 9.);
        for x in [-0.35, 0., 0.35] {
            for z in [3., 3.8, 4.3] {
                let xz = home.xz() + Vec2::new(x, z);
                let y = World
                    .floors(xz, Agent::default())
                    .into_iter()
                    .min_by(f32::total_cmp)
                    .unwrap_or_else(|| {
                        base_surface_height(
                            Vec3::new(
                                xz.x,
                                crate::player::movement::PLAYER_EYE_HEIGHT - 0.16,
                                xz.y,
                            ),
                            crate::player::movement::PLAYER_RADIUS,
                        )
                        .unwrap_or(0.)
                    });
                let p = chase(Vec3::new(xz.x, y - 0.015, xz.y), inside);
                chase(p, outside);
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }

    #[test]
    fn overhead_ground_is_a_separate_surface() {
        let (_, home) = den(0, 532);
        let p = home + Vec3::new(0., tunnel_floor(Vec2::new(0., -7.)).unwrap(), -7.);
        let above = p.with_y(0.);
        let floors = World.floors(p.xz(), Agent::default());
        assert!(floors.len() >= 2);
        assert!(!nav::clear(&World, Agent::default(), p, above));
        assert!(!attack_reachable(p, above));
    }
    #[test]
    fn closed_hatch_stops_motion_and_opening_invalidates_tiles() {
        let (cell, home) = den(0, 533);
        let mut navigation = Navigation::default();
        navigation.refresh();
        let revision = navigation.graph.revision;
        let p = home + Vec3::new(0., ramp_height(0., -2.), -2.);
        let target = home + Vec3::new(0., GRATE_HEIGHT, 4.7);
        assert!(!nav::clear(&World, Agent::default(), p, target));
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        navigation.refresh();
        assert!(navigation.graph.revision > revision);
        chase(p, target);
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
        navigation.refresh();
        assert!(navigation.graph.revision > revision + 1);
        assert!(!nav::clear(&World, Agent::default(), p, target));
    }
    #[test]
    fn closed_den_can_be_crossed_on_top() {
        let (_, home) = den(0, 534);
        chase(
            home + Vec3::new(0., GRATE_HEIGHT, 5.),
            home + Vec3::new(0., 0., -6.),
        );
    }
    #[test]
    fn varied_den_approaches_and_ramp_starts_do_not_stall() {
        for row in [41, 42, 43] {
            for kind in 0..3 {
                let (cell, home) = den(kind, row);
                open_gates().write().unwrap().insert(cell);
                gate_openness().write().unwrap().insert(cell, 1.);
                for x in [-0.5, 0., 0.5] {
                    for z in [-3., 0., 3., 5.] {
                        let from =
                            home + Vec3::new(x, if z > 4. { 0. } else { ramp_height(x, z) }, z);
                        let target = home + Vec3::new(3., 0., -7.);
                        chase(from, target);
                    }
                }
                open_gates().write().unwrap().remove(&cell);
                gate_openness().write().unwrap().remove(&cell);
            }
        }
    }

    #[test]
    fn accepted_ramp_edges_can_be_walked_in_small_steps() {
        for kind in 0..3 {
            let (cell, home) = den(kind, 51);
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            let agent = Agent::default();
            for x in [-0.5, 0., 0.5] {
                for zi in -8..=8 {
                    let xz = home.xz() + Vec2::new(x, zi as f32 * 0.5);
                    for y in World.floors(xz, agent) {
                        let a = Vec3::new(xz.x, y, xz.y);
                        for delta in [Vec2::X * 0.5, -Vec2::X * 0.5, Vec2::Y * 0.5, -Vec2::Y * 0.5]
                        {
                            let Some(b) = nav::walk(&World, agent, a, xz + delta) else {
                                continue;
                            };
                            let mut p = a;
                            for _ in 0..200 {
                                let toward = b.xz() - p.xz();
                                if toward.length() < 0.003 {
                                    break;
                                }
                                let to =
                                    p.xz() + toward.normalize() * toward.length().min(1.55 / 240.);
                                let next = nav::walk(&World, agent, p, to);
                                assert!(
                                    next.is_some(),
                                    "kind {kind}, accepted {:?} -> {:?}, stopped {:?}, slope {}",
                                    a - home,
                                    b - home,
                                    p - home,
                                    World.slope(p, agent)
                                );
                                p = next.unwrap();
                            }
                        }
                    }
                }
            }
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
    }
    #[test]
    fn off_center_entries_and_exits_work_at_low_and_high_frame_rates() {
        for kind in 0..3 {
            let (cell, home) = den(kind, 61);
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            for dt in [1. / 30., 1. / 144.] {
                let xz = home.xz() + Vec2::new(-0.5, 0.);
                let y = World
                    .floors(xz, Agent::default())
                    .into_iter()
                    .min_by(f32::total_cmp)
                    .unwrap();
                let inside = Vec3::new(xz.x, y, xz.y);
                let outside = home + Vec3::new(4., 0., -7.);
                let p = chase_at_dt(outside, inside, dt);
                chase_at_dt(p, outside, dt);
            }
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
    }
}
