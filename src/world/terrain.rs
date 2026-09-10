//! Culled terrain tiles with worker-cached elevation/normals and bounded-error
//! distant LOD. Nearby geometry and collision retain the one-metre lattice.
use crate::world::streaming::{Coordinator, Layer};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TerrainDiscovery;

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            publish_gpu_bounds.before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
        app.add_systems(
            Update,
            resize
                .in_set(TerrainDiscovery)
                .in_set(Layer::Terrain)
                .after(crate::rendering::sdf::update_shader_uniforms),
        );
    }
}

// Vertex spacing is one metre on CPU and GPU. Cache lattice samples rather than
// arbitrary positions: plants and movement repeatedly query the same cells.
type HeightSlot = Option<(IVec2, f32)>;
thread_local! {
    static HEIGHTS: RefCell<Vec<HeightSlot>> = RefCell::new(vec![None; 16384]);
}
fn vertex_height(cell: IVec2) -> f32 {
    HEIGHTS.with_borrow_mut(|heights| {
        let h = (cell.x as u32).wrapping_mul(0x9e3779b9) ^ (cell.y as u32).wrapping_mul(0x85ebca6b);
        let index = ((h ^ (h >> 16)) as usize) & (heights.len() - 1);
        if let Some((key, value)) = heights[index]
            && key == cell
        {
            return value;
        }
        let p = cell.as_vec2();
        let height = crate::world::biome::terrain_for_seed(p, crate::world::biome::world_seed())
            * crate::world::orcs::terrain_clearance(p);
        // Constant-time replacement: never flush/rehash the cache during flight.
        heights[index] = Some((cell, height));
        height
    })
}

/// Exact barycentric support for the rendered grid's two triangles per cell.
pub(crate) fn height(p: Vec2) -> f32 {
    let cell = p.floor().as_ivec2();
    let f = p - cell.as_vec2();
    let a = vertex_height(cell);
    if f == Vec2::ZERO {
        return a;
    }
    let b = vertex_height(cell + IVec2::X);
    let c = vertex_height(cell + IVec2::Y);
    if f.x + f.y <= 1.0 {
        a + f.x * (b - a) + f.y * (c - a)
    } else {
        let d = vertex_height(cell + IVec2::ONE);
        d + (1.0 - f.y) * (b - d) + (1.0 - f.x) * (c - d)
    }
}

/// Exact rise/run gradient of the same triangle used by height().
pub(crate) fn gradient(p: Vec2) -> Vec2 {
    let cell = p.floor().as_ivec2();
    let f = p - cell.as_vec2();
    let b = vertex_height(cell + IVec2::X);
    let c = vertex_height(cell + IVec2::Y);
    if f.x + f.y <= 1.0 {
        let a = vertex_height(cell);
        Vec2::new(b - a, c - a)
    } else {
        let d = vertex_height(cell + IVec2::ONE);
        Vec2::new(d - c, d - b)
    }
}

/// Height and slope from one support-triangle lookup, with the same arithmetic
/// as the individual queries. Vegetation frequently needs both at one position.
pub(crate) fn height_and_gradient(p: Vec2) -> (f32, Vec2) {
    let cell = p.floor().as_ivec2();
    let f = p - cell.as_vec2();
    let b = vertex_height(cell + IVec2::X);
    let c = vertex_height(cell + IVec2::Y);
    if f.x + f.y <= 1.0 {
        let a = vertex_height(cell);
        let height = if f == Vec2::ZERO {
            a
        } else {
            a + f.x * (b - a) + f.y * (c - a)
        };
        (height, Vec2::new(b - a, c - a))
    } else {
        let d = vertex_height(cell + IVec2::ONE);
        (
            d + (1.0 - f.y) * (b - d) + (1.0 - f.x) * (c - d),
            Vec2::new(d - c, d - b),
        )
    }
}

/// Keep vegetation gravity-upright, but bury the entire root base in the
/// rendered surface. The downhill side, not the center point, sets its datum.
/// Test a footprint in every biome so climate blends cannot admit cliff sites.
pub(crate) fn vegetation_anchor(p: Vec2, radius: f32, max_slope: f32) -> Option<f32> {
    let center = height(p);
    let mut bottom = center;
    for x in [-1.0, 0.0, 1.0] {
        for z in [-1.0, 0.0, 1.0] {
            let q = p + Vec2::new(x, z) * radius;
            let (height, gradient) = height_and_gradient(q);
            if gradient.length_squared() > max_slope * max_slope {
                return None;
            }
            bottom = bottom.min(height);
        }
    }
    // A small seam allowance hides tessellation/numerical gaps on sloping
    // ground. Preserve the exact datum on level ground (including the castle).
    Some(bottom - (center - bottom).min(0.01))
}

const TILE: i32 = 32;
const TERRAIN_JOBS: usize = 8;
const LOD_ERROR: f32 = 0.04;

#[derive(Component)]
pub(crate) struct Grid;

struct TileState {
    entity: Entity,
    mesh: Option<Handle<Mesh>>,
    level: usize,
    stride: usize,
    samples: Option<Arc<TileSamples>>,
}
#[derive(Resource)]
struct TerrainTiles {
    fallback: Handle<Mesh>,
    material: Handle<crate::rendering::sdf::SdfMaterial>,
    loaded: HashMap<IVec2, TileState>,
    pending: HashMap<IVec2, TerrainJob>,
    queue: TileQueue,
    window: Option<(IVec2, i32, i32)>,
    retirement: VecDeque<IVec2>,
    retirement_remaining: usize,
}
/// Queue order and membership have one owner so deferred work cannot drift
/// out of sync with discovery's constant-time duplicate check.
#[derive(Default)]
struct TileQueue {
    cells: VecDeque<(IVec2, usize)>,
    members: HashSet<IVec2>,
}
impl TileQueue {
    fn push_back(&mut self, value: (IVec2, usize)) {
        if self.members.insert(value.0) {
            self.cells.push_back(value);
        }
    }
    fn push_front(&mut self, value: (IVec2, usize)) {
        if self.members.insert(value.0) {
            self.cells.push_front(value);
        }
    }
    fn pop_front(&mut self) -> Option<(IVec2, usize)> {
        let value = self.cells.pop_front()?;
        self.members.remove(&value.0);
        Some(value)
    }
    fn len(&self) -> usize {
        self.cells.len()
    }
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    #[cfg(test)]
    fn iter(&self) -> impl Iterator<Item = &(IVec2, usize)> {
        self.cells.iter()
    }
    fn extend(&mut self, values: impl IntoIterator<Item = (IVec2, usize)>) {
        for value in values {
            self.push_back(value);
        }
    }
}
struct TerrainJob {
    level: usize,
    task: crate::world::streaming::BuildTask<(Mesh, Arc<TileSamples>)>,
}
impl TerrainJob {
    fn new(
        cell: IVec2,
        level: usize,
        samples: Option<Arc<TileSamples>>,
        permit: crate::world::streaming::WorkerPermit,
    ) -> Self {
        Self {
            level,
            task: crate::world::streaming::BuildTask::new(permit, move || {
                let samples = samples.unwrap_or_else(|| Arc::new(TileSamples::new(cell)));
                (samples.mesh(cell, level), samples)
            }),
        }
    }
}

// A level encodes a geometric error budget, not a fixed distance band.
// Quantized powers of two avoid rebuilding for subpixel camera movements.
fn tile_level(cell: IVec2, center: IVec2, detail: i32, focal: f32) -> usize {
    let separation = ((cell - center).abs() - IVec2::ONE).max(IVec2::ZERO);
    let distance = separation.as_vec2().length() * TILE as f32;
    if detail > 0 && distance <= detail as f32 {
        return 1;
    }
    let error = (distance / focal.max(1.0)).max(LOD_ERROR);
    (2.0_f32.powf((error / LOD_ERROR).log2().floor()) as usize).clamp(1, 128)
}

fn fallback_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for z in 0..=TILE {
        for x in 0..=TILE {
            positions.push([x as f32, 0.0, z as f32]);
        }
    }
    let side = (TILE + 1) as u16;
    for z in 0..TILE as u16 {
        for x in 0..TILE as u16 {
            let a = z * side + x;
            indices.extend([a, a + side, a + 1, a + 1, a + side, a + side + 1]);
        }
    }
    let normals = vec![[0.0; 3]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U16(indices))
}

fn sampled_height(p: Vec2) -> f32 {
    crate::world::biome::terrain_for_seed(p, crate::world::biome::world_seed())
        * crate::world::orcs::terrain_clearance(p)
}

fn stride_error(heights: &[f32], stride: usize) -> f32 {
    let mut error = 0.0_f32;
    let side = TILE as usize + 1;
    for z in 0..TILE as usize {
        for x in 0..TILE as usize {
            let bx = x / stride * stride;
            let bz = z / stride * stride;
            let f = Vec2::new((x - bx) as f32, (z - bz) as f32) / stride as f32;
            let a = heights[bz * side + bx];
            let b = heights[bz * side + bx + stride];
            let c = heights[(bz + stride) * side + bx];
            let d = heights[(bz + stride) * side + bx + stride];
            let expected = if f.x + f.y <= 1.0 {
                a + f.x * (b - a) + f.y * (c - a)
            } else {
                d + (1.0 - f.y) * (b - d) + (1.0 - f.x) * (c - d)
            };
            error = error.max((expected - heights[z * side + x]).abs());
        }
    }
    // Include the far edges: adjacent tiles can have a different LOD.
    for i in 0..TILE as usize {
        let base = i / stride * stride;
        let f = (i - base) as f32 / stride as f32;
        for (a, b, v) in [
            (
                heights[TILE as usize * side + base],
                heights[TILE as usize * side + base + stride],
                heights[TILE as usize * side + i],
            ),
            (
                heights[base * side + TILE as usize],
                heights[(base + stride) * side + TILE as usize],
                heights[i * side + TILE as usize],
            ),
        ] {
            error = error.max((a.lerp(b, f) - v).abs());
        }
    }
    error
}

#[cfg(test)]
fn tile_mesh(cell: IVec2, requested: usize) -> Mesh {
    TileSamples::new(cell).mesh(cell, requested)
}

struct TileSamples {
    heights: Vec<f32>,
    errors: [f32; 6],
    normals: Mutex<Vec<Option<[f32; 3]>>>,
}
impl TileSamples {
    fn new(cell: IVec2) -> Self {
        let origin = cell * TILE;
        let mut heights = Vec::with_capacity(((TILE + 1) * (TILE + 1)) as usize);
        for z in 0..=TILE {
            for x in 0..=TILE {
                heights.push(vertex_height(origin + IVec2::new(x, z)));
            }
        }
        let errors = std::array::from_fn(|i| stride_error(&heights, 1 << i));
        let normals = Mutex::new(vec![None; heights.len()]);
        Self {
            heights,
            errors,
            normals,
        }
    }
    fn mesh(&self, cell: IVec2, requested: usize) -> Mesh {
        let origin = cell * TILE;
        let heights = &self.heights;
        let mut cached_normals = self.normals.lock().unwrap();
        let mut stride = requested.min(TILE as usize);
        let error = LOD_ERROR * requested as f32;
        while stride > 1 && self.errors[stride.ilog2() as usize] > error {
            stride /= 2;
        }
        // Fine-grid neighbors share half-metre derivative samples. Evaluate each
        // edge once (2,244 samples per tile instead of 4,356); coarser grids have
        // no shared derivative samples and keep the direct path.
        let mut dx_samples = Vec::new();
        let mut dz_samples = Vec::new();
        if stride == 1 && cached_normals.iter().any(Option::is_none) {
            for z in 0..=TILE {
                dx_samples.push(sampled_height(
                    (origin + IVec2::new(0, z)).as_vec2() - Vec2::X * 0.5,
                ));
                for x in 0..=TILE {
                    dx_samples.push(sampled_height(
                        (origin + IVec2::new(x, z)).as_vec2() + Vec2::X * 0.5,
                    ));
                }
            }
            for x in 0..=TILE {
                dz_samples.push(sampled_height(
                    (origin + IVec2::new(x, 0)).as_vec2() - Vec2::Y * 0.5,
                ));
            }
            for z in 0..=TILE {
                for x in 0..=TILE {
                    dz_samples.push(sampled_height(
                        (origin + IVec2::new(x, z)).as_vec2() + Vec2::Y * 0.5,
                    ));
                }
            }
        }
        let side = (TILE as usize / stride + 1) as u16;
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        for z in (0..=TILE as usize).step_by(stride) {
            for x in (0..=TILE as usize).step_by(stride) {
                let p = (origin + IVec2::new(x as i32, z as i32)).as_vec2();
                positions.push([x as f32, heights[z * (TILE as usize + 1) + x], z as f32]);
                let index = z * (TILE as usize + 1) + x;
                if let Some(normal) = cached_normals[index] {
                    normals.push(normal);
                    continue;
                }
                let (dx, dz) = if stride == 1 {
                    (
                        dx_samples[z * (TILE as usize + 2) + x]
                            - dx_samples[z * (TILE as usize + 2) + x + 1],
                        dz_samples[z * (TILE as usize + 1) + x]
                            - dz_samples[(z + 1) * (TILE as usize + 1) + x],
                    )
                } else {
                    (
                        sampled_height(p - Vec2::X * 0.5) - sampled_height(p + Vec2::X * 0.5),
                        sampled_height(p - Vec2::Y * 0.5) - sampled_height(p + Vec2::Y * 0.5),
                    )
                };
                let normal = Vec3::new(dx, 1.0, dz).normalize().to_array();
                cached_normals[index] = Some(normal);
                normals.push(normal);
            }
        }
        for z in 0..side - 1 {
            for x in 0..side - 1 {
                let a = z * side + x;
                indices.extend([a, a + side, a + 1, a + 1, a + side, a + side + 1]);
            }
        }
        // Skirts cover the error envelopes of independently simplified neighbors.
        // The larger adjacent quantized error is included. Surface vertices keep exact heights; no collision changes.
        for edge in [
            (0..side).collect::<Vec<_>>(),
            (0..side).map(|z| z * side + side - 1).collect(),
            (0..side).rev().map(|x| (side - 1) * side + x).collect(),
            (0..side).rev().map(|z| z * side).collect(),
        ] {
            let base = positions.len() as u16;
            for &i in &edge {
                let mut p = positions[i as usize];
                p[1] -= (error * 4.0).max(0.12);
                positions.push(p);
                normals.push(normals[i as usize]);
            }
            for j in 0..edge.len() - 1 {
                let a = edge[j];
                let b = edge[j + 1];
                let c = base + j as u16;
                let d = c + 1;
                indices.extend([a, b, c, b, d, c]);
            }
        }
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U16(indices))
    }
}

pub(crate) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<crate::rendering::sdf::SdfMaterial>,
    _distance: f32,
) {
    commands.insert_resource(TerrainTiles {
        fallback: meshes.add(fallback_mesh()),
        material,
        loaded: HashMap::new(),
        pending: HashMap::new(),
        queue: TileQueue::default(),
        window: None,
        retirement: VecDeque::new(),
        retirement_remaining: 0,
    });
}

// Bounds can tighten only while the exact GPU-sampled fallback is in use.
// Cache invalidation restores the full envelope before this frame's culling.
fn publish_gpu_bounds(
    mut cache: Option<ResMut<crate::rendering::surface_cache::SurfaceCache>>,
    state: Option<Res<TerrainTiles>>,
    mut bounds: Query<&mut bevy::camera::primitives::Aabb, With<Grid>>,
) {
    let (Some(cache), Some(state)) = (cache.as_mut(), state) else {
        return;
    };
    for (cell, heights) in cache.take_terrain_bounds() {
        if let Some(tile) = state.loaded.get(&cell)
            && tile.mesh.is_none()
            && let Ok(mut aabb) = bounds.get_mut(tile.entity)
        {
            let heights = heights.unwrap_or(Vec2::new(-16.0, 512.0));
            *aabb = bevy::camera::primitives::Aabb::from_min_max(
                Vec3::new(0.0, heights.x, 0.0),
                Vec3::new(TILE as f32, heights.y, TILE as f32),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn resize(
    mut force_fallback: Local<Option<bool>>,
    mut discovery: Local<Option<crate::world::streaming::CellScan>>,
    mut entering: Local<crate::world::streaming::WindowScan>,
    mut retry: Local<Option<IVec2>>,
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    settings: Res<crate::app::settings::GraphicsSettings>,
    view: Res<crate::player::camera::CameraView>,
    mut state: ResMut<TerrainTiles>,
    mut surface: Option<ResMut<crate::rendering::surface_cache::SurfaceCache>>,
    mut meshes: ResMut<Assets<Mesh>>,
    bounds: Query<(&bevy::camera::primitives::Aabb, &GlobalTransform), With<Grid>>,
    cameras: Query<&bevy::camera::primitives::Frustum, With<crate::player::avatar::PlayerCamera>>,
    shadow_frusta: Query<
        &bevy::camera::primitives::CascadesFrusta,
        With<crate::rendering::lighting::Sun>,
    >,
) {
    let focal = coordinator.focal_pixels();
    let center = (view.position.xz() / TILE as f32).floor().as_ivec2();
    let radius = (settings.render_distance / TILE as f32).ceil() as i32 + 1;
    let detail = settings
        .detail_distance
        .min(settings.render_distance)
        .ceil() as i32;
    let window = (center, radius, detail);
    let projection_changed = coordinator.projection_changed();
    if state.window != Some(window) || projection_changed {
        state.window = Some(window);
        state.retirement_remaining = state.retirement.len();
        entering.update(center, radius);
        *discovery = Some(crate::world::streaming::CellScan::new(center, radius));
    }
    // Visit a bounded rotating queue, including bookkeeping and asset removal.
    // Recheck against the latest window so a return teleport cannot retire a
    // tile that has become useful again. New loads add exactly one queue entry.
    let retirement_start = std::time::Instant::now();
    for _ in 0..state.retirement_remaining.min(128) {
        if retirement_start.elapsed().as_micros() >= 500 {
            break;
        }
        state.retirement_remaining -= 1;
        let cell = state.retirement.pop_front().unwrap();
        if (cell - center).abs().max_element() <= radius {
            state.retirement.push_back(cell);
        } else if let Some(tile) = state.loaded.remove(&cell) {
            if let Some(cache) = surface.as_mut() {
                cache.forget_terrain(cell);
            }
            crate::world::streaming::retire(&mut commands, tile.entity);
            if let Some(mesh) = tile.mesh {
                meshes.remove(mesh.id());
            }
        }
    }
    let command_timing = coordinator.time_commands(&mut commands);
    let mut command_count = 0;
    let mut ready = Vec::new();
    let state = &mut *state;
    let loaded = &mut state.loaded;
    state.pending.retain(|cell, job| {
        if !job.task.is_ready() {
            return true;
        }
        if let Some(tile) = loaded.get_mut(cell) {
            tile.samples = Some(job.task.ready_ref().unwrap().1.clone());
        }
        if !loaded.contains_key(cell)
            || (*cell - center).abs().max_element() > radius
            || job.level != tile_level(*cell, center, detail, focal)
        {
            return false;
        }
        // Ready CPU geometry will be admitted without another GPU height bake.
        if let Some(cache) = surface.as_mut() {
            cache.forget_terrain(*cell);
        }
        ready.push(*cell);
        true
    });
    ready.sort_unstable_by_key(|cell| {
        coordinator.visual_rank(
            Vec3::new(
                (cell.x * TILE + TILE / 2) as f32,
                view.position.y,
                (cell.y * TILE + TILE / 2) as f32,
            ),
            24.0,
        )
    });
    for cell in ready {
        let job = state.pending.get_mut(&cell).unwrap();
        let bytes = crate::world::streaming::mesh_bytes(&job.task.ready_ref().unwrap().0);
        let Some(_installation) = coordinator.install(Layer::Terrain, 4, bytes) else {
            break;
        };
        let (mesh, samples) = job.task.take();
        let level = job.level;
        state.pending.remove(&cell);
        if let Some(tile) = state.loaded.get_mut(&cell) {
            // Keep already visible geometry until a replacement is ready.
            use bevy::camera::primitives::MeshAabb;
            let bounds = mesh.compute_aabb().unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                unreachable!()
            };
            let stride = (positions[1][0] - positions[0][0]) as usize;
            command_count += 1;
            let handle = meshes.add(mesh);
            commands
                .entity(tile.entity)
                .insert((Mesh3d(handle.clone()), bounds));
            if let Some(old) = tile.mesh.replace(handle) {
                meshes.remove(old.id());
            }
            if let Some(cache) = surface.as_mut() {
                cache.forget_terrain(cell);
            }
            tile.samples = Some(samples);
            tile.level = level;
            tile.stride = stride;
            if level != tile_level(cell, center, detail, focal) {
                state
                    .queue
                    .push_front((cell, tile_level(cell, center, detail, focal)));
            }
        }
    }
    // Discovery time and entity creation have separate admission costs. Retain
    // unfinished entering strips across movement; only the LOD refresh restarts.
    // A denied spawn keeps its cell for the next frame instead of losing work.
    if let Some(_scan) = coordinator.install(Layer::Terrain, 1, 0) {
        for step in 0..128 {
            let cell = retry.take().or_else(|| {
                if step % 2 == 0 {
                    entering
                        .next()
                        .or_else(|| discovery.as_mut().and_then(Iterator::next))
                } else {
                    discovery
                        .as_mut()
                        .and_then(Iterator::next)
                        .or_else(|| entering.next())
                }
            });
            let Some(cell) = cell else {
                break;
            };
            if (cell - center).abs().max_element() > radius {
                continue;
            }
            let level = tile_level(cell, center, detail, focal);
            if !state.loaded.contains_key(&cell) {
                let Some(_spawn) = coordinator.install_entities(Layer::Terrain, 1, 0, 1) else {
                    *retry = Some(cell);
                    break;
                };
                command_count += 1;
                let entity = commands
                    .spawn((
                        Name::new("Terrain / cached tile"),
                        Grid,
                        bevy::camera::visibility::NoAutoAabb,
                        Mesh3d(state.fallback.clone()),
                        MeshMaterial3d(state.material.clone()),
                        Transform::from_xyz((cell.x * TILE) as f32, 0., (cell.y * TILE) as f32),
                        // Procedural elevation is bounded by < 450m for every seed.
                        bevy::camera::primitives::Aabb::from_min_max(
                            Vec3::new(0., -16., 0.),
                            Vec3::new(TILE as f32, 512., TILE as f32),
                        ),
                    ))
                    .id();
                if let Some(cache) = surface.as_mut() {
                    cache.request_terrain(cell);
                }
                state.retirement.push_back(cell);
                state.loaded.insert(
                    cell,
                    TileState {
                        entity,
                        mesh: None,
                        level: 0,
                        stride: 0,
                        samples: None,
                    },
                );
            }
            let tile = &state.loaded[&cell];
            if (tile.mesh.is_none() || tile.level != level)
                && !state
                    .pending
                    .get(&cell)
                    .is_some_and(|job| job.level == level)
            {
                state.queue.push_back((cell, level));
            }
        }
    }
    command_timing.finish(&mut commands, command_count);
    // Diagnostic comparison: exercise the GPU cache against the original
    // procedural vertex path without CPU meshes hiding either implementation.
    if *force_fallback
        .get_or_insert_with(|| std::env::var_os("HITHER_PROFILE_FORCE_TERRAIN_FALLBACK").is_some())
    {
        return;
    }
    // Obsolete workers still count toward this limit until they finish.
    let mut deferred = Vec::new();
    let frustum = cameras.iter().next();
    // Visibility may change without movement (turning in place). Retain hidden
    // work and inspect a bounded portion of it each frame. Its exact procedural
    // fallback renders immediately while visible tiles get CPU baking priority.
    // Use the 3D camera frustum specifically: the UI camera must not make world
    // tiles eligible for baking merely because they overlap its large bounds.
    let mut candidates = Vec::new();
    let scans = state.queue.len().min(128);
    for _ in 0..scans {
        let Some((cell, _)) = state.queue.pop_front() else {
            break;
        };
        if (cell - center).abs().max_element() > radius {
            continue;
        }
        let level = tile_level(cell, center, detail, focal);
        if state.pending.contains_key(&cell) {
            deferred.push((cell, level));
            continue;
        }
        if state
            .loaded
            .get(&cell)
            .is_none_or(|tile| tile.mesh.is_some() && tile.level == level)
        {
            continue;
        }
        if let Some(frustum) = frustum {
            let entity = state.loaded[&cell].entity;
            if !bounds.get(entity).is_ok_and(|(aabb, transform)| {
                let affine = transform.affine();
                frustum.intersects_obb(aabb, &affine, true, true)
                    || shadow_frusta
                        .iter()
                        .flat_map(|c| c.frusta.values())
                        .flatten()
                        .any(|shadow| shadow.intersects_obb(aabb, &affine, true, true))
            }) {
                deferred.push((cell, level));
                continue;
            }
        }
        candidates.push((cell, level));
    }
    candidates.sort_unstable_by_key(|(cell, _)| {
        (
            state.loaded[cell].mesh.is_some(),
            coordinator.visual_rank(
                Vec3::new(
                    (cell.x * TILE + TILE / 2) as f32,
                    view.position.y,
                    (cell.y * TILE + TILE / 2) as f32,
                ),
                24.0,
            ),
        )
    });
    for (cell, level) in candidates {
        if state.pending.len() >= TERRAIN_JOBS {
            deferred.push((cell, level));
            continue;
        }
        let Some(permit) = coordinator.worker(Layer::Terrain) else {
            deferred.push((cell, level));
            continue;
        };
        if let Some(cache) = surface.as_mut() {
            cache.cpu_terrain_pending(cell);
        }
        state.pending.insert(
            cell,
            TerrainJob::new(cell, level, state.loaded[&cell].samples.clone(), permit),
        );
    }
    state.queue.extend(deferred);
}

#[cfg(test)]
mod tests {
    #[test]
    fn retirement_is_bounded_and_rechecks_return_teleports() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        let mut meshes = Assets::<Mesh>::default();
        let fallback = meshes.add(fallback_mesh());
        let mut loaded = HashMap::new();
        let mut retirement = VecDeque::new();
        for i in 0..400 {
            let cell = IVec2::new(100 + i, 100);
            let entity = app.world_mut().spawn(Transform::default()).id();
            loaded.insert(
                cell,
                TileState {
                    entity,
                    mesh: None,
                    level: 0,
                    stride: 0,
                    samples: None,
                },
            );
            retirement.push_back(cell);
        }
        app.insert_resource(meshes)
            .insert_resource(crate::app::settings::GraphicsSettings {
                render_distance: 16.0,
                ..default()
            })
            .insert_resource(crate::player::camera::CameraRig::default().view())
            .insert_resource(TerrainTiles {
                fallback,
                material: Handle::default(),
                loaded,
                pending: HashMap::new(),
                queue: default(),
                window: None,
                retirement,
                retirement_remaining: 0,
            })
            .add_systems(Update, resize.in_set(Layer::Terrain));
        app.update();
        let state = app.world().resource::<TerrainTiles>();
        let old = state.loaded.keys().filter(|cell| cell.y == 100).count();
        assert!(
            (272..400).contains(&old),
            "only 128 stale tiles may retire in one frame"
        );
        let keep = *state.loaded.keys().find(|cell| cell.y == 100).unwrap();
        let entity = state.loaded[&keep].entity;
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraView>()
            .position = Vec3::new((keep.x * TILE) as f32, 24.0, (keep.y * TILE) as f32);
        for _ in 0..24 {
            app.update();
        }
        let state = app.world().resource::<TerrainTiles>();
        assert_eq!(state.loaded[&keep].entity, entity);
        assert!(
            state
                .loaded
                .keys()
                .all(|cell| (*cell - keep).abs().max_element() <= 2)
        );
        assert_eq!(state.loaded.len(), state.retirement.len());
    }
    #[test]
    fn tile_queue_membership_survives_defer_and_reprioritize() {
        let mut queue = super::TileQueue::default();
        let a = bevy::prelude::IVec2::ZERO;
        let b = bevy::prelude::IVec2::ONE;
        queue.push_back((a, 0));
        queue.push_back((a, 1));
        queue.push_front((b, 2));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pop_front(), Some((b, 2)));
        queue.extend([(b, 3), (a, 4), (b, 5)]);
        assert_eq!(queue.pop_front(), Some((a, 0)));
        assert_eq!(queue.pop_front(), Some((b, 3)));
        assert!(queue.is_empty());
        assert!(queue.members.is_empty());
        queue.push_front((a, 6));
        assert_eq!(queue.pop_front(), Some((a, 6)));
    }
    use super::*;

    #[test]
    fn combined_support_matches_separate_queries_bit_for_bit() {
        for x in -20..=20 {
            for z in -20..=20 {
                for offset in [
                    Vec2::ZERO,
                    Vec2::splat(0.25),
                    Vec2::new(0.2, 0.8),
                    Vec2::splat(0.75),
                ] {
                    let p = Vec2::new(x as f32 * 41.0, z as f32 * 37.0) + offset;
                    let (h, g) = height_and_gradient(p);
                    assert_eq!(h.to_bits(), height(p).to_bits(), "{p:?}");
                    assert_eq!(
                        g.to_array().map(f32::to_bits),
                        gradient(p).to_array().map(f32::to_bits),
                        "{p:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn retained_lod_samples_preserve_geometry_and_normals() {
        let cell = IVec2::new(-87, 143);
        let samples = TileSamples::new(cell);
        for level in [32, 8, 1, 16, 2, 1] {
            let reused = samples.mesh(cell, level);
            let fresh = tile_mesh(cell, level);
            for attribute in [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL] {
                assert_eq!(reused.attribute(attribute), fresh.attribute(attribute));
            }
            assert_eq!(reused.indices(), fresh.indices());
        }
        assert!(samples.normals.lock().unwrap().iter().all(Option::is_some));
    }
    #[test]
    fn continuous_cell_crossings_do_not_restart_missing_terrain() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        let mut meshes = Assets::<Mesh>::default();
        let fallback = meshes.add(fallback_mesh());
        app.insert_resource(meshes)
            .insert_resource(crate::app::settings::GraphicsSettings {
                render_distance: 1024.,
                detail_distance: 24.,
                ..default()
            })
            .insert_resource(crate::player::camera::CameraRig::default().view())
            .insert_resource(TerrainTiles {
                fallback,
                material: Handle::default(),
                loaded: HashMap::new(),
                pending: HashMap::new(),
                queue: TileQueue::default(),
                window: None,
                retirement: VecDeque::new(),
                retirement_remaining: 0,
            })
            .add_systems(Update, resize.in_set(Layer::Terrain));
        let mut previous = 0;
        for frame in 0..100 {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraView>()
                .position
                .x = (frame % 2) as f32 * TILE as f32;
            app.update();
            let loaded = app.world().resource::<TerrainTiles>().loaded.len();
            assert!(
                loaded <= previous + 32,
                "fallback spawning exceeded the frame cap"
            );
            previous = loaded;
        }
        // Restarting a near-to-far scan on every crossing never reaches here.
        assert!(
            app.world()
                .resource::<TerrainTiles>()
                .loaded
                .contains_key(&IVec2::new(6, 0))
        );
    }
    #[test]
    fn projected_error_scales_with_resolution_and_recedes_again() {
        let cell = IVec2::new(16, 0);
        let near = tile_level(cell, cell, 24, 1600.0);
        let distant = tile_level(cell, IVec2::ZERO, 24, 1600.0);
        let low_resolution = tile_level(cell, IVec2::ZERO, 24, 400.0);
        assert_eq!(near, 1);
        assert!(distant > near && low_resolution > distant);
        let distance = 15.0 * TILE as f32;
        assert!(LOD_ERROR * distant as f32 * 1600.0 / distance <= 1.0);
        assert_eq!(tile_level(cell, IVec2::ZERO, 24, 1600.0), distant);
    }
    #[test]
    fn hidden_tiles_keep_fallbacks_and_bake_when_the_camera_turns() {
        use bevy::camera::CameraProjection;
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.add_plugins((
            bevy::asset::AssetPlugin::default(),
            bevy::transform::TransformPlugin,
        ))
        .init_asset::<Mesh>();
        app.add_systems(PostUpdate, bevy::camera::visibility::calculate_bounds);
        let mut meshes = Assets::<Mesh>::default();
        let fallback = meshes.add(fallback_mesh());
        let projection = crate::rendering::view_distance::FinitePerspective::new(128.);
        let camera = app
            .world_mut()
            .spawn((
                crate::player::avatar::PlayerCamera,
                projection.compute_frustum(&GlobalTransform::IDENTITY),
            ))
            .id();
        let view = crate::player::camera::CameraRig::default().view();
        app.insert_resource(meshes)
            .insert_resource(crate::app::settings::GraphicsSettings {
                render_distance: 128.,
                detail_distance: 24.,
                ..default()
            })
            .insert_resource(view)
            .insert_resource(TerrainTiles {
                fallback,
                material: Handle::default(),
                loaded: HashMap::new(),
                pending: HashMap::new(),
                queue: TileQueue::default(),
                window: None,
                retirement: VecDeque::new(),
                retirement_remaining: 0,
            })
            .add_systems(Update, resize.in_set(Layer::Terrain));
        let behind = IVec2::new(0, 2);
        for _ in 0..12 {
            app.update();
        }
        let state = app.world().resource::<TerrainTiles>();
        assert!(state.loaded[&behind].mesh.is_none());
        assert!(!state.pending.contains_key(&behind));
        let behind_entity = state.loaded[&behind].entity;
        let visible = bevy::camera::visibility::ViewVisibility::VISIBLE;
        app.world_mut().entity_mut(behind_entity).insert(visible);
        for _ in 0..4 {
            app.update();
        }
        assert!(
            app.world().resource::<TerrainTiles>().loaded[&behind]
                .mesh
                .is_none()
        );
        let shadow = projection.compute_frustum(&GlobalTransform::from(Transform::from_rotation(
            Quat::from_rotation_y(std::f32::consts::PI),
        )));
        app.world_mut().spawn((
            crate::rendering::lighting::Sun,
            bevy::camera::primitives::CascadesFrusta {
                frusta: [(camera, vec![shadow])].into_iter().collect(),
            },
        ));
        for _ in 0..20 {
            app.update();
        }
        assert!(
            app.world().resource::<TerrainTiles>().loaded[&behind]
                .mesh
                .is_some()
        );
        let state = app.world().resource::<TerrainTiles>();

        assert!(state.loaded[&IVec2::new(0, -2)].mesh.is_some());
        app.world_mut()
            .entity_mut(camera)
            .insert(
                projection.compute_frustum(&GlobalTransform::from(Transform::from_rotation(
                    Quat::from_rotation_y(std::f32::consts::PI),
                ))),
            );
        for _ in 0..20 {
            app.update();
        }
        let state = app.world().resource::<TerrainTiles>();
        assert!(state.loaded[&behind].mesh.is_some());
        assert!(state.pending.len() <= TERRAIN_JOBS);
    }
    #[test]
    fn error_limited_fine_tiles_are_not_rebuilt_when_crossing_lod_bands() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        let mut meshes = Assets::<Mesh>::default();
        let fallback = meshes.add(fallback_mesh());
        let cell = IVec2::new(4, 0);
        let cached = meshes.add(tile_mesh(cell, 1));
        let entity = app.world_mut().spawn(Mesh3d(cached.clone())).id();
        let mut loaded = HashMap::new();
        // A distant LOD request can yield full detail because the terrain's
        // curvature exceeds the allowed error. Approaching must reuse it.
        loaded.insert(
            cell,
            TileState {
                entity,
                mesh: Some(cached.clone()),
                level: 2,
                stride: 1,
                samples: None,
            },
        );
        let mut view = crate::player::camera::CameraRig::default().view();
        view.position.x = 32.0;
        app.insert_resource(meshes)
            .insert_resource(crate::app::settings::GraphicsSettings {
                render_distance: 128.,
                detail_distance: 24.,
                ..default()
            })
            .insert_resource(view)
            .insert_resource(TerrainTiles {
                fallback,
                material: Handle::default(),
                loaded,
                pending: HashMap::new(),
                queue: TileQueue::default(),
                window: Some((IVec2::ZERO, 5, 24)),
                retirement: VecDeque::new(),
                retirement_remaining: 0,
            })
            .add_systems(Update, resize.in_set(Layer::Terrain));
        app.update();
        let state = app.world().resource::<TerrainTiles>();
        assert_eq!(state.loaded[&cell].mesh, Some(cached));
        assert!(!state.pending.contains_key(&cell));
        assert!(state.queue.iter().all(|(queued, _)| *queued != cell));
    }
    #[test]
    fn cached_tiles_bound_lod_error_and_preserve_nearby_support() {
        for cell in [IVec2::ZERO, IVec2::new(6, 2), IVec2::new(-8, -10)] {
            for level in [1, 2, 4, 8] {
                let mesh = tile_mesh(cell, level);
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!()
                };
                let side = p.iter().take_while(|p| p[2] == 0.0).count();
                let stride = TILE as usize / (side - 1);
                assert!(stride <= level);
                for z in 0..=TILE as usize {
                    for x in 0..=TILE as usize {
                        let bx = (x / stride).min(side - 2);
                        let bz = (z / stride).min(side - 2);
                        let f = Vec2::new((x - bx * stride) as f32, (z - bz * stride) as f32)
                            / stride as f32;
                        let a = p[bz * side + bx][1];
                        let b = p[bz * side + bx + 1][1];
                        let c = p[(bz + 1) * side + bx][1];
                        let d = p[(bz + 1) * side + bx + 1][1];
                        let rendered = if f.x + f.y <= 1.0 {
                            a + f.x * (b - a) + f.y * (c - a)
                        } else {
                            d + (1.0 - f.y) * (b - d) + (1.0 - f.x) * (c - d)
                        };
                        let exact =
                            height((cell * TILE + IVec2::new(x as i32, z as i32)).as_vec2());
                        assert!(
                            (rendered - exact).abs() <= LOD_ERROR * level as f32 + 0.0001,
                            "{cell:?} {level} {x} {z}"
                        );
                        if level == 1 {
                            assert!((rendered - exact).abs() < 0.0001);
                        }
                    }
                }
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!()
                };
                for (v, n) in p[..side * side].iter().zip(normals) {
                    let world = (cell * TILE).as_vec2() + Vec2::new(v[0], v[2]);
                    let expected = Vec3::new(
                        sampled_height(world - Vec2::X * 0.5)
                            - sampled_height(world + Vec2::X * 0.5),
                        1.0,
                        sampled_height(world - Vec2::Y * 0.5)
                            - sampled_height(world + Vec2::Y * 0.5),
                    )
                    .normalize();
                    assert!(Vec3::from(*n).distance(expected) < 0.000001);
                }
                use bevy::camera::primitives::MeshAabb;
                let bounds = mesh.compute_aabb().unwrap();
                for vertex in p {
                    let delta = (bevy::math::Vec3A::from(*vertex) - bounds.center).abs();
                    assert!(
                        delta
                            .cmple(bounds.half_extents + bevy::math::Vec3A::splat(0.001))
                            .all()
                    );
                }
            }
        }
    }
    #[test]
    fn terrain_streaming_is_bounded_and_keeps_fallbacks_until_ready() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        let mut meshes = Assets::<Mesh>::default();
        let fallback = meshes.add(fallback_mesh());
        let rig = crate::player::camera::CameraRig::default();
        app.insert_resource(meshes)
            .insert_resource(crate::app::settings::GraphicsSettings {
                render_distance: 16.,
                ..default()
            })
            .insert_resource(rig.view())
            .insert_resource(TerrainTiles {
                fallback,
                material: Handle::default(),
                loaded: HashMap::new(),
                pending: HashMap::new(),
                queue: TileQueue::default(),
                window: None,
                retirement: VecDeque::new(),
                retirement_remaining: 0,
            })
            .add_systems(Update, resize.in_set(Layer::Terrain));
        app.update();
        assert_eq!(app.world().resource::<TerrainTiles>().loaded.len(), 25);
        assert!(
            app.world()
                .resource::<TerrainTiles>()
                .loaded
                .values()
                .all(|t| t.mesh.is_none())
        );
        for _ in 0..8 {
            app.update();
        }
        let tiles = app.world().resource::<TerrainTiles>();
        assert!(tiles.loaded.values().all(|t| t.mesh.is_some()));
        assert!(tiles.pending.is_empty() && tiles.queue.is_empty());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 26);
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraView>()
            .position += Vec3::new(3200., 0., -3200.);
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 26);
        assert_eq!(app.world().resource::<TerrainTiles>().loaded.len(), 25);
        // Shrink while larger-window jobs are outstanding. Eviction runs even
        // when installation credits are exhausted, and old results cannot return.
        app.world_mut()
            .resource_mut::<crate::app::settings::GraphicsSettings>()
            .render_distance = 128.0;
        app.update();
        // A larger window must not create all 121 fallback entities in one frame.
        assert!(app.world().resource::<TerrainTiles>().loaded.len() <= 25 + 32);
        app.world_mut()
            .resource_mut::<crate::app::settings::GraphicsSettings>()
            .render_distance = 16.0;
        app.update();
        assert_eq!(app.world().resource::<TerrainTiles>().loaded.len(), 25);
        for _ in 0..16 {
            app.update();
        }
        assert_eq!(app.world().resource::<TerrainTiles>().loaded.len(), 25);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 26);
    }
    #[test]
    fn vegetation_bases_are_buried_on_slopes_and_cliffs_are_rejected() {
        let (mut grounded, mut rejected) = (0, 0);
        for x in -75..75 {
            for z in -75..75 {
                let p = Vec2::new(x as f32 * 13.7, z as f32 * 13.7);
                let slope = gradient(p).length();
                let anchor = vegetation_anchor(p, 0.6, 0.8);
                if slope > 0.8 {
                    assert!(anchor.is_none(), "cliff accepted at {p}");
                    rejected += 1;
                }
                if let Some(y) = anchor {
                    assert!(y <= height(p));
                    // Probe the circumference independently of the square
                    // samples used for conservative placement.
                    for i in 0..32 {
                        let angle = i as f32 * std::f32::consts::TAU / 32.0;
                        let foot = p + Vec2::new(angle.cos(), angle.sin()) * 0.6;
                        assert!(y <= height(foot) + 0.001, "floating root at {p}");
                    }
                    grounded += usize::from(height(p) - y > 0.03);
                }
            }
        }
        assert!(grounded > 100 && rejected > 100);
        assert_eq!(vegetation_anchor(Vec2::ZERO, 0.6, 0.8), Some(0.0));
    }

    #[test]
    fn support_matches_rendered_triangles_across_negative_coordinates_and_hills() {
        for cell in [
            IVec2::new(192, 80),
            IVec2::new(112, 128),
            IVec2::new(-231, -307),
        ] {
            let a = vertex_height(cell);
            let b = vertex_height(cell + IVec2::X);
            let c = vertex_height(cell + IVec2::Y);
            let d = vertex_height(cell + IVec2::ONE);
            for f in [
                Vec2::new(0.2, 0.3),
                Vec2::new(0.8, 0.7),
                Vec2::new(0.4, 0.6),
            ] {
                let expected = if f.x + f.y <= 1.0 {
                    a * (1.0 - f.x - f.y) + b * f.x + c * f.y
                } else {
                    b * (1.0 - f.y) + c * (1.0 - f.x) + d * (f.x + f.y - 1.0)
                };
                assert!((height(cell.as_vec2() + f) - expected).abs() < 0.0001);
            }
        }
    }

    #[test]
    fn sampling_preserves_relief_and_castle_and_cave_clearings() {
        let mut max_error = 0.0_f32;
        let mut max_mountain_error = 0.0_f32;
        for x in -80..80 {
            for z in -80..80 {
                let p = Vec2::new(x as f32 * 13.17, z as f32 * 13.17);
                let raw =
                    crate::world::biome::terrain_for_seed(p, crate::world::biome::world_seed())
                        * crate::world::orcs::terrain_clearance(p);
                if crate::world::biome::mountain_amount(p) > 0.0 {
                    max_mountain_error = max_mountain_error.max((height(p) - raw).abs());
                } else {
                    max_error = max_error.max((height(p) - raw).abs());
                }
                assert!(height(p).is_finite());
                if p.length() < 10.0 {
                    assert_eq!(height(p), 0.0);
                }
                if crate::world::orcs::entrance_overlap(Vec3::new(p.x, 0.0, p.y), 1.0) {
                    assert_eq!(height(p), 0.0);
                }
            }
        }
        assert!(
            max_mountain_error < 0.65,
            "grid changed alpine relief by {max_mountain_error}m"
        );
        assert!(max_error < 0.08, "grid changed relief by {max_error}m");
    }
}
