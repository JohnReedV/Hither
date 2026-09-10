//! Static floor connectivity shared by recovery for every site of each style.
//! Clear space alone can be an isolated pocket behind a bank rock. Flood the
//! actual supported movement graph from the mouth before accepting such space.
use super::*;
use crate::world::navigation::{self as nav, Agent, Geometry as _};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct StaticWorld {
    kind: usize,
    home: Vec3,
}
impl nav::Geometry for StaticWorld {
    fn floors(&self, at: Vec2, agent: Agent) -> Vec<f32> {
        floors::mesh(self.kind)
            .rounded_support(at - self.home.xz(), agent.radius)
            .into_iter()
            .collect()
    }
    fn support_boundaries(&self, a: Vec2, b: Vec2, agent: Agent) -> Vec<f32> {
        // Match runtime sweeps at thin rock corners, aperture edges and lips;
        // uniform sampling alone can skip an otherwise blocking boundary.
        navigation::den_boundaries(a, b, agent, self.kind, self.home)
    }
    fn body_clear(&self, feet: Vec3, agent: Agent) -> bool {
        !floors::body_blocks(feet - self.home, agent.radius, self.kind, true)
    }
}
struct Region {
    connected: HashMap<IVec2, Vec3>,
    world: StaticWorld,
    links: HashMap<IVec2, Vec<IVec2>>,
    route: Mutex<Option<(IVec2, HashMap<IVec2, IVec2>)>>,
}
impl Region {
    fn build(kind: usize, home: Vec3) -> Self {
        let world = StaticWorld { kind, home };
        let agent = Agent::default();
        let mut connected = HashMap::new();
        let mut pending = VecDeque::new();
        // The render mesh's open front edge joins the outdoor ground. Seed all
        // body-clear quarter-metre samples on its first complete interior row.
        let (low, high) = floors::mesh(kind).bounds();
        let (low, high) = (low + home.xz(), high + home.xz());
        let front = (high.y / nav::SPACING).floor() as i32 - 1;
        for x in (low.x / nav::SPACING).floor() as i32..=(high.x / nav::SPACING).ceil() as i32 {
            let key = IVec2::new(x, front);
            let at = key.as_vec2() * nav::SPACING;
            if let Some(y) = world.floors(at, agent).first() {
                let feet = Vec3::new(at.x, *y, at.y);
                if world.body_clear(feet, agent) {
                    connected.insert(key, feet);
                    pending.push_back(key);
                }
            }
        }
        while let Some(key) = pending.pop_front() {
            let from = connected[&key];
            for x in -1..=1 {
                for z in -1..=1 {
                    if x == 0 && z == 0 {
                        continue;
                    }
                    let next = key + IVec2::new(x, z);
                    if connected.contains_key(&next) {
                        continue;
                    }
                    let at = next.as_vec2() * nav::SPACING;
                    // Missing rendered support terminates the flood naturally.
                    if let Some(feet) = nav::walk(&world, agent, from, at)
                        && nav::walk(&world, agent, feet, from.xz()).is_some()
                    {
                        connected.insert(next, feet);
                        pending.push_back(next);
                    }
                }
            }
        }
        // Store every verified connection, including links between the flood's
        // mouth seeds. Searches inside a den then use this small static graph
        // instead of exploring the outdoor surface above the tunnel.
        let mut links: HashMap<IVec2, Vec<IVec2>> = HashMap::new();
        for (&key, &from) in &connected {
            for delta in [IVec2::X, IVec2::Y, IVec2::new(1, 1), IVec2::new(1, -1)] {
                let next = key + delta;
                if let Some(&to) = connected.get(&next)
                    && nav::clear(&world, agent, from, to)
                    && nav::clear(&world, agent, to, from)
                {
                    links.entry(key).or_default().push(next);
                    links.entry(next).or_default().push(key);
                }
            }
        }
        Self {
            connected,
            world,
            links,
            route: Mutex::new(None),
        }
    }
    fn anchor(&self, feet: Vec3, interior: bool) -> Option<IVec2> {
        let mut candidates: Vec<_> = self
            .connected
            .iter()
            .filter(|(_, p)| {
                if interior {
                    p.xz().distance_squared(feet.xz()) < 0.75 * 0.75 && (p.y - feet.y).abs() < 0.7
                } else {
                    p.z - self.world.home.z > 3. && p.xz().distance_squared(feet.xz()) < 3. * 3.
                }
            })
            .map(|(&key, &p)| (key, p))
            .collect();
        candidates.sort_by(|a, b| {
            a.1.distance_squared(feet)
                .total_cmp(&b.1.distance_squared(feet))
                .then_with(|| a.0.x.cmp(&b.0.x))
                .then_with(|| a.0.y.cmp(&b.0.y))
        });
        candidates.into_iter().find_map(|(key, p)| {
            nav::clear(&navigation::World, Agent::default(), feet, p).then_some(key)
        })
    }

    fn path(&self, from: Vec3, target: Vec3) -> Option<VecDeque<Vec3>> {
        let on_floor = |p: Vec3| {
            let local = p - self.world.home;
            local.y < -0.15
                && floors::mesh(self.world.kind)
                    .rounded_support(local.xz(), Agent::default().radius)
                    .is_some_and(|y| (y - local.y).abs() < 0.21)
        };
        let (from_inside, target_inside) = (on_floor(from), on_floor(target));
        if !from_inside && !target_inside {
            return None;
        }
        let destination = if target_inside {
            target
        } else {
            return_mouth(self.world.home)
        };
        let start = self.anchor(from, from_inside)?;
        let end = self.anchor(destination, target_inside)?;
        if start == end && nav::clear(&navigation::World, Agent::default(), from, destination) {
            return Some(VecDeque::from([destination]));
        }
        let mut cache = self.route.lock().unwrap();
        if cache.as_ref().is_none_or(|(key, _)| *key != end) {
            let mut next = HashMap::from([(end, end)]);
            let mut queue = VecDeque::from([end]);
            while let Some(key) = queue.pop_front() {
                for &neighbor in self.links.get(&key).into_iter().flatten() {
                    if let std::collections::hash_map::Entry::Vacant(entry) = next.entry(neighbor) {
                        entry.insert(key);
                        queue.push_back(neighbor);
                    }
                }
            }
            *cache = Some((end, next));
        }
        let tree = &cache.as_ref()?.1;
        let mut key = start;
        let mut path = VecDeque::from([self.connected[&key]]);
        while key != end {
            key = *tree.get(&key)?;
            path.push_back(self.connected[&key]);
        }
        path.push_back(destination);
        Some(path)
    }
    fn reaches(&self, feet: Vec3) -> bool {
        // Recovery must land on ordinary walkable support, not a steep bank
        // whose only legal move is a fragile downhill escape.
        if !nav::walkable_support(&self.world, Agent::default(), feet) {
            return false;
        }
        let grid = (feet.xz() / nav::SPACING).round().as_ivec2();
        let world = &self.world;
        if let Some(next) = self.connected.get(&grid)
            && nav::walk(world, Agent::default(), feet, next.xz()).is_some()
        {
            return true;
        }
        for x in -1..=1 {
            for z in -1..=1 {
                if x == 0 && z == 0 {
                    continue;
                }
                if let Some(next) = self.connected.get(&(grid + IVec2::new(x, z)))
                    && nav::walk(world, Agent::default(), feet, next.xz()).is_some()
                {
                    return true;
                }
            }
        }
        false
    }
}
type RegionSlot = Arc<OnceLock<Arc<Region>>>;

fn request_region(kind: usize, home: Vec3) -> Option<RegionSlot> {
    type Key = (usize, u32, u32);
    static REGIONS: OnceLock<Mutex<HashMap<Key, RegionSlot>>> = OnceLock::new();
    let key = (kind, home.x.to_bits(), home.z.to_bits());
    let mut cache = REGIONS.get_or_init(Default::default).lock().unwrap();
    if let Some(region) = cache.get(&key) {
        return Some(region.clone());
    }
    // Never hold the cache lock while flooding the floor graph. In particular,
    // a physics query must not wait for several seconds of connectivity work.
    if cache.len() >= 32 {
        let old = cache
            .iter()
            .find_map(|(key, slot)| slot.get().map(|_| *key))?;
        cache.remove(&old);
    }
    let slot = Arc::new(OnceLock::new());
    cache.insert(key, slot.clone());
    drop(cache);
    let result = slot.clone();
    #[cfg(test)]
    bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    bevy::tasks::AsyncComputeTaskPool::get()
        .spawn(async move {
            let _ = result.set(Arc::new(Region::build(kind, home)));
        })
        .detach();
    Some(slot)
}

pub(super) fn prepare(home: Vec3) {
    let _ = request_region(den_kind((home.xz() / CELL).floor().as_ivec2()), home);
}

#[cfg(test)]
fn region(kind: usize, home: Vec3) -> Arc<Region> {
    loop {
        if let Some(slot) = request_region(kind, home) {
            return slot.wait().clone();
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Residents retain a corridor through cave corners instead of selecting a
/// fresh nearest anchor on every frame (which can alternate across a corner).
#[derive(Default)]
pub(super) struct Follower {
    path: VecDeque<Vec3>,
    goal: Option<Vec3>,
    smoothing: f32,
    progress: Option<(Vec3, f32)>,
    stalled: f32,
}
impl Follower {
    pub fn steer(&mut self, from: Vec3, target: Vec3, dt: f32) -> Option<Vec3> {
        let cell = (from.xz() / CELL).floor().as_ivec2();
        let home = site(cell)?;
        if from.xz().distance_squared(home.xz()) > 24. * 24. {
            return None;
        }
        let kind = den_kind(cell);
        let inside = |p: Vec3| {
            let local = p - home;
            local.y < -0.15
                && floors::mesh(kind)
                    .rounded_support(local.xz(), Agent::default().radius)
                    .is_some_and(|y| (y - local.y).abs() < 0.21)
        };
        let target_inside = inside(target);
        if !inside(from) && !target_inside {
            *self = Self::default();
            return None;
        }
        let goal = if target_inside {
            target
        } else {
            return_mouth(home)
        };
        self.smoothing -= dt;
        if let Some(next) = self.path.front().copied() {
            let distance = from.distance(next);
            if self
                .progress
                .is_none_or(|(old, best)| old != next || distance < best - 0.04)
            {
                self.progress = Some((next, distance));
                self.stalled = 0.;
            } else {
                self.stalled += dt;
            }
        }
        if self.path.is_empty()
            || self.goal.is_none_or(|old| old.distance(goal) > 0.6)
            || self.stalled > 0.8
        {
            #[cfg(test)]
            let prepared = region(kind, home);
            #[cfg(not(test))]
            let prepared = request_region(kind, home)?.get()?.clone();
            self.path = prepared.path(from, goal)?;
            self.goal = Some(goal);
            self.smoothing = 0.;
            self.progress = None;
            self.stalled = 0.;
        }
        while self
            .path
            .front()
            .is_some_and(|next| from.distance(*next) < 0.02)
        {
            self.path.pop_front();
            self.smoothing = 0.;
        }
        if self.smoothing <= 0. {
            self.smoothing = 0.15;
            for i in (1..self.path.len().min(6)).rev() {
                if from.xz().distance(self.path[i].xz()) <= 1.
                    && nav::clear(&navigation::World, Agent::default(), from, self.path[i])
                {
                    self.path.drain(..i);
                    break;
                }
            }
        }
        self.path.front().copied()
    }
}

/// Airborne actors and the outdoor layer are not restricted to cave-floor
/// components. A body landing on the cave floor must have a supported exit.
pub(super) fn connected(feet: Vec3) -> bool {
    let cell = (feet.xz() / CELL).floor().as_ivec2();
    let Some(home) = site(cell) else {
        return true;
    };
    let local = feet - home;
    if local.y >= 0. || local.xz().length_squared() > 23. * 23. {
        return true;
    }
    let kind = den_kind(cell);
    let Some(y) =
        floors::mesh(kind).rounded_support(local.xz(), crate::player::movement::PLAYER_RADIUS)
    else {
        return true;
    };
    if (y - local.y).abs() > 0.201 {
        return true;
    }
    // Pending connectivity is not a wall. Actual support and swept body
    // collision remain authoritative while the background result is prepared.
    #[cfg(test)]
    return region(kind, home).reaches(feet);
    #[cfg(not(test))]
    request_region(kind, home)
        .is_none_or(|slot| slot.get().is_none_or(|region| region.reaches(feet)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cold_connectivity_request_returns_without_waiting_for_the_flood() {
        // Exercise the production request path, not the blocking test helper.
        let home = Vec3::new(7351.125, 0., 9230.75);
        let before = std::time::Instant::now();
        let slot = request_region(0, home).unwrap();
        assert!(before.elapsed() < std::time::Duration::from_millis(100));
        assert!(Arc::ptr_eq(&slot, &request_region(0, home).unwrap()));
        assert!(slot.wait().connected.len() > 200);
    }

    #[test]
    fn certified_floor_regions_use_actual_world_body_clearance() {
        for row in [43, 139, 162] {
            for kind in 0..3 {
                let (cell, home) = (100..500)
                    .find_map(|x| {
                        let cell = IVec2::new(x, row);
                        site(cell)
                            .filter(|_| den_kind(cell) == kind)
                            .map(|home| (cell, home))
                    })
                    .unwrap();
                open_gates().write().unwrap().insert(cell);
                gate_openness().write().unwrap().insert(cell, 1.);
                let region = region(kind, home);
                assert!(region.connected.len() > 200);
                for (node, parent) in region.links.iter().flat_map(|(node, neighbors)| {
                    neighbors.iter().map(move |parent| (node, parent))
                }) {
                    let (a, b) = (region.connected[parent], region.connected[node]);
                    assert!(
                        nav::clear(&navigation::World, Agent::default(), a, b)
                            && nav::clear(&navigation::World, Agent::default(), b, a),
                        "false floor edge kind={kind} row={row} a={:?} b={:?} global {:?}/{:?} local {:?}/{:?}",
                        a - home,
                        b - home,
                        nav::walk(&navigation::World, Agent::default(), a, b.xz()),
                        nav::walk(&navigation::World, Agent::default(), b, a.xz()),
                        nav::walk(&region.world, Agent::default(), a, b.xz()),
                        nav::walk(&region.world, Agent::default(), b, a.xz())
                    );
                }
                for point in region.connected.values() {
                    assert!(
                        navigation::World.body_clear(*point, Agent::default()),
                        "false floor certification: kind={kind} row={row} p={point:?}"
                    );
                }
                open_gates().write().unwrap().remove(&cell);
                gate_openness().write().unwrap().remove(&cell);
            }
        }
    }
}
