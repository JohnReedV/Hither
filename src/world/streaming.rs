//! Shared admission and scheduling for world streaming. Content adapters retain
//! their own geometry, cell sizes and gameplay state; this module owns the
//! frame budget, worker lifetime and the authoritative loading envelope.
use bevy::prelude::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

mod feedback;

const FRAME_UNITS: usize = 64;
const LAYER_UNITS: usize = 32;
const RESERVED_UNITS: usize = 16;
const WORKERS: usize = 12;

#[derive(SystemSet, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Layer {
    Terrain,
    Grass,
    Citrus,
    Conifers,
    Oaks,
    Alpine,
    Rocks,
    Understory,
    Logs,
    Dens,
    Goblins,
    Orcs,
    Contacts,
}
const LAYERS: [Layer; 13] = [
    Layer::Terrain,
    Layer::Grass,
    Layer::Citrus,
    Layer::Conifers,
    Layer::Oaks,
    Layer::Alpine,
    Layer::Rocks,
    Layer::Understory,
    Layer::Logs,
    Layer::Dens,
    Layer::Goblins,
    Layer::Orcs,
    Layer::Contacts,
];

pub(crate) struct StreamingPlugin;
impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Coordinator>()
            .init_resource::<Retirements>()
            .add_systems(Last, collect_retired)
            .add_systems(
                Update,
                begin.after(crate::rendering::sdf::update_shader_uniforms),
            )
            .configure_sets(
                Update,
                (
                    Layer::Terrain,
                    Layer::Grass,
                    Layer::Citrus,
                    Layer::Conifers,
                    Layer::Oaks,
                    Layer::Alpine,
                    Layer::Rocks,
                    Layer::Understory,
                    Layer::Logs,
                    Layer::Dens,
                    Layer::Goblins,
                    Layer::Orcs,
                    Layer::Contacts,
                )
                    .after(begin),
            );
        feedback::install(app, app.world().resource::<Coordinator>().feedback.clone());
    }
}

/// Hide obsolete roots in the current command flush, then dismantle their
/// hierarchies leaf-first. Despawning a parent directly would bypass the cap.
#[derive(Resource, Default)]
struct Retirements {
    roots: std::collections::VecDeque<Entity>,
    stack: Vec<Entity>,
}
pub(crate) fn retire(commands: &mut Commands, entity: Entity) {
    commands.queue(move |world: &mut World| {
        let Ok(mut root) = world.get_entity_mut(entity) else {
            return;
        };
        root.insert(Visibility::Hidden);
        world.init_resource::<Retirements>();
        world.resource_mut::<Retirements>().roots.push_back(entity);
    });
}
fn collect_retired(world: &mut World) {
    let start = std::time::Instant::now();
    world.resource_scope(|world, mut retired: Mut<Retirements>| {
        let mut removed = 0;
        for _ in 0..512 {
            if start.elapsed().as_micros() >= 500 {
                break;
            }
            if retired.stack.is_empty() {
                let Some(root) = retired.roots.pop_front() else {
                    break;
                };
                retired.stack.push(root);
            }
            let entity = *retired.stack.last().unwrap();
            if let Some(child) = world
                .get::<Children>(entity)
                .and_then(|c| c.last())
                .copied()
            {
                retired.stack.push(child);
            } else {
                retired.stack.pop();
                if world.despawn(entity) {
                    removed += 1;
                }
                if removed == 256 {
                    break;
                }
            }
        }
    });
}

#[derive(Resource)]
pub(crate) struct Coordinator {
    remaining: usize,
    spent: [usize; 13],
    denied: [bool; 13],
    priority: usize,
    installation_priority: Option<usize>,
    worker_denied: [bool; 13],
    worker_priority: usize,
    workers: Arc<AtomicUsize>,
    focal: f32,
    projection_changed: bool,
    installation_us: Arc<AtomicUsize>,
    upload_bytes: usize,
    entities: usize,
    bake_taken: bool,
    bake_priority: usize,
    bake_reserved: Option<usize>,
    bake_waiting: [bool; 2],
    camera: crate::player::camera::CameraView,
    view_changed: bool,
    generation_us: Arc<AtomicUsize>,
    generation_recent: usize,
    feedback: feedback::Feedback,
}
impl Default for Coordinator {
    fn default() -> Self {
        Self {
            remaining: FRAME_UNITS,
            spent: [0; 13],
            denied: [false; 13],
            priority: 0,
            installation_priority: None,
            worker_denied: [false; 13],
            worker_priority: 0,
            workers: Arc::new(AtomicUsize::new(0)),
            focal: 720.0 / 1.32,
            projection_changed: true,
            installation_us: Arc::new(AtomicUsize::new(0)),
            upload_bytes: 0,
            entities: 0,
            bake_taken: false,
            bake_priority: 0,
            bake_reserved: None,
            bake_waiting: [false; 2],
            camera: crate::player::camera::CameraRig::default().view(),
            view_changed: true,
            generation_us: Arc::new(AtomicUsize::new(0)),
            generation_recent: 0,
            feedback: default(),
        }
    }
}
fn next_waiter(waiting: &[bool; 13], previous: usize) -> usize {
    (1..=LAYERS.len())
        .map(|n| (previous + n) % LAYERS.len())
        .find(|&i| waiting[i])
        .unwrap_or((previous + 1) % LAYERS.len())
}
impl Coordinator {
    pub(crate) fn view_changed(&self) -> bool {
        self.view_changed
    }
    pub(crate) fn focal_pixels(&self) -> f32 {
        self.focal
    }
    pub(crate) fn projection_changed(&self) -> bool {
        self.projection_changed
    }
    fn begin(&mut self) {
        self.generation_recent = self.generation_us.swap(0, Ordering::Relaxed);
        self.installation_us.store(0, Ordering::Relaxed);
        self.upload_bytes = 0;
        self.entities = 0;
        self.bake_taken = false;
        if self.bake_waiting[1 - self.bake_priority] {
            self.bake_priority = 1 - self.bake_priority;
        }
        self.bake_reserved = self.bake_waiting[self.bake_priority].then_some(self.bake_priority);
        self.bake_waiting.fill(false);
        self.priority = next_waiter(&self.denied, self.priority);
        self.installation_priority = self.denied[self.priority].then_some(self.priority);
        self.worker_priority = next_waiter(&self.worker_denied, self.worker_priority);
        self.remaining = FRAME_UNITS;
        self.spent.fill(0);
        self.denied.fill(false);
        self.worker_denied.fill(false);
    }

    /// Weighted installation/generation units, not milliseconds. A reservation
    /// rotates among blocked adapters so fixed system ordering cannot starve one.
    /// Eviction and polling obsolete tasks must never depend on this budget.
    pub(crate) fn claim(&mut self, layer: Layer, units: usize) -> bool {
        assert!(units > 0 && units <= RESERVED_UNITS);
        let i = layer as usize;
        let reserve = if i != self.priority
            && (self.spent[self.priority] == 0 || self.installation_priority == Some(self.priority))
        {
            RESERVED_UNITS
        } else {
            0
        };
        if self.spent[i] + units > LAYER_UNITS || self.remaining < units + reserve {
            self.denied[i] = true;
            return false;
        }
        self.remaining -= units;
        self.spent[i] += units;
        true
    }

    /// Time is charged at scope exit. Jobs are atomic, so one installation may
    /// overshoot; subsequent work is deferred rather than adding another stall.
    pub(crate) fn install(
        &mut self,
        layer: Layer,
        units: usize,
        bytes: usize,
    ) -> Option<Installation> {
        self.install_work(layer, units, bytes, 0)
    }
    pub(crate) fn install_entities(
        &mut self,
        layer: Layer,
        units: usize,
        bytes: usize,
        entities: usize,
    ) -> Option<Installation> {
        self.install_work(layer, units, bytes, entities)
    }
    fn install_work(
        &mut self,
        layer: Layer,
        units: usize,
        bytes: usize,
        entities: usize,
    ) -> Option<Installation> {
        let actual_work = bytes > 0 || entities > 0;
        let priority_work = actual_work && self.installation_priority == Some(layer as usize);
        // Preserve a waiting layer's first installation, not just abstract units.
        // Otherwise small early uploads can exhaust time/bytes before a larger
        // waiting payload every frame. Discovery may proceed; an abandoned
        // reservation expires at begin(), and the next waiting layer takes over.
        if actual_work
            && self
                .installation_priority
                .is_some_and(|i| i != layer as usize)
        {
            self.denied[layer as usize] = true;
            return None;
        }
        let downstream = self.feedback.estimate(
            self.upload_bytes.saturating_add(bytes),
            self.entities.saturating_add(entities),
        );
        let measured = self.installation_us.load(Ordering::Relaxed);
        let over_time = measured >= 1500
            || ((self.upload_bytes > 0 || self.entities > 0)
                && measured.saturating_add(downstream) >= 1500);
        if (!priority_work && over_time)
            || self.upload_bytes.saturating_add(bytes) > 4 * 1024 * 1024
            || self.entities.saturating_add(entities) > 256
        {
            self.denied[layer as usize] = true;
            return None;
        }
        if !self.claim(layer, units) {
            return None;
        }
        if priority_work {
            self.installation_priority = None;
        }
        self.upload_bytes += bytes;
        self.entities += entities;
        Some(Installation {
            start: std::time::Instant::now(),
            elapsed: self.installation_us.clone(),
        })
    }
    /// Both custom bakers share one adaptive batch per frame. A denied baker
    /// receives the next turn so continuous terrain work cannot starve rocks.
    pub(crate) fn bake(&mut self, rock: bool) -> bool {
        let index = usize::from(rock);
        if self.bake_taken || self.bake_reserved.is_some_and(|reserved| index != reserved) {
            self.bake_waiting[index] = true;
            return false;
        }
        self.bake_taken = true;
        self.bake_waiting[index] = false;
        self.bake_priority = index;
        true
    }
    /// Conservative view-cone priority, grouped into 128m regions then 32m
    /// cells. Hidden residency remains available without outranking coverage.
    pub(crate) fn visual_rank(&self, p: Vec3, radius: f32) -> (u8, i32, i32) {
        let region = |span: f32| {
            let xz = (p.xz() / span).floor() * span + Vec2::splat(span * 0.5);
            Vec3::new(xz.x, p.y, xz.y)
        };
        let parent_distance = region(128.0).distance(self.camera.position);
        let delta = region(32.0) - self.camera.position;
        let radius = radius + 32.0 * std::f32::consts::FRAC_1_SQRT_2;
        let distance = delta.length();
        let forward = delta.dot(self.camera.forward);
        let visible = forward + radius > 0.0 && forward + radius >= distance * 0.45;
        (
            if visible {
                0
            } else if distance < 64.0 + radius {
                1
            } else {
                2
            },
            parent_distance as i32,
            distance as i32,
        )
    }

    pub(crate) fn worker(&mut self, layer: Layer) -> Option<WorkerPermit> {
        if self.generation_recent >= 32_000 {
            self.worker_denied[layer as usize] = true;
            return None;
        }
        let limit = WORKERS - usize::from(layer as usize != self.worker_priority);
        if self.workers.load(Ordering::Acquire) >= limit {
            self.worker_denied[layer as usize] = true;
            return None;
        }
        self.workers.fetch_add(1, Ordering::AcqRel);
        Some(WorkerPermit(
            self.workers.clone(),
            self.generation_us.clone(),
        ))
    }
}

pub(crate) struct Installation {
    start: std::time::Instant,
    elapsed: Arc<AtomicUsize>,
}
impl Drop for Installation {
    fn drop(&mut self) {
        self.elapsed
            .fetch_add(self.start.elapsed().as_micros() as usize, Ordering::Relaxed);
    }
}

fn begin(
    mut coordinator: ResMut<Coordinator>,
    settings: Option<Res<crate::app::settings::GraphicsSettings>>,
    rig: Option<Res<crate::player::camera::CameraRig>>,
    view: Option<Res<crate::player::camera::CameraView>>,
    range: Option<ResMut<crate::rendering::view_distance::Range>>,
    windows: Query<&Window>,
) {
    coordinator.begin();
    if let Some(view) = view.as_ref() {
        coordinator.view_changed = coordinator.camera.position.distance_squared(view.position)
            > 16.0
            || coordinator.camera.forward.dot(view.forward) < 0.98;
        if coordinator.view_changed {
            coordinator.camera = **view;
        }
    }
    if let (Some(settings), Some(mut range)) = (settings, range) {
        let native = windows.iter().next().map_or(UVec2::new(1280, 720), |w| {
            UVec2::new(w.physical_width().max(1), w.physical_height().max(1))
        });
        let focal =
            crate::rendering::graphics::render_size(settings.resolution, native).y as f32 / 1.32;
        coordinator.projection_changed = coordinator.focal != focal;
        coordinator.focal = focal;
        // Stream around the player, including the actual third-person camera
        // offset. Chunk/object extent allowances are added by each adapter.
        let offset = rig
            .zip(view)
            .map_or(4.0, |(rig, view)| rig.position.distance(view.position));
        range.load = settings.render_distance + offset.max(4.0);
    }
}

/// Captured by the worker, never merely by its ECS owner. Cancelling an obsolete
/// task cannot release its slot while synchronous meshing is still executing.
pub(crate) struct WorkerPermit(
    Arc<AtomicUsize>,
    #[cfg_attr(test, allow(dead_code))] Arc<AtomicUsize>,
);
impl Drop for WorkerPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) struct BuildTask<T: Send + 'static> {
    #[cfg(not(test))]
    task: bevy::tasks::Task<(T, WorkerPermit)>,
    ready: Option<(T, WorkerPermit)>,
}
impl<T: Send + 'static> BuildTask<T> {
    #[cfg(test)]
    pub(crate) fn pending_for_test() -> Self {
        Self { ready: None }
    }
    #[cfg(test)]
    pub(crate) fn finish_for_test(&mut self, value: T) {
        self.ready = Some((
            value,
            WorkerPermit(Arc::new(AtomicUsize::new(1)), Arc::new(AtomicUsize::new(0))),
        ));
    }
    pub(crate) fn new(permit: WorkerPermit, build: impl FnOnce() -> T + Send + 'static) -> Self {
        Self {
            #[cfg(not(test))]
            task: bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
                let start = std::time::Instant::now();
                let value = build();
                permit
                    .1
                    .fetch_add(start.elapsed().as_micros() as usize, Ordering::Relaxed);
                (value, permit)
            }),
            #[cfg(not(test))]
            ready: None,
            #[cfg(test)]
            ready: Some((build(), permit)),
        }
    }
    pub(crate) fn is_ready(&mut self) -> bool {
        #[cfg(not(test))]
        if self.ready.is_none() {
            self.ready = bevy::tasks::block_on(bevy::tasks::poll_once(&mut self.task));
        }
        self.ready.is_some()
    }
    pub(crate) fn ready_ref(&self) -> Option<&T> {
        self.ready.as_ref().map(|(value, _)| value)
    }
    pub(crate) fn ready_mut(&mut self) -> Option<&mut T> {
        self.ready.as_mut().map(|(value, _)| value)
    }
    pub(crate) fn take(&mut self) -> T {
        self.ready
            .take()
            .expect("poll before taking a streaming result")
            .0
    }
}

/// Lazy near-to-far square traversal. Resetting a window costs O(1), including
/// teleports and large distance increases; adapters admit each bounded batch.
#[derive(Default)]
pub(crate) struct CellScan {
    center: IVec2,
    radius: i32,
    ring: i32,
    edge: i32,
}
impl CellScan {
    pub(crate) fn new(center: IVec2, radius: i32) -> Self {
        Self {
            center,
            radius,
            ring: 0,
            edge: 0,
        }
    }
}
impl Iterator for CellScan {
    type Item = IVec2;
    fn next(&mut self) -> Option<IVec2> {
        let r = self.ring;
        if r > self.radius {
            return None;
        }
        if r == 0 {
            self.ring = 1;
            return Some(self.center);
        }
        let side = self.edge / (2 * r);
        let t = self.edge % (2 * r);
        let offset = match side {
            0 => IVec2::new(-r + t, -r),
            1 => IVec2::new(r, -r + t),
            2 => IVec2::new(r - t, r),
            _ => IVec2::new(-r, r - t),
        };
        self.edge += 1;
        if self.edge == 8 * r {
            self.ring += 1;
            self.edge = 0;
        }
        Some(self.center + offset)
    }
}

/// Preserve unfinished discovery on movement, adding only the entering strips.
/// This prevents continual movement from restarting a large window forever.
#[derive(Default)]
pub(crate) struct WindowScan {
    window: Option<(IVec2, i32)>,
    scans: std::collections::VecDeque<Box<dyn Iterator<Item = IVec2> + Send + Sync>>,
}
impl WindowScan {
    pub(crate) fn update(&mut self, center: IVec2, radius: i32) {
        let previous = self.window;
        self.window = Some((center, radius));
        if previous
            .is_none_or(|(old, r)| (old - center).abs().max_element() > r + radius || radius < r)
        {
            self.scans.clear();
            self.scans
                .push_back(Box::new(CellScan::new(center, radius)));
        } else {
            self.scans
                .push_back(Box::new(entering_cells(center, radius, previous)));
        }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.scans.is_empty()
    }
}
impl Iterator for WindowScan {
    type Item = IVec2;
    fn next(&mut self) -> Option<IVec2> {
        while let Some(scan) = self.scans.front_mut() {
            if let Some(cell) = scan.next() {
                return Some(cell);
            }
            self.scans.pop_front();
        }
        None
    }
}

/// Lazy cells intersecting a circular distance band, padded for cell extent.
/// Visits strips of the annulus, not the interior of the loaded square.
pub(crate) fn distance_band(
    center: IVec2,
    inner: f32,
    outer: f32,
) -> impl Iterator<Item = IVec2> + Send + Sync {
    let outer = outer.ceil() as i32;
    let inner = inner.max(0.0);
    (-outer..=outer).flat_map(move |x| {
        let top = ((outer * outer - x * x) as f32).sqrt().ceil() as i32;
        let hole = (inner * inner - (x * x) as f32).max(0.0).sqrt().floor() as i32;
        (-top..=-hole)
            .chain((hole.max(1))..=top)
            .map(move |z| center + IVec2::new(x, z))
    })
}
#[derive(Default)]
pub(crate) struct LodBands {
    scans: std::collections::VecDeque<Box<dyn Iterator<Item = IVec2> + Send + Sync>>,
    fallback: Option<CellScan>,
    latest: Option<(IVec2, i32)>,
    dirty: bool,
    alternate: bool,
}
impl LodBands {
    pub(crate) fn push(
        &mut self,
        cells: impl Iterator<Item = IVec2> + Send + Sync + 'static,
        center: IVec2,
        radius: i32,
    ) {
        self.latest = Some((center, radius));
        if self.scans.len() >= 32 {
            // Coalesce overload into one lazy catch-up sweep. Never restart an
            // unfinished sweep during movement, and never retain an unbounded
            // history of superseded distance bands.
            self.scans.clear();
            self.dirty = true;
        }
        if self.dirty && self.fallback.is_none() {
            self.fallback = Some(CellScan::new(center, radius));
            self.dirty = false;
        }
        self.scans.push_back(Box::new(cells));
    }
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}
impl Iterator for LodBands {
    type Item = IVec2;
    fn next(&mut self) -> Option<IVec2> {
        self.alternate = !self.alternate;
        if (self.alternate || self.scans.is_empty())
            && let Some(scan) = &mut self.fallback
        {
            if let Some(cell) = scan.next() {
                return Some(cell);
            }
            self.fallback = None;
            if self.dirty
                && let Some((center, radius)) = self.latest
            {
                self.fallback = Some(CellScan::new(center, radius));
                self.dirty = false;
            }
        }
        while let Some(scan) = self.scans.front_mut() {
            if let Some(cell) = scan.next() {
                return Some(cell);
            }
            self.scans.pop_front();
        }
        self.fallback.as_mut().and_then(Iterator::next)
    }
}

/// Bounded placement discovery. Both accepted and empty cells are remembered;
/// workers retain their permit through completion and stale results are discarded.
pub(crate) struct Discovery<T: Send + Sync + 'static> {
    scan: WindowScan,
    evaluated: std::collections::HashSet<IVec2>,
    task: Option<BuildTask<Vec<(IVec2, T)>>>,
    ready: std::collections::VecDeque<(IVec2, T)>,
    window: Option<(IVec2, i32)>,
}
impl<T: Send + Sync + 'static> Default for Discovery<T> {
    fn default() -> Self {
        Self {
            scan: WindowScan::default(),
            evaluated: default(),
            task: None,
            ready: default(),
            window: None,
        }
    }
}
impl<T: Send + Sync + 'static> Discovery<T> {
    pub(crate) fn update(&mut self, center: IVec2, radius: i32) {
        if self.window == Some((center, radius)) {
            return;
        }
        self.window = Some((center, radius));
        self.evaluated
            .retain(|c| (*c - center).abs().max_element() <= radius);
        self.ready
            .retain(|(c, _)| (*c - center).abs().max_element() <= radius);
        self.scan.update(center, radius);
    }
    pub(crate) fn poll(
        &mut self,
        coordinator: &mut Coordinator,
        layer: Layer,
        build: fn(IVec2) -> T,
    ) {
        let Some((center, radius)) = self.window else {
            return;
        };
        if let Some(task) = &mut self.task
            && task.is_ready()
        {
            for (cell, value) in task.take() {
                if (cell - center).abs().max_element() <= radius && self.evaluated.insert(cell) {
                    self.ready.push_back((cell, value));
                }
            }
            self.task = None;
        }
        if self.task.is_some() || self.ready.len() >= 64 || self.scan.is_empty() {
            return;
        }
        let Some(permit) = coordinator.worker(layer) else {
            return;
        };
        let Some(_scope) = coordinator.install(layer, 1, 0) else {
            return;
        };
        let mut batch = Vec::new();
        for _ in 0..128 {
            let Some(cell) = self.scan.next() else {
                break;
            };
            if (cell - center).abs().max_element() <= radius
                && !self.evaluated.contains(&cell)
                && !batch.contains(&cell)
            {
                batch.push(cell);
            }
            if batch.len() == 32 {
                break;
            }
        }
        if !batch.is_empty() {
            self.task = Some(BuildTask::new(permit, move || {
                batch.into_iter().map(|c| (c, build(c))).collect()
            }));
        }
    }
    pub(crate) fn front(&self) -> Option<&(IVec2, T)> {
        self.ready.front()
    }
    pub(crate) fn pop(&mut self) -> Option<(IVec2, T)> {
        self.ready.pop_front()
    }
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.scan.is_empty() && self.task.is_none() && self.ready.is_empty()
    }
}

// Inclusive rectangle difference, shared by every adapter using cell windows.
#[allow(clippy::reversed_empty_ranges)]
pub(crate) fn entering_cells(
    center: IVec2,
    radius: i32,
    previous: Option<(IVec2, i32)>,
) -> impl Iterator<Item = IVec2> {
    let low = center - IVec2::splat(radius);
    let high = center + IVec2::splat(radius);
    (low.x..=high.x).flat_map(move |x| {
        let (bottom, top) = match previous {
            Some((old, r)) if (old.x - r..=old.x + r).contains(&x) => (
                low.y..=high.y.min(old.y - r - 1),
                low.y.max(old.y + r + 1)..=high.y,
            ),
            _ => (low.y..=high.y, 1..=0),
        };
        bottom.chain(top).map(move |z| IVec2::new(x, z))
    })
}

/// Vertex layouts and index widths determine actual mesh upload payload.
pub(crate) fn mesh_bytes(mesh: &Mesh) -> usize {
    mesh.get_vertex_buffer_size()
        + mesh.indices().map_or(0, |i| match i {
            bevy::mesh::Indices::U16(v) => v.len() * 2,
            bevy::mesh::Indices::U32(v) => v.len() * 4,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retiring_large_hierarchies_hides_now_and_despawns_in_batches() {
        let mut world = World::new();
        world.init_resource::<Retirements>();
        let root = world.spawn(Visibility::Inherited).id();
        let children: Vec<_> = (0..1000).map(|_| world.spawn_empty().id()).collect();
        world.entity_mut(root).add_children(&children);
        retire(&mut world.commands(), root);
        world.flush();
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
        collect_retired(&mut world);
        assert!(world.get_entity(root).is_ok());
        assert!(
            children
                .iter()
                .filter(|&&e| world.get_entity(e).is_err())
                .count()
                <= 256
        );
        for _ in 0..10 {
            collect_retired(&mut world);
        }
        assert!(world.get_entity(root).is_err());
        assert!(children.iter().all(|&e| world.get_entity(e).is_err()));
    }
    #[test]
    fn gpu_bakers_alternate_and_abandoned_reservations_expire() {
        let mut c = Coordinator::default();
        c.begin();
        assert!(c.bake(false));
        assert!(!c.bake(true));
        c.begin();
        assert!(!c.bake(false));
        assert!(c.bake(true));
        c.begin();
        assert!(c.bake(false));
        assert!(!c.bake(true));
        c.begin();
        assert!(!c.bake(false)); // Rock work disappeared after requesting a turn.
        c.begin();
        assert!(c.bake(false));
        c.begin();
        assert!(c.bake(false));
    }
    #[test]
    fn discovery_is_bounded_retains_progress_and_discards_teleport_results() {
        let mut c = Coordinator::default();
        let mut discovery = Discovery::<IVec2>::default();
        discovery.update(IVec2::ZERO, 114);
        discovery.poll(&mut c, Layer::Alpine, |cell| cell);
        assert!(discovery.task.as_ref().unwrap().ready_ref().unwrap().len() <= 32);
        discovery.update(IVec2::splat(-1000), 2);
        let mut found = std::collections::HashSet::new();
        for _ in 0..100 {
            c.begin();
            discovery.poll(&mut c, Layer::Alpine, |cell| cell);
            while let Some((cell, value)) = discovery.pop() {
                assert_eq!(cell, value);
                assert!((cell + IVec2::splat(1000)).abs().max_element() <= 2);
                assert!(found.insert(cell));
            }
        }
        assert_eq!(found.len(), 25);
        assert!(discovery.is_empty());
    }
    #[test]
    fn distance_bands_cover_every_threshold_crossing() {
        let center = IVec2::new(-4, 7);
        let old = center + IVec2::new(2, -3);
        let radius = 31.5;
        let pad = (center - old).as_vec2().length() + 3.0;
        let cells: std::collections::HashSet<_> =
            distance_band(center, radius - pad, radius + pad).collect();
        for cell in CellScan::new(center, 50) {
            let d = |c: IVec2| {
                ((cell - c).abs() - IVec2::ONE)
                    .max(IVec2::ZERO)
                    .as_vec2()
                    .length()
            };
            if (d(center) < radius) != (d(old) < radius) {
                assert!(cells.contains(&cell), "{cell:?}");
            }
        }
        assert!(cells.len() < 101 * 101 / 2);
    }
    #[test]
    fn entity_admission_is_independent_of_weighted_units() {
        let mut c = Coordinator::default();
        assert!(c.install_entities(Layer::Rocks, 1, 0, 228).is_some());
        assert!(c.install_entities(Layer::Alpine, 1, 0, 29).is_none());
        assert!(c.install_entities(Layer::Alpine, 1, 0, 28).is_some());
        c.begin();
        assert!(c.install_entities(Layer::Alpine, 1, 0, 29).is_some());
        assert!(c.install_entities(Layer::Dens, 1, 0, 128).is_some());
    }
    #[test]
    fn lazy_discovery_survives_partial_moves_shrinks_and_teleports() {
        use std::collections::HashSet;
        let center = IVec2::new(-8, -3);
        let cells: Vec<_> = CellScan::new(center, 33).collect();
        assert_eq!(cells.len(), 4489);
        assert_eq!(cells.iter().copied().collect::<HashSet<_>>().len(), 4489);
        assert!(cells.windows(2).all(|w| (w[0] - center).abs().max_element() <= (w[1] - center).abs().max_element()));
        let mut scan = WindowScan::default();
        let mut visited = HashSet::new();
        for (center, radius) in [
            (center, 8),
            (center + IVec2::X, 8),
            (center + IVec2::Y, 12),
            (center, 3),
            (-center * 100, 8),
        ] {
            scan.update(center, radius);
            visited.retain(|c: &IVec2| (*c - center).abs().max_element() <= radius);
            visited.extend(
                scan.by_ref()
                    .take(17)
                    .filter(|c| (*c - center).abs().max_element() <= radius),
            );
        }
        let (center, radius) = scan.window.unwrap();
        visited.extend(scan.filter(|c| (*c - center).abs().max_element() <= radius));
        assert_eq!(visited, entering_cells(center, radius, None).collect());
    }
    #[test]
    fn measured_time_and_upload_bytes_are_independent_limits() {
        let mut c = Coordinator::default();
        assert!(c.install(Layer::Terrain, 1, 4 * 1024 * 1024 + 1).is_none());
        let guard = c.install(Layer::Terrain, 1, 1024).unwrap();
        drop(guard);
        assert_eq!(c.upload_bytes, 1024);
        c.installation_us.store(1500, Ordering::Relaxed);
        assert!(c.install(Layer::Terrain, 1, 0).is_none());
        c.begin();
        c.upload_bytes = 4 * 1024 * 1024;
        assert!(c.install(Layer::Grass, 1, 1).is_none());
    }
    #[test]
    fn global_budget_is_bounded_and_every_layer_progresses() {
        let mut manager = Coordinator::default();
        let mut served = [0; 13];
        for _ in 0..52 {
            manager.begin();
            let mut total = 0;
            for layer in LAYERS {
                while manager.claim(layer, 16) {
                    total += 16;
                    served[layer as usize] += 1;
                }
            }
            assert!(total <= FRAME_UNITS);
        }
        assert!(served.iter().all(|&n| n > 0), "{served:?}");
    }
    #[test]
    fn ready_results_keep_worker_slots_until_consumed() {
        let mut manager = Coordinator::default();
        let mut tasks = Vec::new();
        for _ in 0..WORKERS {
            let permit = manager.worker(Layer::Terrain).unwrap();
            tasks.push(BuildTask::new(permit, || 7));
        }
        assert!(manager.worker(Layer::Terrain).is_none());
        assert!(tasks[0].is_ready());
        assert!(manager.worker(Layer::Terrain).is_none());
        assert_eq!(tasks[0].take(), 7);
        assert!(manager.worker(Layer::Terrain).is_some());
        drop(tasks);
        assert_eq!(manager.workers.load(Ordering::Acquire), 0);
    }
    #[test]
    fn windows_cover_negative_coordinates_shrink_and_teleport() {
        use std::collections::HashSet;
        let mut previous = None;
        let mut loaded = HashSet::new();
        for (center, radius) in [
            (IVec2::ZERO, 3),
            (IVec2::new(-1, -2), 3),
            (IVec2::new(-1, -2), 1),
            (IVec2::new(100, -100), 4),
        ] {
            loaded.retain(|c: &IVec2| (*c - center).abs().max_element() <= radius);
            loaded.extend(entering_cells(center, radius, previous));
            assert_eq!(loaded, entering_cells(center, radius, None).collect());
            previous = Some((center, radius));
        }
    }
    #[test]
    fn render_distance_and_camera_offset_apply_before_adapters() {
        fn observe(range: Res<crate::rendering::view_distance::Range>, mut seen: ResMut<Seen>) {
            seen.0 = range.load;
        }
        #[derive(Resource, Default)]
        struct Seen(f32);
        let rig = crate::player::camera::CameraRig::default();
        let mut view = rig.view();
        view.position = rig.position + Vec3::Z * 9.0;
        let mut app = App::new();
        app.add_plugins(StreamingPlugin)
            .insert_resource(rig)
            .insert_resource(view)
            .insert_resource(crate::app::settings::GraphicsSettings {
                render_distance: 1024.0,
                ..default()
            })
            .init_resource::<crate::rendering::view_distance::Range>()
            .init_resource::<Seen>()
            .add_systems(Update, observe.in_set(Layer::Grass));
        app.update();
        assert_eq!(app.world().resource::<Seen>().0, 1033.0);
        app.world_mut()
            .resource_mut::<crate::app::settings::GraphicsSettings>()
            .render_distance = 16.0;
        app.update();
        assert_eq!(app.world().resource::<Seen>().0, 25.0);
        // A teleport does not make the envelope depend on distance from origin.
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position += Vec3::splat(-10000.0);
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraView>()
            .position += Vec3::splat(-10000.0);
        app.update();
        assert_eq!(app.world().resource::<Seen>().0, 25.0);
    }
    #[test]
    fn worker_reservation_eventually_admits_each_waiting_layer() {
        let mut manager = Coordinator::default();
        let mut live = Vec::new();
        let mut served = [false; 13];
        for _ in 0..100 {
            manager.begin();
            if !live.is_empty() {
                live.remove(0);
            }
            for layer in LAYERS {
                while let Some(permit) = manager.worker(layer) {
                    served[layer as usize] = true;
                    live.push(permit);
                }
            }
            assert!(live.len() <= WORKERS);
        }
        assert!(served.into_iter().all(|s| s));
    }
}
