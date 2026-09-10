//! Shared, tiled, multi-surface navigation. Geometry supplies floors, never route labels.
use bevy::prelude::*;
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, VecDeque},
};

// Quarter-meter sampling keeps body-clear nodes inside narrow passages even
// when their centerline falls between world-grid columns.
pub(crate) const SPACING: f32 = 0.25;
const TILE_CELLS: i32 = 64;
const MAX_TILES: usize = 256;
const MAX_SEARCH_NODES: usize = 65_536;
// Favor useful forward connections over expanding a wide band of nearly
// equivalent quarter-metre nodes. All chosen edges still require full sweeps.
const HEURISTIC_WEIGHT: f32 = 1.15;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Agent {
    pub radius: f32,
    pub height: f32,
    pub step: f32,
    pub slope: f32,
}
impl Default for Agent {
    fn default() -> Self {
        Self {
            radius: 0.28,
            height: 1.6,
            step: 0.20,
            slope: 1.0,
        }
    }
}
/// Return every supporting surface, at any elevation. Body queries must include
/// walls, ceilings and dynamic obstacles, using the same geometry as locomotion.
pub(crate) trait Geometry {
    fn floors(&self, at: Vec2, agent: Agent) -> Vec<f32>;
    fn body_clear(&self, feet: Vec3, agent: Agent) -> bool;
    /// Fractions along a segment where support changes discontinuously. Analytic
    /// geometry supplies its boundaries so sampling cannot skip a ledge.
    fn support_boundaries(&self, _a: Vec2, _b: Vec2, _agent: Agent) -> Vec<f32> {
        Vec::new()
    }
    /// One-sided derivatives keep a discrete stair traversable while rejecting
    /// a continuous steep face, independently of the movement frame duration.
    fn slope(&self, feet: Vec3, agent: Agent) -> f32 {
        let derivative = |axis: Vec2| {
            let side = |sign: f32| {
                self.floors(feet.xz() + axis * sign * 0.125, agent)
                    .into_iter()
                    .min_by(|a, b| (a - feet.y).abs().total_cmp(&(b - feet.y).abs()))
                    .map(|y| (y - feet.y).abs() / 0.125)
                    .unwrap_or(0.)
            };
            side(1.).min(side(-1.))
        };
        Vec2::new(derivative(Vec2::X), derivative(Vec2::Y)).length()
    }
}

/// A steep local normal can belong to a low root or ripple, rather than a
/// hillside. Permit it only when the support across the body footprint fits
/// within one step. This spatial test is shared by graph edges and locomotion;
/// a shorter frame must never turn a sustained steep slope into a staircase.
pub(crate) fn walkable_support(world: &impl Geometry, agent: Agent, feet: Vec3) -> bool {
    if world.slope(feet, agent) <= agent.slope + 0.02 {
        return true;
    }
    let mut low = feet.y;
    let mut high = feet.y;
    let rings = (agent.radius / 0.08).ceil().max(1.) as usize;
    for ring in 1..=rings {
        let radius = agent.radius * ring as f32 / rings as f32;
        for direction in 0..8 {
            let angle = direction as f32 * std::f32::consts::FRAC_PI_4;
            let at = feet.xz() + Vec2::new(angle.cos(), angle.sin()) * radius;
            let Some(y) = world
                .floors(at, agent)
                .into_iter()
                .filter(|y| y.is_finite())
                .min_by(|a, b| (a - feet.y).abs().total_cmp(&(b - feet.y).abs()))
            else {
                return false;
            };
            low = low.min(y);
            high = high.max(y);
            if high - low > agent.step + 0.001 {
                return false;
            }
        }
    }
    true
}

/// Release a shallow horizontal body overlap without crossing a solid wall or
/// choosing another floor. A smaller capsule must already fit at the start;
/// expand it back to full size along a short, swept correction.
#[derive(Default)]
pub(crate) struct Recovery {
    origin: Option<(Vec3, Agent)>,
    cursor: usize,
    cooldown: f32,
}
impl Recovery {
    fn recover(
        &mut self,
        world: &impl Geometry,
        agent: Agent,
        feet: &mut Vec3,
        dt: f32,
        attempts: &mut usize,
    ) -> bool {
        if self.origin != Some((*feet, agent)) {
            *self = Self {
                origin: Some((*feet, agent)),
                ..default()
            };
        }
        self.cooldown -= dt;
        if self.cooldown > 0. || *attempts == 0 {
            return false;
        }
        let allowance = agent.radius.min(0.12);
        let small = Agent {
            radius: (agent.radius - allowance).max(0.01),
            ..agent
        };
        if !world.body_clear(*feet, small) {
            return false;
        }
        let start = *feet;
        while self.cursor < 6 * 16 && *attempts > 0 {
            let ring = 1 + self.cursor / 16;
            let direction = self.cursor % 16;
            self.cursor += 1;
            *attempts -= 1;
            let distance = allowance * ring as f32 / 6.;
            let angle = direction as f32 * std::f32::consts::TAU / 16.;
            let at = start.xz() + Vec2::new(angle.cos(), angle.sin()) * distance;
            let Some(y) = world
                .floors(at, agent)
                .into_iter()
                .filter(|y| y.is_finite() && (y - start.y).abs() <= agent.step + 0.001)
                .min_by(|a, b| (a - start.y).abs().total_cmp(&(b - start.y).abs()))
            else {
                continue;
            };
            let end = Vec3::new(at.x, y, at.y);
            if !world.body_clear(end, agent) {
                continue;
            }
            let samples = (start.distance(end) / 0.02).ceil().max(1.) as usize;
            if (0..=samples).all(|i| {
                let t = i as f32 / samples as f32;
                let probe = start.lerp(end, t);
                let capsule = Agent {
                    radius: small.radius + (agent.radius - small.radius) * t,
                    ..agent
                };
                world.body_clear(probe, capsule)
                    && world
                        .floors(probe.xz(), capsule)
                        .iter()
                        .any(|y| (y - probe.y).abs() <= agent.step + 0.001)
            }) {
                *feet = end;
                *self = Self::default();
                return true;
            }
        }
        if self.cursor == 6 * 16 {
            self.cursor = 0;
            self.cooldown = 0.5;
        }
        false
    }
}

/// Fall onto the next supporting surface when moving geometry removes support.
/// Sweep the body so recovery cannot teleport through floors or obstacles.
pub(crate) fn settle(
    world: &impl Geometry,
    agent: Agent,
    feet: &mut Vec3,
    velocity: &mut f32,
    dt: f32,
) -> bool {
    let mut attempts = usize::MAX;
    settle_with_recovery(
        world,
        agent,
        feet,
        velocity,
        dt,
        &mut Recovery::default(),
        &mut attempts,
    )
}

/// Resume overlap recovery without repeating the failed prefix every tick.
/// The caller shares a probe allowance across actors; accepted corrections still
/// perform the original expanding-capsule and supporting-floor sweeps.
pub(crate) fn settle_with_recovery(
    world: &impl Geometry,
    agent: Agent,
    feet: &mut Vec3,
    velocity: &mut f32,
    dt: f32,
    recovery: &mut Recovery,
    attempts: &mut usize,
) -> bool {
    if !world.body_clear(*feet, agent) && !recovery.recover(world, agent, feet, dt, attempts) {
        *velocity = 0.;
        return false;
    }
    *recovery = Recovery::default();
    let floors: Vec<_> = world
        .floors(feet.xz(), agent)
        .into_iter()
        .filter(|y| y.is_finite())
        .collect();
    // Scripted motion and world-coordinate rounding can leave feet slightly
    // inside their support. Recover to the nearest local floor within the
    // normal step allowance, rather than falling through that floor forever.
    let support = floors
        .iter()
        .copied()
        .filter(|y| (y - feet.y).abs() <= agent.step + 0.001)
        .min_by(|a, b| (a - feet.y).abs().total_cmp(&(b - feet.y).abs()));
    if let Some(y) = support.filter(|y| feet.y - y <= 0.005) {
        *velocity = 0.;
        let start = *feet;
        let samples = ((y - start.y).abs() / 0.025).ceil().max(1.) as usize;
        for i in 1..=samples {
            if !world.body_clear(
                start.with_y(start.y + (y - start.y) * i as f32 / samples as f32),
                agent,
            ) {
                return false;
            }
        }
        feet.y = y;
        return true;
    }
    let floor = crate::world::floor::support(floors, feet.y, 0.);
    *velocity -= 9.81 * dt;
    let end = (feet.y + *velocity * dt).max(floor.unwrap_or(f32::NEG_INFINITY));
    let start = *feet;
    let samples = ((start.y - end).abs() / 0.08).ceil().max(1.) as usize;
    for i in 1..=samples {
        let next = start.with_y(start.y + (end - start.y) * i as f32 / samples as f32);
        if !world.body_clear(next, agent) {
            *velocity = 0.;
            return false;
        }
        *feet = next;
    }
    let grounded = floor.is_some_and(|y| (feet.y - y).abs() <= 0.005);
    if grounded {
        *velocity = 0.;
    }
    grounded
}

/// Follow continuous support, checking the whole edge rather than just endpoints.
/// Nearest-height selection is local continuity, never a global surface/cave test.
pub(crate) fn walk(world: &impl Geometry, agent: Agent, from: Vec3, to: Vec2) -> Option<Vec3> {
    if !from.is_finite() || !to.is_finite() {
        return None;
    }
    if !world.body_clear(from, agent) {
        return None;
    }
    let length = from.xz().distance(to);
    let count = (length / 0.08).ceil().max(1.) as usize;
    let mut samples: Vec<f32> = (1..=count).map(|i| i as f32 / count as f32).collect();
    let mut boundaries = world.support_boundaries(from.xz(), to, agent);
    boundaries.retain(|t| t.is_finite() && *t > 0. && *t < 1.);
    boundaries.extend([0., 1.]);
    boundaries.sort_by(f32::total_cmp);
    boundaries.dedup();
    // A rounded passage can be narrowest at the boundary itself. Testing only
    // either side allows a long edge to skip a bank that small steps cannot.
    samples.extend(boundaries.iter().copied());
    for window in boundaries.windows(2) {
        let margin = (0.01 / length.max(0.01)).min((window[1] - window[0]) * 0.25);
        samples.extend([window[0] + margin, window[1] - margin]);
    }
    samples.sort_by(f32::total_cmp);
    samples.dedup();
    let mut p = from;
    let mut escaping_steep = false;
    for t in samples {
        let xz = from.xz().lerp(to, t);
        let y = world
            .floors(xz, agent)
            .into_iter()
            .filter(|y| {
                y.is_finite()
                    && (*y - p.y).abs() <= agent.step + 0.001
                    && world.body_clear(Vec3::new(xz.x, *y, xz.y), agent)
            })
            .min_by(|a, b| (a - p.y).abs().total_cmp(&(b - p.y).abs()))?;
        // Bound discontinuous steps locally; surface gradients handle slopes.
        // An overall rise cap would incorrectly reject a slope followed by a step.
        let next = Vec3::new(xz.x, y, xz.y);
        if !walkable_support(world, agent, next) {
            // A body already perched on a steep bank must be able to get off
            // it. Permit only a continuously descending escape until it reaches
            // walkable support; this never permits entering/climbing steep ground.
            if t == 0. {
                escaping_steep = true;
            } else if !escaping_steep {
                return None;
            } else if y >= p.y - 0.00001 {
                // At a rounded crest, tiny frames can quantize the first drop
                // to zero. Verify downhill direction at a fixed spatial scale
                // instead of making escape depend on frame duration.
                let ahead = xz + (to - from.xz()).normalize_or_zero() * 0.08;
                let downhill = world
                    .floors(ahead, agent)
                    .into_iter()
                    .filter(|h| h.is_finite())
                    .min_by(|a, b| (a - y).abs().total_cmp(&(b - y).abs()))
                    .is_some_and(|h| h < y - 0.00001 && y - h <= agent.step + 0.001);
                if y > p.y + 0.00001 || !downhill {
                    return None;
                }
            }
        } else {
            escaping_steep = false;
        }
        // Probe the intervening body at the higher floor across a step.
        if y > p.y && !world.body_clear(p.with_y(y), agent) {
            return None;
        }
        p = next;
    }
    Some(p)
}
pub(crate) fn clear(world: &impl Geometry, agent: Agent, a: Vec3, b: Vec3) -> bool {
    if a.xz().distance(b.xz()) > 16. {
        return false;
    }
    // Graph edges must end on the surface named by their destination. Step
    // height is a traversal limit, not permission to finish on a different floor.
    walk(world, agent, a, b.xz()).is_some_and(|p| (p.y - b.y).abs() <= 0.005)
}

/// Slide a rejected local step along either horizontal axis. Each candidate
/// retains the full supported body sweep; no position is nudged through a wall.
/// Normal movement pays for one sweep, blocked movement for at most three.
/// Acceptance lets crowd reservations participate in the same choice.
pub(crate) fn slide(
    world: &impl Geometry,
    agent: Agent,
    from: Vec3,
    to: Vec2,
    accept: impl Fn(Vec3) -> bool,
) -> Option<Vec3> {
    let delta = to - from.xz();
    let axes = if delta.x.abs() >= delta.y.abs() {
        [Vec2::new(to.x, from.z), Vec2::new(from.x, to.y)]
    } else {
        [Vec2::new(from.x, to.y), Vec2::new(to.x, from.z)]
    };
    let candidates = [to, axes[0], axes[1]];
    for (index, at) in candidates.into_iter().enumerate() {
        if !candidates[..index].contains(&at)
            && at != from.xz()
            && let Some(p) = walk(world, agent, from, at)
            && accept(p)
        {
            return Some(p);
        }
    }
    None
}

/// A shortcut follows the supported floor, not the straight 3-D chord between
/// its endpoints. The swept walk already checks every rise, ledge and body
/// clearance; requiring a planar floor rejected ordinary hills and valleys.
fn shortcut(world: &impl Geometry, agent: Agent, a: Vec3, b: Vec3) -> bool {
    clear(world, agent, a, b)
}

/// Normalize an actor's small foot offset once, at the destination boundary.
/// Internal nodes and corridor connections always retain exact support heights.
fn grounded_goal(world: &impl Geometry, agent: Agent, goal: Vec3) -> Vec3 {
    world
        .floors(goal.xz(), agent)
        .into_iter()
        .filter(|y| y.is_finite() && (y - goal.y).abs() <= agent.step + 0.001)
        .map(|y| goal.with_y(y))
        .filter(|p| world.body_clear(*p, agent))
        .min_by(|a, b| (a.y - goal.y).abs().total_cmp(&(b.y - goal.y).abs()))
        .unwrap_or(goal)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Node {
    x: i32,
    z: i32,
    height: u32,
}
impl Node {
    fn new(cell: IVec2, y: f32) -> Self {
        Self {
            x: cell.x,
            z: cell.y,
            height: y.to_bits(),
        }
    }
    fn cell(self) -> IVec2 {
        IVec2::new(self.x, self.z)
    }
    fn position(self) -> Vec3 {
        Vec3::new(
            self.x as f32 * SPACING,
            f32::from_bits(self.height),
            self.z as f32 * SPACING,
        )
    }
}
#[derive(Default)]
struct Tile {
    columns: HashMap<(IVec2, [u32; 4]), Vec<Node>>,
    touched: u64,
}
#[derive(Default)]
pub(crate) struct Graph {
    tiles: HashMap<IVec2, Tile>,
    clock: u64,
    edges: HashMap<(Node, Node, [u32; 4], bool), bool>,
    probes: HashMap<(Node, Node, [u32; 4], bool), Vec3>,
    pub revision: u64,
    deadline: Option<std::time::Instant>,
}
impl Graph {
    /// Give navigation tests a fixed one-millisecond planning allowance.
    #[cfg(test)]
    pub fn begin_frame(&mut self) {
        self.begin_slice(std::time::Duration::from_millis(1));
    }
    /// Resume a shared planning allowance after physics/animation work. Those
    /// systems must not consume the time reserved for a resident's search.
    pub fn begin_slice(&mut self, remaining: std::time::Duration) {
        self.deadline = Some(std::time::Instant::now() + remaining);
    }
    fn out_of_time(&self) -> bool {
        self.deadline
            .is_some_and(|end| std::time::Instant::now() >= end)
    }
    pub fn invalidate(&mut self, min: Vec2, max: Vec2) {
        let size = SPACING * TILE_CELLS as f32;
        let a = (min / size).floor().as_ivec2();
        let b = (max / size).floor().as_ivec2();
        self.tiles
            .retain(|key, _| key.x < a.x || key.y < a.y || key.x > b.x || key.y > b.y);
        self.edges.clear();
        self.probes.clear();
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn reset(&mut self) {
        self.tiles.clear();
        self.edges.clear();
        self.probes.clear();
        self.revision = self.revision.wrapping_add(1);
    }
    fn edge_probe(
        &mut self,
        world: &impl Geometry,
        agent: Agent,
        from: Node,
        to: Node,
        coarse: bool,
    ) -> Option<bool> {
        let key = (
            from,
            to,
            [
                agent.radius.to_bits(),
                agent.height.to_bits(),
                agent.step.to_bits(),
                agent.slope.to_bits(),
            ],
            coarse,
        );
        if let Some(clear) = self.edges.get(&key) {
            return Some(*clear);
        }
        let end = to.position();
        let mut p = self.probes.get(&key).copied().unwrap_or(from.position());
        let valid = loop {
            if self.out_of_time() {
                self.probes.insert(key, p);
                return None;
            }
            if from.position().xz().distance(end.xz()) > 16. {
                break false;
            }
            let delta = end.xz() - p.xz();
            // Yield during long edges, not only after an entire 16 m sweep.
            let at = if delta.length() <= 0.25 {
                end.xz()
            } else {
                p.xz() + delta.normalize() * 0.25
            };
            let Some(next) = walk(world, agent, p, at) else {
                break false;
            };
            p = next;
            if at == end.xz() {
                break (p.y - end.y).abs() <= 0.005;
            }
        };
        self.probes.remove(&key);
        if self.edges.len() >= 65_536 {
            self.edges.clear();
        }
        self.edges.insert(key, valid);
        Some(valid)
    }
    #[cfg(test)]
    fn edge(
        &mut self,
        world: &impl Geometry,
        agent: Agent,
        from: Node,
        to: Node,
        coarse: bool,
    ) -> bool {
        self.edge_probe(world, agent, from, to, coarse)
            .unwrap_or(false)
    }
    fn nodes(&mut self, world: &impl Geometry, agent: Agent, cell: IVec2) -> Vec<Node> {
        let key = IVec2::new(cell.x.div_euclid(TILE_CELLS), cell.y.div_euclid(TILE_CELLS));
        self.clock += 1;
        if !self.tiles.contains_key(&key) && self.tiles.len() >= MAX_TILES {
            let oldest = *self.tiles.iter().min_by_key(|(_, t)| t.touched).unwrap().0;
            self.tiles.remove(&oldest);
        }
        let tile = self.tiles.entry(key).or_default();
        tile.touched = self.clock;
        tile.columns
            .entry((
                cell,
                [
                    agent.radius.to_bits(),
                    agent.height.to_bits(),
                    agent.step.to_bits(),
                    agent.slope.to_bits(),
                ],
            ))
            .or_insert_with(|| {
                let mut heights = world.floors(cell.as_vec2() * SPACING, agent);
                heights.retain(|y| y.is_finite());
                heights.sort_by(f32::total_cmp);
                heights.dedup();
                heights
                    .into_iter()
                    .map(|y| Node::new(cell, y))
                    .filter(|n| world.body_clear(n.position(), agent))
                    .collect()
            })
            .clone()
    }
}
#[derive(Clone, Copy)]
struct Open {
    node: Node,
    cost: f32,
    score: f32,
}
impl PartialEq for Open {
    fn eq(&self, b: &Self) -> bool {
        self.score == b.score && self.node == b.node
    }
}
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Open {
    fn cmp(&self, b: &Self) -> Ordering {
        b.score
            .total_cmp(&self.score)
            .then_with(|| self.node.cmp(&b.node))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Searching,
    Complete,
    Unreachable,
    BudgetExceeded,
    Invalidated,
}
struct SearchStart {
    from: Vec3,
    column: usize,
    nodes: VecDeque<Node>,
}
pub(crate) struct Search {
    start: Option<SearchStart>,
    open: BinaryHeap<Open>,
    pending: Option<(Open, usize)>,
    costs: HashMap<Node, f32>,
    parents: HashMap<Node, Node>,
    goal: Vec3,
    revision: u64,
    agent: Agent,
}
impl Search {
    pub fn new(
        graph: &mut Graph,
        world: &impl Geometry,
        agent: Agent,
        from: Vec3,
        goal: Vec3,
    ) -> Self {
        let goal = grounded_goal(world, agent, goal);
        Self {
            start: Some(SearchStart {
                from,
                column: 0,
                nodes: default(),
            }),
            open: BinaryHeap::new(),
            pending: None,
            costs: HashMap::new(),
            parents: HashMap::new(),
            goal,
            revision: graph.revision,
            agent,
        }
    }
    pub fn advance(
        &mut self,
        graph: &mut Graph,
        world: &impl Geometry,
        agent: Agent,
        budget: &mut usize,
    ) -> (Status, VecDeque<Vec3>) {
        if self.revision != graph.revision || self.agent != agent {
            return (Status::Invalidated, VecDeque::new());
        }
        // Initial attachment used to trace all nine neighboring columns in
        // Search::new, outside the deadline. Retain the column and floor cursor
        // and yield between these short (at most 0.54m) supported connections.
        while let Some(start) = self.start.as_mut() {
            if start.column == 9 && start.nodes.is_empty() {
                self.start = None;
                break;
            }
            if *budget == 0 || graph.out_of_time() {
                return (Status::Searching, VecDeque::new());
            }
            if let Some(node) = start.nodes.pop_front() {
                *budget -= 1;
                let p = node.position();
                if clear(world, agent, start.from, p) {
                    let cost = start.from.distance(p);
                    self.costs.insert(node, cost);
                    self.open.push(Open {
                        node,
                        cost,
                        score: cost + p.distance(self.goal) * HEURISTIC_WEIGHT,
                    });
                }
            } else if start.column < 9 {
                let center = (start.from.xz() / SPACING).round().as_ivec2();
                let delta =
                    IVec2::new((start.column / 3) as i32 - 1, (start.column % 3) as i32 - 1);
                start.column += 1;
                *budget -= 1;
                start.nodes = graph.nodes(world, agent, center + delta).into();
            } else {
                self.start = None;
            }
        }
        let mut slice = 48;
        while *budget > 0 && slice > 0 && !graph.out_of_time() {
            let (open, first_edge) = if let Some(pending) = self.pending.take() {
                pending
            } else if let Some(open) = self.open.pop() {
                (open, 0)
            } else {
                return (Status::Unreachable, VecDeque::new());
            };
            *budget -= 1;
            slice -= 1;
            if open.cost > self.costs[&open.node] + 0.0001 {
                continue;
            }
            let p = open.node.position();
            if p.distance(self.goal) < SPACING * 1.6 && clear(world, agent, p, self.goal) {
                let mut path = VecDeque::from([self.goal]);
                let mut node = open.node;
                path.push_front(node.position());
                while let Some(parent) = self.parents.get(&node) {
                    node = *parent;
                    path.push_front(node.position());
                }
                return (Status::Complete, path);
            }
            if self.costs.len() >= MAX_SEARCH_NODES {
                return (Status::BudgetExceeded, VecDeque::new());
            }
            // Coarse lattice junctions add verified 4 m and 16 m connections.
            // Fine edges always remain available around obstacles and ramps.
            for edge in first_edge..27 {
                // A coarse junction can have several costly long edges. Yield
                // between edges and retain the exact cursor, not just between
                // whole nodes, so a junction cannot consume the entire frame.
                if graph.out_of_time() {
                    self.pending = Some((open, edge));
                    return (Status::Searching, VecDeque::new());
                }
                let stride = [1, 16, 64][edge / 9];
                let x = (edge % 9 / 3) as i32 - 1;
                let z = (edge % 3) as i32 - 1;
                if (x == 0 && z == 0)
                    || open.node.x.rem_euclid(stride) != 0
                    || open.node.z.rem_euclid(stride) != 0
                {
                    continue;
                }
                for next in graph.nodes(world, agent, open.node.cell() + IVec2::new(x, z) * stride)
                {
                    let q = next.position();
                    let cost = open.cost + p.distance(q);
                    if (p.y - q.y).abs()
                        > agent.step + p.xz().distance(q.xz()) * agent.slope + 0.001
                        || cost >= *self.costs.get(&next).unwrap_or(&f32::INFINITY)
                    {
                        continue;
                    }
                    match graph.edge_probe(world, agent, open.node, next, stride != 1) {
                        Some(true) => {}
                        Some(false) => continue,
                        None => {
                            self.pending = Some((open, edge));
                            return (Status::Searching, VecDeque::new());
                        }
                    }
                    self.costs.insert(next, cost);
                    self.parents.insert(next, open.node);
                    self.open.push(Open {
                        node: next,
                        cost,
                        score: cost + q.distance(self.goal) * HEURISTIC_WEIGHT,
                    });
                }
            }
        }
        (Status::Searching, VecDeque::new())
    }
}

pub(crate) struct Navigator {
    path: VecDeque<Vec3>,
    search: Option<Search>,
    goal: Option<Vec3>,
    revision: u64,
    retry: f32,
    smoothing_retry: f32,
    stuck: f32,
    previous: Option<(Vec3, f32)>,
    ready: Option<VecDeque<Vec3>>,
    agent: Option<Agent>,
    pub status: Status,
    forward_progress: bool,
    same_level_prefix: bool,
    prefix: bool,
}
impl Default for Navigator {
    fn default() -> Self {
        Self {
            path: VecDeque::new(),
            search: None,
            goal: None,
            revision: 0,
            retry: 0.,
            smoothing_retry: 0.,
            stuck: 0.,
            previous: None,
            ready: None,
            agent: None,
            status: Status::Searching,
            forward_progress: false,
            same_level_prefix: false,
            prefix: false,
        }
    }
}
impl Navigator {
    pub fn pursuit() -> Self {
        Self {
            forward_progress: true,
            ..Self::default()
        }
    }
    /// A floor above/below a resident must not disable short supported motion
    /// along its current floor. A height-changing goal still uses full search.
    pub fn layered_pursuit() -> Self {
        Self {
            same_level_prefix: true,
            ..Self::pursuit()
        }
    }
    // Keep agent, world, timing and search budget explicit at the routing boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn steer(
        &mut self,
        graph: &mut Graph,
        world: &impl Geometry,
        agent: Agent,
        p: Vec3,
        goal: Vec3,
        dt: f32,
        budget: &mut usize,
    ) -> Option<Vec3> {
        self.retry -= dt;
        self.smoothing_retry -= dt;
        // The shared deadline covers planning and smoothing as well as A*.
        // Continue following a valid existing route until this actor's next
        // planning slice; movement independently sweeps the actual body.
        if graph.out_of_time() {
            return (self.revision == graph.revision && self.agent == Some(agent))
                .then(|| self.path.front().copied())
                .flatten();
        }
        let goal = grounded_goal(world, agent, goal);
        // Progress is a new best distance to the current waypoint. Mere
        // displacement lets an actor oscillate forever between two positions.
        if let Some(waypoint) = self.path.front().copied() {
            let distance = p.distance(waypoint);
            if self
                .previous
                .is_none_or(|(old, best)| old != waypoint || distance < best - 0.04)
            {
                self.previous = Some((waypoint, distance));
                self.stuck = 0.;
            } else {
                self.stuck += dt;
            }
        }
        let shortcut_limit = if self.forward_progress { 1. } else { 16. };
        let direct = |a: Vec3, b: Vec3| {
            a.xz().distance(b.xz()) <= shortcut_limit && shortcut(world, agent, a, b)
        };
        let geometry_changed = self.revision != graph.revision || self.agent != Some(agent);
        // A player returning to reachable ground must not wait for an old
        // underground query to exhaust its search. Keep expensive searches
        // running for moving targets unless a usable replacement is proven
        // or the old destination has no valid body clearance/support. A queue
        // slot inside an obstacle must not pin an actor after its slot changes.
        let direct_retarget = self.search.is_some()
            && self.goal.is_some_and(|old| {
                old.distance(goal) > 0.6
                    && (!world.body_clear(old, agent)
                        || !world
                            .floors(old.xz(), agent)
                            .iter()
                            .any(|y| (y - old.y).abs() < 0.005)
                        || direct(p, goal))
            });
        // A moving goal must not continually cancel an unfinished query.
        let changed = geometry_changed
            || direct_retarget
            || (self.search.is_none()
                && self.ready.is_none()
                && self.goal.is_none_or(|old| old.distance(goal) > 0.6));
        // Movement owns swept collision and avoidance. Do not suppress its
        // waypoint because a new centerline trace from a rounded intermediate
        // position fails; it may need a sideways step to follow the corridor.
        // Geometry revisions and actual lack of progress still trigger replans.
        let blocked = !world.body_clear(p, agent);
        if changed
            || blocked
            || (self.search.is_none()
                && self.retry <= 0.
                && (self.path.is_empty() || self.stuck > 0.8))
        {
            let use_prefix = self.forward_progress
                && self.stuck <= 0.8
                && (self.path.is_empty() || self.prefix || geometry_changed);
            if geometry_changed || blocked {
                self.path.clear();
            }
            self.agent = Some(agent);
            self.search = None;
            self.ready = None;
            self.goal = Some(goal);
            self.revision = graph.revision;
            self.stuck = 0.;
            self.retry = 0.7;
            let same_level = self.same_level_prefix && (p.y - goal.y).abs() <= agent.step + 0.001;
            let forward = if use_prefix
                && p.xz().distance(goal.xz()) > 1.
                && (same_level
                    || (world.floors(p.xz(), agent).len() == 1
                        && world.floors(goal.xz(), agent).len() == 1))
            {
                // Begin pursuit on a verified one-metre corridor.
                // Revalidate at its end; a blocked corridor invokes the full
                // planner. Never replace an active detour with a greedy step.
                walk(
                    world,
                    agent,
                    p,
                    p.xz() + (goal.xz() - p.xz()).normalize_or_zero(),
                )
                .filter(|next| !same_level || (next.y - goal.y).abs() <= agent.step + 0.001)
            } else {
                None
            };
            self.prefix = forward.is_some();
            if let Some(next) = forward.or_else(|| direct(p, goal).then_some(goal)) {
                self.path.clear();
                self.path.push_back(next);
                self.status = Status::Complete;
            } else {
                self.search = Some(Search::new(graph, world, agent, p, goal));
                self.status = Status::Searching;
            }
        }
        if let Some(search) = &mut self.search {
            let (status, path) = search.advance(graph, world, agent, budget);
            self.status = status;
            if status != Status::Searching {
                self.prefix = false;
                self.ready = (status == Status::Complete).then_some(path);
                if status != Status::Complete {
                    self.path.clear();
                }
                self.search = None;
                self.retry = 1.5;
            }
        }
        // A completed search begins at a historical pose. Keep following the
        // existing corridor until there is time to verify an attachment from
        // the current pose; never install the untrimmed historical prefix.
        if !graph.out_of_time()
            && let Some(path) = &self.ready
        {
            let nearest = path
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
                .map(|(i, _)| i);
            if let Some(nearest) = nearest {
                let mut joined = None;
                for index in (nearest..(nearest + 6).min(path.len())).rev() {
                    if graph.out_of_time() {
                        break;
                    }
                    if direct(p, path[index]) {
                        joined = Some(index);
                        break;
                    }
                }
                if let Some(index) = joined {
                    self.path = self.ready.take().unwrap();
                    self.path.drain(..index);
                    self.smoothing_retry = 0.;
                    self.previous = None;
                } else if !graph.out_of_time() {
                    // The actor left this corridor while it was being built.
                    self.ready = None;
                    self.path.clear();
                    self.retry = 0.;
                }
            } else {
                self.ready = None;
            }
        }
        // A nearby waypoint may be the only safe corner around a wall, lip or
        // change in floor height. Proximity alone does not permit skipping it.
        while self.path.front().is_some_and(|q| {
            q.xz().distance(p.xz()) < if self.path.len() == 1 { 0.15 } else { 0.02 }
                && (q.y - p.y).abs() <= agent.step + 0.001
                && (self.path.len() == 1 || clear(world, agent, p, self.path[1]))
        }) {
            self.path.pop_front();
            if self.prefix && self.path.is_empty() {
                self.retry = 0.;
            }
            self.smoothing_retry = 0.;
        }
        // A verified corridor remains usable while the actor follows it. Retry
        // optional corner cutting at most every 150 ms, and immediately on a
        // new route/waypoint. Movement still sweeps every actual step.
        if self.smoothing_retry <= 0. && self.path.len() > 1 {
            self.smoothing_retry = 0.15;
            for i in (1..self.path.len().min(6)).rev() {
                if graph.out_of_time() {
                    break;
                }
                if direct(p, self.path[i]) {
                    self.path.drain(..i);
                    break;
                }
            }
        }
        self.path.front().copied().filter(|_| !blocked)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod traversal_tests;

#[cfg(test)]
mod moving_target_tests;

#[cfg(test)]
mod waypoint_tests;

#[cfg(test)]
mod boundary_tests;

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::cell::Cell;
    struct Wall;
    impl Geometry for Wall {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0.]
        }
        fn body_clear(&self, p: Vec3, a: Agent) -> bool {
            p.x >= a.radius
        }
    }
    #[test]
    fn sliced_recovery_resumes_to_the_same_swept_correction() {
        let agent = Agent::default();
        let mut reference = Vec3::new(0.18, 0., 0.);
        assert!(settle(&Wall, agent, &mut reference, &mut 0., 1. / 30.));
        let mut p = Vec3::new(0.18, 0., 0.);
        let mut recovery = Recovery::default();
        let mut landed = false;
        for tick in 0..12 {
            let mut budget = 8;
            landed = settle_with_recovery(
                &Wall,
                agent,
                &mut p,
                &mut 0.,
                1. / 30.,
                &mut recovery,
                &mut budget,
            );
            assert!(budget <= 8);
            if tick == 0 {
                assert!(!landed);
            }
            if landed {
                break;
            }
            assert_eq!(p, Vec3::new(0.18, 0., 0.));
        }
        assert!(landed);
        assert_eq!(p, reference);
    }
    struct Blocked {
        floors: Cell<usize>,
        open: Cell<bool>,
    }
    impl Geometry for Blocked {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            self.floors.set(self.floors.get() + 1);
            vec![0.]
        }
        fn body_clear(&self, _: Vec3, a: Agent) -> bool {
            self.open.get() || a.radius < 0.2
        }
    }
    #[test]
    fn exhausted_recovery_backs_off_but_a_removed_obstacle_releases_immediately() {
        let world = Blocked {
            floors: Cell::new(0),
            open: Cell::new(false),
        };
        let mut p = Vec3::ZERO;
        let mut recovery = Recovery::default();
        for _ in 0..12 {
            assert!(!settle_with_recovery(
                &world,
                Agent::default(),
                &mut p,
                &mut 0.,
                1. / 30.,
                &mut recovery,
                &mut 8
            ));
        }
        assert_eq!(world.floors.get(), 96);
        assert!(!settle_with_recovery(
            &world,
            Agent::default(),
            &mut p,
            &mut 0.,
            1. / 30.,
            &mut recovery,
            &mut 8
        ));
        assert_eq!(world.floors.get(), 96);
        world.open.set(true);
        assert!(settle_with_recovery(
            &world,
            Agent::default(),
            &mut p,
            &mut 0.,
            1. / 30.,
            &mut recovery,
            &mut 8
        ));
        assert!(recovery.origin.is_none());
    }
    #[test]
    fn relocation_restarts_recovery_and_a_shared_budget_bounds_all_actors() {
        let world = Blocked {
            floors: Cell::new(0),
            open: Cell::new(false),
        };
        let mut a = Recovery::default();
        let mut b = Recovery::default();
        let mut budget = 8;
        let mut first_position = Vec3::ZERO;
        let mut second_position = Vec3::ZERO;
        let mut relocated = Vec3::X;
        assert!(!settle_with_recovery(
            &world,
            Agent::default(),
            &mut first_position,
            &mut 0.,
            1. / 30.,
            &mut a,
            &mut budget
        ));
        assert!(!settle_with_recovery(
            &world,
            Agent::default(),
            &mut second_position,
            &mut 0.,
            1. / 30.,
            &mut b,
            &mut budget
        ));
        assert_eq!(world.floors.get(), 8);
        assert_eq!(a.cursor, 8);
        assert_eq!(b.cursor, 0);
        assert!(!settle_with_recovery(
            &world,
            Agent::default(),
            &mut relocated,
            &mut 0.,
            1. / 30.,
            &mut a,
            &mut 1
        ));
        assert_eq!(a.cursor, 1);
    }
}
