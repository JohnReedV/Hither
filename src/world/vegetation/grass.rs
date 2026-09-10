//! Opaque curved grass blades over the infinite SDF soil/grass surface.
//! Bounded, deterministic chunks; one shared PBR material, GPU-only wind.
use crate::world::streaming::{Coordinator, Layer};
use bevy::{
    asset::RenderAssetUsages,
    camera::{primitives::MeshAabb, visibility::VisibilityRange},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use std::collections::{HashMap, VecDeque};

const CHUNK: i32 = 4;
const RADIUS: i32 = 3;
const BUILD_PER_FRAME: usize = 4;
const TUFTS_PER_SIDE: i32 = 32;
const BLADE_SEGMENTS: u32 = 2;
type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassWind>;

#[derive(Component)]
#[require(bevy::camera::visibility::NoAutoAabb)]
struct GrassChunk;

pub struct GrassPlugin;
impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
            .init_resource::<GrassChunks>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (stream.in_set(Layer::Grass), wind)
                    .after(crate::player::camera::update_camera_view),
            )
            .add_systems(
                PostUpdate,
                update_draw_ranges
                    .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            );
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct GrassWind {
    #[uniform(100)]
    // time, player x/z, height above the ground
    motion: Vec4,
    #[uniform(101)]
    detail: Vec4,
    #[uniform(102)]
    camera_position: Vec4,
    #[uniform(103)]
    tramplers: [Vec4; 16],
    #[uniform(104)]
    player_steps: [Vec4; 16],
    #[uniform(105)]
    contact_bounds: Vec4,
}
#[derive(Default)]
struct PlayerTrample {
    contacts: Vec<(Vec2, f32)>,
}
impl PlayerTrample {
    fn update(&mut self, position: Vec2, grounded: bool, dt: f32) -> [Vec4; 16] {
        for (_, age) in &mut self.contacts {
            *age += dt;
        }
        self.contacts.retain(|(_, age)| *age < 1.2);
        if grounded {
            if let Some((_, age)) = self
                .contacts
                .last_mut()
                .filter(|(p, _)| p.distance(position) < 0.2)
            {
                *age = 0.;
            } else {
                if self.contacts.len() == 16 {
                    self.contacts.remove(0);
                }
                self.contacts.push((position, 0.));
            }
        }
        let mut out = [Vec4::ZERO; 16];
        for (slot, (p, age)) in out.iter_mut().zip(&self.contacts) {
            let t = (*age / 1.2).clamp(0., 1.);
            *slot = Vec4::new(p.x, p.y, 0.58, 1. - t * t * (3. - 2. * t));
        }
        out
    }
}
impl MaterialExtension for GrassWind {
    fn vertex_shader() -> ShaderRef {
        "shaders/grass.wgsl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/grass.wgsl".into()
    }
}

#[derive(Resource)]
struct GrassAssets(Handle<GrassMaterial>);
#[derive(Resource, Default)]
struct GrassChunks {
    loaded: HashMap<IVec2, (Entity, Option<Handle<Mesh>>)>,
    pending: HashMap<IVec2, GrassJob>,
    levels: HashMap<IVec2, usize>,
    desired: HashMap<IVec2, usize>,
    queue: VecDeque<IVec2>,
    window: Option<(IVec2, i32, f32)>,
    discovery: crate::world::streaming::WindowScan,
    bands: crate::world::streaming::LodBands,
}

// The deterministic mesher runs off the frame thread. Tests use immediately
// ready jobs so the same queue/eviction/upload logic is checked without timing.
struct GrassJob {
    stride: usize,
    task: crate::world::streaming::BuildTask<Mesh>,
}
impl GrassJob {
    fn new(cell: IVec2, stride: usize, permit: crate::world::streaming::WorkerPermit) -> Self {
        Self {
            stride,
            task: crate::world::streaming::BuildTask::new(permit, move || {
                chunk_mesh_detail(cell, stride)
            }),
        }
    }
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<GrassMaterial>>) {
    commands.insert_resource(GrassAssets(materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.94,
            reflectance: 0.12,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        extension: GrassWind {
            motion: Vec4::ZERO,
            detail: Vec4::splat(24.0),
            camera_position: Vec4::ZERO,
            tramplers: [Vec4::ZERO; 16],
            player_steps: [Vec4::ZERO; 16],
            contact_bounds: Vec4::ZERO,
        },
    })));
}

fn hash(x: i32, z: i32, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9e3779b9) ^ (z as u32).wrapping_mul(0x85ebca6b) ^ salt;
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
    h ^ (h >> 16)
}
fn unit(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    (*seed >> 8) as f32 / 16777216.0
}

// Continuous world-space noise lets stands merge, split and cross chunk
// boundaries. Independent scales add lobes, gaps and ragged tuft-sized edges.
fn patch_noise(point: Vec2, seed: u32) -> f32 {
    let cell = point.floor().as_ivec2();
    let t = point - cell.as_vec2();
    let t = t * t * (Vec2::splat(3.0) - 2.0 * t);
    let sample = |x, z| (hash(x, z, seed) >> 8) as f32 / 16_777_216.0;
    let a = sample(cell.x, cell.y);
    let b = sample(cell.x + 1, cell.y);
    let c = sample(cell.x, cell.y + 1);
    let d = sample(cell.x + 1, cell.y + 1);
    (a + (b - a) * t.x) * (1.0 - t.y) + (c + (d - c) * t.x) * t.y
}

fn grass_density(point: Vec2, world_seed: u32) -> f32 {
    let warped = point
        + 6.0
            * (Vec2::new(
                patch_noise(point / 17.0, world_seed ^ 0x77617270),
                patch_noise(point / 17.0, world_seed ^ 0x77617271),
            ) - Vec2::splat(0.5));
    let rotated = Vec2::new(
        warped.x * 0.8 + warped.y * 0.6,
        -warped.x * 0.6 + warped.y * 0.8,
    );
    let growth = 0.55 * patch_noise(warped / 9.0, world_seed ^ 0x70617463)
        + 0.27 * patch_noise(rotated / 3.1, world_seed ^ 0x70617464)
        + 0.13 * patch_noise(warped / 1.05, world_seed ^ 0x70617465)
        + 0.05 * patch_noise(warped / 0.29, world_seed ^ 0x70617466);
    // Broad growing conditions fade gradually into bare ground. Sub-metre
    // colonies vary density within each stand instead of filling a silhouette.
    let vigor = crate::world::biome::smooth_range(0.5225, 0.70, growth);
    let colony = crate::world::biome::smooth_range(
        0.20,
        0.70,
        patch_noise(point / 0.65, world_seed ^ 0x726f6f74),
    );
    // Calibrated mean occupancy is about 15%; fringes occupy more area than
    // their equivalent full-density cover, with most blades in the cores.
    vigor * (0.20 + 0.80 * colony)
}

// Gather nearby roots into irregular bunches without changing the site budget.
// Centers live in world space, so a bunch can cross a streaming boundary.
fn bunch_root(p: Vec2, seed: u32) -> Vec2 {
    let cell = (p / 0.8).floor().as_ivec2();
    let mut best = 1.0;
    let mut center = p;
    for x in -1..=1 {
        for z in -1..=1 {
            let c = cell + IVec2::new(x, z);
            let mut rng = hash(c.x, c.y, seed ^ 0x62756e63);
            if unit(&mut rng) < 0.22 {
                continue;
            }
            let candidate = (c.as_vec2() + Vec2::new(unit(&mut rng), unit(&mut rng))) * 0.8;
            let radius = 0.30 + 0.30 * unit(&mut rng);
            let distance = p.distance(candidate) / radius;
            if distance < best {
                best = distance;
                center = candidate;
            }
        }
    }
    let gather = 0.72 * (1.0 - crate::world::biome::smooth_range(0.0, 1.0, best));
    p.lerp(center, gather)
}

fn clearing_density(density: f32, forest: f32, stand: f32) -> f32 {
    let opening = crate::world::biome::smooth_range(0.55, 0.9, forest)
        * (1.0 - crate::world::biome::smooth_range(0.08, 0.60, stand));
    density.max(opening * 0.70)
}

#[derive(Clone, Copy)]
enum GrassVariant {
    Regular,
    Tall,
}

impl GrassVariant {
    fn height_scale(self) -> f32 {
        match self {
            Self::Regular => 1.0,
            Self::Tall => 2.0,
        }
    }
}

fn tall_chance(density: f32) -> f32 {
    // Lush patch interiors (including forest clearings) favor tall crowns;
    // sparse fringes keep occasional tall tufts without forming a hard ring.
    0.03 + 0.67 * crate::world::biome::smooth_range(0.15, 0.75, density)
}

fn extra_site(gx: i32, gz: i32, seed: u32) -> bool {
    let mut rng = hash(gx, gz, seed ^ 0x65787472);
    unit(&mut rng) < 0.40
}

fn allowed(p: Vec3) -> bool {
    // Leave the existing foundation and roots exposed. Grass is visual only;
    // the level's flat walk/jump collision surface remains unchanged.
    !crate::player::movement::player_collides(
        p + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
    ) && !crate::world::orcs::entrance_overlap(p, 0.0)
}

fn winter_color(green: Vec3, t: f32, climate: f32) -> Vec3 {
    // Dormant straw first, then frost on exposed tips; never green in white snow.
    let dormant = (climate / 0.08).clamp(0.0, 1.0);
    let straw = Vec3::new(0.045, 0.039, 0.028).lerp(Vec3::new(0.19, 0.16, 0.11), t);
    let frost = ((climate - 0.04) / 0.36).clamp(0.0, 1.0) * t.powf(0.65);
    green
        .lerp(straw, dormant)
        .lerp(Vec3::new(0.46, 0.52, 0.57), frost)
}

#[cfg(test)]
fn chunk_mesh(cell: IVec2) -> Mesh {
    chunk_mesh_detail(cell, 1)
}
fn chunk_mesh_detail(cell: IVec2, stride: usize) -> Mesh {
    build_chunk_mesh(cell, stride, true)
}

fn build_chunk_mesh(cell: IVec2, stride: usize, tall_grass: bool) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    let origin = Vec3::new((cell.x * CHUNK) as f32, 0.0, (cell.y * CHUNK) as f32);
    let world_seed = crate::world::biome::shader_seed().x;
    // A second independent population at 40% preserves the requested increase
    // even where acceptance probability is already one (no clamped multiplier).
    for population in 0..2u32 {
        for x in 0..TUFTS_PER_SIDE {
            for z in 0..TUFTS_PER_SIDE {
                let gx = cell.x * TUFTS_PER_SIDE + x;
                let gz = cell.y * TUFTS_PER_SIDE + z;
                if population == 1 && !extra_site(gx, gz, world_seed) {
                    continue;
                }
                let salt = population.wrapping_mul(0x9e3779b9);
                let mut seed = hash(gx, gz, 0x67726173 ^ world_seed ^ salt);
                // Beyond individual tuft resolution, one representative covers a
                // group of sites. Preserve seeded placement at surviving sites.
                let group = (stride / 4).max(1);
                if !(x as usize).is_multiple_of(group) || !(z as usize).is_multiple_of(group) {
                    continue;
                }
                let mut rank_seed = hash(gx, gz, 0x72616e6b ^ salt);
                let rank = unit(&mut rank_seed);
                let base = Vec3::new(
                    (x as f32 + unit(&mut seed)) * CHUNK as f32 / TUFTS_PER_SIDE as f32,
                    0.0,
                    (z as f32 + unit(&mut seed)) * CHUNK as f32 / TUFTS_PER_SIDE as f32,
                );
                let site = (origin + base).xz();
                let root = bunch_root(site, world_seed);
                let base = Vec3::new(root.x - origin.x, 0.0, root.y - origin.z);
                let world = origin + base;
                let forest = crate::world::biome::forest_amount(root);
                let stand = crate::world::biome::stand_density(root);
                let density = clearing_density(grass_density(site, world_seed), forest, stand);
                // A separate site roll preserves every tuft's existing random
                // sequence and keeps surviving roots identical at every LOD.
                let mut growth_seed = hash(gx, gz, 0x67726f77 ^ world_seed ^ salt);
                if unit(&mut growth_seed) >= density {
                    continue;
                }
                if crate::world::terrain::vegetation_anchor(world.xz(), 0.08, 1.0).is_none() {
                    continue;
                }
                if rank > crate::world::biome::mountain_habitat(world.xz()) {
                    continue;
                }
                // Test the actual hillside elevation, including embedded rock models.
                if !allowed(world.with_y(crate::world::terrain::height(world.xz()))) {
                    continue;
                }
                let snow = crate::world::biome::surface_snow(world.xz());
                let woodland = forest * stand;
                // Woodland floor is mostly leaf litter with broken patches of grass.
                // Keep the courtyard lawn, even when the castle is in a forest.
                if (world.x.abs() > 6.0 || world.z.abs() > 6.0) && unit(&mut seed) < woodland * 0.72
                {
                    continue;
                }
                // Blades return in exposed patches as the rendered snow retreats.
                let burial = crate::world::biome::smooth_range(0.12, 0.95, snow);
                if unit(&mut seed) < burial {
                    continue;
                }
                let path = (world.x.abs() < 0.72 && world.z > 0.6 && world.z < 8.0)
                    || (world.xz().length() - 1.10).abs() < 0.20;
                let sparse = unit(&mut seed);
                if sparse < if path { 0.75 } else { 0.08 } {
                    continue;
                }
                let patch = unit(&mut seed);
                let blade_count = 5 + (seed % 3);
                // Independent of blade randomness and LOD: the entire crown
                // keeps its variant when streamed or simplified.
                let mut variant_seed = hash(gx, gz, world_seed ^ salt ^ 0x74616c6c);
                let variant = if tall_grass && unit(&mut variant_seed) < tall_chance(density) {
                    GrassVariant::Tall
                } else {
                    GrassVariant::Regular
                };
                let crown_angle = patch * std::f32::consts::TAU;
                let crown_height = 0.13 + 0.10 * unit(&mut seed);
                let crown_tint = patch_noise(root / 1.3, world_seed ^ 0x74696e74);
                let segments = BLADE_SEGMENTS;
                for blade in 0..blade_count {
                    let angle = crown_angle + blade as f32 * 2.3999631 + unit(&mut seed) * 0.35;
                    let lean_amount = 0.025 + unit(&mut seed) * 0.065;
                    let root_offset = 0.004 + unit(&mut seed) * 0.010;
                    let height = crown_height
                        * (0.68 + unit(&mut seed) * 0.32)
                        * if path { 0.6 } else { 1.0 }
                        * (1.0 - burial * 0.75)
                        * (0.70 + 0.30 * density.sqrt())
                        * variant.height_scale();
                    let width = 0.004 + unit(&mut seed) * 0.004;
                    let dry = unit(&mut seed) < 0.07;
                    // Keep a three-blade fan at lower LODs. Every surviving
                    // blade retains its full curve, width, height and root;
                    // never replace the crown with an inflated triangle.
                    if stride != 1 && blade >= 3 {
                        continue;
                    }
                    let outward = Vec3::new(angle.cos(), 0.0, angle.sin());
                    let side = Vec3::new(-outward.z, 0.0, outward.x);
                    let lean = outward * lean_amount;
                    let root = base + outward * root_offset;
                    let root_xz = (origin + root).xz();
                    let (ground_height, gradient) =
                        crate::world::terrain::height_and_gradient(root_xz);
                    let ground = ground_height - gradient.length() * 0.025;
                    let root = root.with_y(ground);
                    let root_color = Vec3::new(0.012, 0.025, 0.004);
                    let tip_color = if dry {
                        Vec3::new(0.16, 0.13, 0.045)
                    } else {
                        Vec3::new(0.065 + crown_tint * 0.025, 0.12 + crown_tint * 0.040, 0.014)
                    };
                    let first = positions.len() as u32;
                    for row in 0..=segments {
                        let t = row as f32 / segments as f32;
                        let center = root + Vec3::Y * height * t + lean * t * t;
                        let tangent = (Vec3::Y * height + lean * (2.0 * t)).normalize();
                        let normal = (side.cross(tangent) + Vec3::Y * 0.65).normalize();
                        for col in 0..if row == segments { 1 } else { 2 } {
                            positions.push(
                                (center
                                    + side
                                        * width
                                        * (1.0 - t).powf(0.7)
                                        * (col as f32 * 2.0 - 1.0))
                                    .to_array(),
                            );
                            normals.push(normal.to_array());
                            // No texture UV is needed by this material. X carries
                            // a stable tuft identifier, Y is blade height.
                            uvs.push([rank, t]);
                            colors.push(
                                winter_color(root_color.lerp(tip_color, t.sqrt()), t, snow)
                                    .extend(root.y) // Alpha carries the blade root datum to the vertex shader.
                                    .to_array(),
                            );
                        }
                        if row > 0 {
                            let a = first + (row - 1) * 2;
                            indices.extend([a, a + 1, a + 2]);
                            if row < segments {
                                indices.extend([a + 1, a + 3, a + 2]);
                            }
                        }
                    }
                }
            }
        }
    }
    let indices = if positions.len() <= u16::MAX as usize {
        Indices::U16(indices.into_iter().map(|i| i as u16).collect())
    } else {
        Indices::U32(indices)
    };
    Mesh::new(
        PrimitiveTopology::TriangleList,
        // Discard the CPU vertex copy once uploaded; streaming retains only
        // the handle and can deterministically rebuild a revisited chunk.
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(indices)
}

#[cfg(test)]
fn grass_stride(cell: IVec2, center: IVec2) -> usize {
    grass_stride_at(cell, center, 36.0, None)
}
fn grass_stride_at(cell: IVec2, center: IVec2, detail: f32, previous: Option<usize>) -> usize {
    let separation = ((cell - center).abs() - IVec2::ONE).max(IVec2::ZERO);
    let distance = separation.as_vec2().length() * CHUNK as f32;
    let threshold = detail * (16.0 / 36.0) + 4.0;
    // Switch between the complete crown and its three-blade fan, retaining
    // the previous choice within a four-metre hysteresis band.
    let hysteresis = match previous {
        Some(1) => 2.0,
        Some(_) => -2.0,
        None => 0.0,
    };
    if distance < threshold + hysteresis {
        1
    } else {
        4
    }
}

fn projected_grass_stride(
    cell: IVec2,
    center: IVec2,
    detail: f32,
    previous: Option<usize>,
    focal: f32,
) -> usize {
    let distance = (((cell - center).abs() - IVec2::ONE).max(IVec2::ZERO))
        .as_vec2()
        .length()
        * CHUNK as f32;
    let site_pixels = CHUNK as f32 / TUFTS_PER_SIDE as f32 * focal / distance.max(1.0);
    if site_pixels < 0.5 {
        16
    } else if site_pixels < 1.0 {
        8
    } else {
        grass_stride_at(cell, center, detail.min(focal * 0.025), previous)
    }
}

#[allow(clippy::too_many_arguments)]
fn stream(
    mut coordinator: ResMut<Coordinator>,
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    range: Option<Res<crate::rendering::view_distance::Range>>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    assets: Res<GrassAssets>,
    mut state: ResMut<GrassChunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    settings: Option<Res<crate::app::settings::GraphicsSettings>>,
) {
    let radius = range.as_ref().map_or(RADIUS, |r| {
        let load = match (detail.as_ref(), settings.as_ref()) {
            (Some(d), Some(s)) => detail_load_radius(r.load, d.0, s.render_distance),
            _ => r.load.min(detail.as_ref().map_or(r.load, |d| d.0 + 4.0)),
        };
        (load / CHUNK as f32).ceil() as i32 + 1
    });
    if settings.is_some_and(|settings| !settings.grass_blades)
        || detail.as_ref().is_some_and(|d| d.0 <= 0.0)
    {
        for (_, (entity, mesh)) in state.loaded.drain() {
            crate::world::streaming::retire(&mut commands, entity);
            if let Some(mesh) = mesh {
                meshes.remove(mesh.id());
            }
        }
        // Drain existing workers without starting any more while disabled.
        state.pending.retain(|_, job| !job.task.is_ready());
        state.levels.clear();
        state.desired.clear();
        state.queue.clear();
        state.window = None;
        state.discovery = default();
        state.bands.clear();
        return;
    }
    let center = IVec2::new(
        (rig.position.x / CHUNK as f32).floor() as i32,
        (rig.position.z / CHUNK as f32).floor() as i32,
    );
    let detail_limit = detail.as_ref().map_or(36.0, |d| d.0);
    let window = (center, radius, detail_limit);
    if state.window != Some(window) || coordinator.projection_changed() {
        let previous = state.window.replace(window);
        if let Some((old, old_radius, _)) = previous {
            // Reverse the rectangle difference to visit only leaving strips.
            for cell in
                crate::world::streaming::entering_cells(old, old_radius, Some((center, radius)))
            {
                if let Some((entity, mesh)) = state.loaded.remove(&cell) {
                    crate::world::streaming::retire(&mut commands, entity);
                    if let Some(mesh) = mesh {
                        meshes.remove(mesh.id());
                    }
                }
                state.levels.remove(&cell);
                state.desired.remove(&cell);
            }
        }
        state
            .queue
            .retain(|c| (*c - center).abs().max_element() <= radius);
        state.discovery.update(center, radius);
        if previous.is_none_or(|(old, r, d)| {
            d != detail_limit || r != radius || (old - center).abs().max_element() > radius
        }) || coordinator.projection_changed()
        {
            state.bands.clear();
            state.bands.push(
                crate::world::streaming::CellScan::new(center, radius),
                center,
                radius,
            );
        } else if let Some((old, _, _)) = previous {
            let pad = (old - center).as_vec2().length() + 3.0;
            let focal = coordinator.focal_pixels();
            for boundary in [
                focal * 0.125,
                focal * 0.25,
                detail_limit.min(focal * 0.025) * (16.0 / 36.0) + 2.0,
                detail_limit.min(focal * 0.025) * (16.0 / 36.0) + 6.0,
            ] {
                let r = boundary / CHUNK as f32;
                if r - pad <= radius as f32 * std::f32::consts::SQRT_2 {
                    state.bands.push(
                        crate::world::streaming::distance_band(center, r - pad, r + pad),
                        center,
                        radius,
                    );
                }
            }
        }
    }
    if state.queue.len() < 128
        && let Some(_scope) = coordinator.install(Layer::Grass, 1, 0)
    {
        for i in 0..128 {
            let cell = if i % 2 == 0 {
                state.discovery.next().or_else(|| state.bands.next())
            } else {
                state.bands.next().or_else(|| state.discovery.next())
            };
            let Some(cell) = cell else {
                break;
            };
            if (cell - center).abs().max_element() > radius {
                continue;
            }
            let stride = projected_grass_stride(
                cell,
                center,
                detail_limit,
                state.levels.get(&cell).copied(),
                coordinator.focal_pixels(),
            );
            state.desired.insert(cell, stride);
            if state.levels.get(&cell).copied() != Some(stride) && !state.queue.contains(&cell) {
                state.queue.push_back(cell);
            }
        }
    }
    // Steady frames only service the bounded queue/workers, with no window
    // scan or sort. Keep obsolete workers counted until they finish.
    let queued = state.queue.len();
    for _ in 0..queued {
        if state.pending.len() >= BUILD_PER_FRAME {
            break;
        }
        let Some(cell) = state.queue.pop_front() else {
            break;
        };
        if state.pending.contains_key(&cell) {
            state.queue.push_back(cell);
            continue;
        }
        let stride = projected_grass_stride(
            cell,
            center,
            detail_limit,
            state.levels.get(&cell).copied(),
            coordinator.focal_pixels(),
        );
        state.desired.insert(cell, stride);
        if state.levels.get(&cell).copied() == Some(stride) {
            continue;
        }
        let Some(permit) = coordinator.worker(Layer::Grass) else {
            state.queue.push_front(cell);
            break;
        };
        state
            .pending
            .insert(cell, GrassJob::new(cell, stride, permit));
    }
    let mut ready = Vec::new();
    let state = &mut *state;
    let desired = &state.desired;
    let mut rejected = Vec::new();
    let mut completed = Vec::new();
    for (cell, task) in &mut state.pending {
        if task.task.is_ready() {
            if (*cell - center).abs().max_element() <= radius
                && desired.get(cell).copied() == Some(task.stride)
            {
                ready.push((*cell, task.stride));
                continue;
            } else if desired.contains_key(cell) {
                rejected.push(*cell);
            }
            completed.push(*cell);
        }
    }
    for cell in completed {
        state.pending.remove(&cell);
    }
    state.queue.extend(rejected);
    ready.sort_unstable_by_key(|(cell, _)| {
        coordinator.visual_rank(
            Vec3::new(
                (cell.x * CHUNK) as f32,
                rig.position.y,
                (cell.y * CHUNK) as f32,
            ),
            3.0,
        )
    });
    for (cell, stride) in ready {
        let task = &mut state.pending.get_mut(&cell).unwrap().task;
        let bytes = crate::world::streaming::mesh_bytes(task.ready_ref().unwrap());
        let Some(_installation) = coordinator.install_entities(
            Layer::Grass,
            4,
            bytes,
            usize::from(!state.loaded.contains_key(&cell)),
        ) else {
            break;
        };
        let geometry = task.take();
        state.pending.remove(&cell);
        // Reuse the cell entity so LOD replacement does not churn ECS/render IDs.
        let existing = state.loaded.remove(&cell);
        if let Some((_, Some(mesh))) = &existing {
            meshes.remove(mesh.id());
        }
        state.levels.insert(cell, stride);
        if geometry.count_vertices() == 0 {
            // The level map records empty results without allocating an ECS entity.
            if let Some((entity, _)) = existing {
                crate::world::streaming::retire(&mut commands, entity);
            }
            continue;
        }
        let entity = existing
            .map(|(e, _)| e)
            .unwrap_or_else(|| commands.spawn_empty().id());
        // GPU occlusion tests must include wind and trampling displacement.
        // grass.wgsl can move tips by < 0.16 m horizontally and sink roots 0.015 m.
        let mut bounds = geometry.compute_aabb().expect("nonempty grass chunk");
        bounds.half_extents += bevy::math::Vec3A::splat(0.2);
        let mesh = meshes.add(geometry);
        commands.entity(entity).insert((
            Name::new("Grass / curved blades"),
            GrassChunk,
            draw_range(&bounds, detail.as_ref().map_or(24.0, |d| d.0)),
            Mesh3d(mesh.clone()),
            bounds,
            MeshMaterial3d(assets.0.clone()),
            NotShadowCaster,
            Transform::from_xyz((cell.x * CHUNK) as f32, 0.0, (cell.y * CHUNK) as f32),
        ));
        state.loaded.insert(cell, (entity, Some(mesh)));
    }
}

fn draw_range(bounds: &bevy::camera::primitives::Aabb, detail: f32) -> VisibilityRange {
    // The shader fades by vertex distance. Expand the center-distance cutoff
    // by the entire chunk radius, so no partially visible blade is rejected.
    // Bevy retains distinct range indices for the lifetime of the app. Round
    // outward to whole metres so changing mesh bounds cannot grow that table
    // by one entry per chunk/LOD during an unbounded journey.
    let end = (detail + bounds.half_extents.length()).ceil();
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: end..end,
        use_aabb: true,
    }
}

fn update_draw_ranges(
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    mut previous: Local<Option<f32>>,
    mut chunks: Query<(&bevy::camera::primitives::Aabb, &mut VisibilityRange), With<GrassChunk>>,
) {
    let detail = detail.as_ref().map_or(24.0, |d| d.0);
    if previous.replace(detail) == Some(detail) {
        return;
    }
    for (bounds, mut range) in &mut chunks {
        *range = draw_range(bounds, detail);
    }
}

// Reduce visible detail without shrinking the independently computed camera offset.
fn detail_load_radius(render_load: f32, detail: f32, render: f32) -> f32 {
    detail.min(render).max(0.0) + (render_load - render).max(0.0)
}

#[allow(clippy::too_many_arguments)]
fn wind(
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    view: Option<Res<crate::player::camera::CameraView>>,
    time: Res<Time>,
    game: Res<crate::app::GameState>,
    rig: Res<crate::player::camera::CameraRig>,
    assets: Res<GrassAssets>,
    mut materials: ResMut<Assets<GrassMaterial>>,
    mut elapsed: Local<f32>,
    trampling: Option<Res<crate::world::orcs::Trampling>>,
    mut player_trample: Local<PlayerTrample>,
) {
    if !game.paused {
        *elapsed += time.delta_secs();
    }
    if let Some(mut material) = materials.get_mut(&assets.0) {
        material.extension.detail = Vec4::splat(detail.as_ref().map_or(24.0, |d| d.0));
        if let Some(view) = view {
            material.extension.camera_position = view.position.extend(0.0);
        }
        if game.paused {
            return;
        }
        material.extension.tramplers = trampling.as_ref().map_or([Vec4::ZERO; 16], |t| t.0);
        material.extension.player_steps = player_trample.update(
            rig.position.xz(),
            !rig.is_spectating() && rig.ground_influence_height() < 0.1,
            time.delta_secs(),
        );
        material.extension.contact_bounds = contact_bounds(
            &material.extension.player_steps,
            &material.extension.tramplers,
        );
        material.extension.motion = Vec4::new(
            *elapsed,
            rig.position.x,
            rig.position.z,
            rig.ground_influence_height(),
        );
    }
}

fn contact_bounds(player: &[Vec4; 16], orcs: &[Vec4; 16]) -> Vec4 {
    let mut low = Vec2::splat(f32::INFINITY);
    let mut high = Vec2::splat(f32::NEG_INFINITY);
    for c in player.iter().chain(orcs).filter(|c| c.w > 0.0) {
        low = low.min(c.xy() - Vec2::splat(c.z));
        high = high.max(c.xy() + Vec2::splat(c.z));
    }
    Vec4::new(low.x, low.y, high.x, high.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_replacement_preserves_authored_wind_bounds() {
        use bevy::camera::{primitives::Aabb, visibility::calculate_bounds};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .add_systems(Update, calculate_bounds);
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::default());
        let padded = Aabb::from_min_max(Vec3::splat(-0.7), Vec3::splat(0.7));
        let grass = app
            .world_mut()
            .spawn((GrassChunk, Mesh3d(mesh.clone()), padded))
            .id();
        let control = app.world_mut().spawn((Mesh3d(mesh), padded)).id();
        app.update();
        let bounds = app.world().get::<Aabb>(grass).unwrap();
        assert_eq!(bounds.half_extents, padded.half_extents);
        assert_ne!(
            app.world().get::<Aabb>(control).unwrap().half_extents,
            padded.half_extents
        );
        // A LOD replacement installs its newly computed padded bounds. The
        // engine must retain those too, rather than rescan the unbent mesh.
        let replacement = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(2., 1., 2.));
        let padded = Aabb::from_min_max(Vec3::new(-1.2, -0.7, -1.2), Vec3::new(1.2, 0.7, 1.2));
        app.world_mut()
            .entity_mut(grass)
            .insert((Mesh3d(replacement.clone()), padded));
        app.world_mut()
            .entity_mut(control)
            .insert(Mesh3d(replacement));
        app.update();
        let bounds = app.world().get::<Aabb>(grass).unwrap();
        assert_eq!(bounds.center, padded.center);
        assert_eq!(bounds.half_extents, padded.half_extents);
        assert_ne!(
            app.world().get::<Aabb>(control).unwrap().half_extents,
            padded.half_extents
        );
    }
    #[test]
    fn draw_range_preserves_every_partially_visible_blade() {
        use bevy::camera::primitives::Aabb;
        let bounds = Aabb::from_min_max(Vec3::new(-0.2, 50.0, -0.2), Vec3::new(4.2, 54.0, 4.2));
        let center = Vec3::from(bounds.center);
        let range = draw_range(&bounds, 35.0);
        assert!(range.use_aabb);
        assert_eq!(range.end_margin.start, range.end_margin.end);
        assert_eq!(range.end_margin.end.fract(), 0.0);
        let mut catalogue = std::collections::HashSet::new();
        for i in 0..10000 {
            let bounds =
                Aabb::from_min_max(Vec3::ZERO, Vec3::new(4.4, i as f32 / 10000.0 * 528.0, 4.4));
            catalogue.insert(draw_range(&bounds, 16.0 + (i as f32 * 0.137) % 1008.0));
        }
        assert!(catalogue.len() < 1400);
        for direction in [
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::ONE.normalize(),
            -Vec3::ONE.normalize(),
        ] {
            let camera = center + direction * range.end_margin.end;
            for x in [-1.0, 0.0, 1.0] {
                for y in [-1.0, 0.0, 1.0] {
                    for z in [-1.0, 0.0, 1.0] {
                        let vertex = center + Vec3::from(bounds.half_extents) * Vec3::new(x, y, z);
                        assert!(camera.distance(vertex) >= 35.0 - 0.00001);
                    }
                }
            }
        }
        let mut app = App::new();
        app.insert_resource(crate::rendering::view_distance::Detail(35.0));
        app.add_systems(Update, update_draw_ranges);
        let entity = app.world_mut().spawn((GrassChunk, bounds, range)).id();
        app.update();
        app.world_mut()
            .resource_mut::<crate::rendering::view_distance::Detail>()
            .0 = 16.0;
        app.update();
        let actual = app.world().get::<VisibilityRange>(entity).unwrap();
        assert!(*actual == draw_range(app.world().get::<Aabb>(entity).unwrap(), 16.0));
    }
    #[test]
    fn contact_broad_phase_contains_all_active_disks() {
        let mut player = [Vec4::ZERO; 16];
        let mut orcs = player;
        player[0] = Vec4::new(-7.0, 12.0, 0.58, 0.4);
        orcs[0] = Vec4::new(20.0, -18.0, 1.5, 1.0);
        let bounds = contact_bounds(&player, &orcs);
        for c in [player[0], orcs[0]] {
            for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                let p = c.xy() + d * c.z;
                assert!(p.cmpge(bounds.xy()).all() && p.cmple(bounds.zw()).all());
            }
        }
        let empty = contact_bounds(&[Vec4::ZERO; 16], &[Vec4::ZERO; 16]);
        assert!(!Vec2::ZERO.cmpge(empty.xy()).all());
    }
    #[test]
    fn grass_lod_retains_its_level_inside_the_hysteresis_band() {
        let boundary = IVec2::new(6, 0);
        assert_eq!(grass_stride_at(boundary, IVec2::ZERO, 36., Some(1)), 1);
        assert_eq!(grass_stride_at(boundary, IVec2::ZERO, 36., Some(4)), 4);
        assert_eq!(
            grass_stride_at(IVec2::new(5, 0), IVec2::ZERO, 36., Some(4)),
            1
        );
        assert_eq!(
            grass_stride_at(IVec2::new(7, 0), IVec2::ZERO, 36., Some(1)),
            4
        );
    }
    #[test]
    fn player_trampling_leaves_a_bounded_smoothly_recovering_trail() {
        let mut trail = PlayerTrample::default();
        trail.update(Vec2::ZERO, true, 0.016);
        for i in 1..10 {
            trail.update(Vec2::new(i as f32 * 0.06, 0.), true, 0.016);
        }
        assert!(trail.contacts.len() > 1);
        assert!(trail.contacts.len() <= 16);
        let before = trail.update(Vec2::ZERO, false, 0.0)[0].w;
        let after = trail.update(Vec2::ZERO, false, 0.2)[0].w;
        assert!(after > 0. && after < before);
        assert_eq!(trail.update(Vec2::ZERO, false, 1.3), [Vec4::ZERO; 16]);
    }
    #[test]
    fn distant_meshes_cluster_unresolved_tuft_sites() {
        let cell = (2..100)
            .map(|x| IVec2::new(x, 2))
            .find(|cell| chunk_mesh_detail(*cell, 1).count_vertices() > 0)
            .expect("a grassy cell outside the castle");
        let full = chunk_mesh_detail(cell, 1);
        let medium = chunk_mesh_detail(cell, 4);
        let far = chunk_mesh_detail(cell, 8);
        let sites = |mesh: &Mesh| {
            let bevy::mesh::VertexAttributeValues::Float32x2(uvs) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
            else {
                panic!()
            };
            uvs.iter()
                .map(|uv| uv[0].to_bits())
                .collect::<std::collections::HashSet<_>>()
        };
        for simplified in [&medium, &far] {
            assert!(sites(simplified).is_subset(&sites(&full)));
            assert!(simplified.count_vertices() <= full.count_vertices() * 3 / 5);
        }
        assert!(sites(&far).len() < sites(&medium).len());
        assert!(far.count_vertices() < medium.count_vertices() / 2);
        for x in -20..=20 {
            for z in -20..=20 {
                let cell = IVec2::new(x, z);
                let level = grass_stride(cell, IVec2::ZERO);
                assert_eq!(level, grass_stride(IVec2::new(z, x), IVec2::ZERO));
                let nearest = ((cell.abs() - IVec2::ONE).max(IVec2::ZERO))
                    .as_vec2()
                    .length()
                    * CHUNK as f32;
                if level >= 4 {
                    assert!(nearest >= 20.0);
                }
                if level >= 8 {
                    assert!(nearest >= 40.0);
                }
            }
        }
    }

    #[test]
    fn every_lod_retains_exact_curved_blades_instead_of_widened_impostors() {
        for cell in [IVec2::new(-300, -279), IVec2::new(-53, 47)] {
            let full = chunk_mesh_detail(cell, 1);
            let full_vertices: std::collections::HashSet<_> = positions(&full)
                .iter()
                .map(|p| p.map(f32::to_bits))
                .collect();
            assert!(!full_vertices.is_empty());
            for stride in [4, 8, 16] {
                let coarse = chunk_mesh_detail(cell, stride);
                assert!(coarse.count_vertices() > 0);
                assert!(coarse.count_vertices() < full.count_vertices());
                for vertex in positions(&coarse) {
                    assert!(
                        full_vertices.contains(&vertex.map(f32::to_bits)),
                        "LOD {stride} changed a blade's shape in {cell:?}"
                    );
                }
                let Some(VertexAttributeValues::Float32x2(uvs)) =
                    coarse.attribute(Mesh::ATTRIBUTE_UV_0)
                else {
                    panic!()
                };
                assert!(
                    uvs.iter().any(|uv| uv[1] == 0.5),
                    "curved blade midpoint is missing"
                );
            }
        }
    }

    #[test]
    fn detail_streaming_covers_render_range_when_distances_match() {
        for load in [70.0, 120.0, 800.0] {
            assert_eq!(detail_load_radius(load, 48.0, 48.0), load);
            assert_eq!(detail_load_radius(load, 24.0, 48.0), load - 24.0);
        }
        let shader = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/shaders/grass.wgsl"
        ));
        assert!(shader.contains("length(world.xyz - camera_position.xyz)"));
        assert!(!shader.contains("smoothstep(detail.x * 0.8"));
        assert!(!shader.contains("let density"));
    }
    #[test]
    fn render_range_changes_rebuild_while_stationary() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<GrassMaterial>::default())
            .insert_resource(crate::player::camera::CameraRig::default())
            .insert_resource(crate::rendering::view_distance::Range { load: 8.0 })
            .init_resource::<GrassChunks>()
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Grass));
        for _ in 0..13 {
            app.update();
        }
        assert_eq!(app.world().resource::<GrassChunks>().levels.len(), 49);
        app.world_mut()
            .resource_mut::<crate::rendering::view_distance::Range>()
            .load = 20.0;
        for _ in 0..45 {
            app.update();
        }
        let chunks = app.world().resource::<GrassChunks>();
        assert_eq!(chunks.levels.len(), 169);
        assert!(chunks.levels.values().any(|stride| *stride == 4));
        app.world_mut()
            .resource_mut::<crate::rendering::view_distance::Range>()
            .load = 8.0;
        app.update();
        assert_eq!(app.world().resource::<GrassChunks>().levels.len(), 49);
    }
    #[test]
    fn detail_range_limits_grass_without_changing_world_range() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<GrassMaterial>::default())
            .insert_resource(crate::player::camera::CameraRig::default())
            .insert_resource(crate::rendering::view_distance::Range { load: 100.0 })
            .insert_resource(crate::rendering::view_distance::Detail(16.0))
            .init_resource::<GrassChunks>()
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Grass));
        for _ in 0..45 {
            app.update();
        }
        assert_eq!(app.world().resource::<GrassChunks>().levels.len(), 169);
        assert_eq!(
            app.world().resource::<GrassChunks>().levels[&IVec2::new(4, 0)],
            4
        );
        app.world_mut()
            .resource_mut::<crate::rendering::view_distance::Detail>()
            .0 = 24.0;
        // Existing chunks now also rebuild at higher density, not only the
        // newly extended outer ring. Allow the bounded queue to drain.
        for _ in 0..80 {
            app.update();
        }
        assert_eq!(app.world().resource::<GrassChunks>().levels.len(), 289);
        assert_eq!(
            app.world().resource::<GrassChunks>().levels[&IVec2::new(4, 0)],
            projected_grass_stride(IVec2::new(4, 0), IVec2::ZERO, 24.0, Some(4), 720.0 / 1.32)
        );
        assert_eq!(
            app.world()
                .resource::<crate::rendering::view_distance::Range>()
                .load,
            100.0
        );
        app.world_mut()
            .resource_mut::<crate::rendering::view_distance::Detail>()
            .0 = 0.0;
        app.update();
        let chunks = app.world().resource::<GrassChunks>();
        assert!(chunks.loaded.is_empty());
        assert!(chunks.levels.is_empty());
        assert!(chunks.queue.is_empty());
        app.world_mut()
            .resource_mut::<crate::rendering::view_distance::Detail>()
            .0 = 16.0;
        for _ in 0..45 {
            app.update();
        }
        assert_eq!(app.world().resource::<GrassChunks>().levels.len(), 169);
    }
    #[test]
    fn transition_blades_are_dormant_or_frosted_not_green() {
        let green = Vec3::new(0.055, 0.145, 0.014);
        assert_eq!(winter_color(green, 1.0, 0.0), green);
        for climate in [0.08, 0.15, 0.30, 0.50, 0.65, 1.0] {
            for t in [0.0, 0.33, 0.66, 1.0] {
                let c = winter_color(green, t, climate);
                assert!(c.is_finite());
                assert!(c.y <= c.x.max(c.z) + 0.012, "{climate} {t}: {c:?}");
            }
        }
    }
    use bevy::mesh::VertexAttributeValues;

    fn positions(mesh: &Mesh) -> &Vec<[f32; 3]> {
        let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions")
        };
        p
    }

    #[test]
    fn tall_grass_doubles_blade_height_without_changing_roots_or_population() {
        let cell = IVec2::new(-300, -279); // A seeded forest clearing.
        for stride in [1, 4, 8, 16] {
            let regular = build_chunk_mesh(cell, stride, false);
            let mixed = chunk_mesh_detail(cell, stride);
            let p = positions(&regular);
            let q = positions(&mixed);
            assert!(!p.is_empty());
            assert_eq!(p.len(), q.len());
            let Some(VertexAttributeValues::Float32x4(colors)) =
                regular.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!()
            };
            let mut tall = 0;
            let mut short = 0;
            for ((a, b), color) in p.iter().zip(q).zip(colors) {
                assert_eq!(a[0], b[0]);
                assert_eq!(a[2], b[2]);
                let h = a[1] - color[3];
                let actual = b[1] - color[3];
                if h > 0.01 {
                    if (actual - h).abs() < 0.00001 {
                        short += 1;
                    } else {
                        assert!((actual - 2.0 * h).abs() < 0.00001);
                        tall += 1;
                    }
                } else {
                    assert!((actual - h).abs() < 0.00001);
                }
            }
            assert!(
                tall > 0 && short > 0,
                "LOD {stride} must contain both variants"
            );
        }
    }

    #[test]
    fn tall_grass_favors_patch_interiors() {
        let mut counts = [0; 3];
        for x in -200..200 {
            for (index, density) in [0.05, 0.4, 0.8].into_iter().enumerate() {
                let mut seed = hash(x, 0, 0x74616c6c);
                counts[index] += usize::from(unit(&mut seed) < tall_chance(density));
            }
        }
        assert!(counts[0] > 0 && counts[0] < 30);
        assert!(counts[1] > counts[0] && counts[1] < counts[2]);
        assert!(counts[2] > 240);
    }

    #[test]
    fn extra_population_adds_forty_percent_without_density_clamping() {
        for seed in [0, 7, 721, u32::MAX] {
            let mut extra = 0;
            for x in -250..250 {
                for z in -200..200 {
                    extra += usize::from(extra_site(x, z, seed));
                }
            }
            let ratio = extra as f32 / 200_000.0;
            assert!((ratio - 0.40).abs() < 0.004, "{seed}: {ratio}");
        }
    }

    #[test]
    fn tree_clearings_grow_grass_even_where_the_open_ground_mask_is_empty() {
        let mut fixtures = Vec::new();
        'search: for x in -300..300 {
            for z in -300..300 {
                let cell = IVec2::new(x, z);
                let p = (cell.as_vec2() + Vec2::splat(0.5)) * CHUNK as f32;
                if crate::world::biome::forest_amount(p) > 0.9
                    && crate::world::biome::stand_density(p) < 0.08
                    && grass_density(p, crate::world::biome::shader_seed().x) < 0.001
                    && crate::world::biome::surface_snow(p) < 0.05
                    && crate::world::biome::mountain_habitat(p) > 0.9
                    && crate::world::terrain::vegetation_anchor(p, 0.1, 0.8).is_some()
                    && allowed(p.extend(0.0).xzy().with_y(crate::world::terrain::height(p)))
                {
                    fixtures.push(cell);
                    if fixtures.len() == 3 {
                        break 'search;
                    }
                }
            }
        }
        assert_eq!(fixtures.len(), 3);
        for cell in fixtures {
            let center = (cell.as_vec2() + Vec2::splat(0.5)) * CHUNK as f32;
            println!(
                "Grass clearing fixture: {center:?} ground {}",
                crate::world::terrain::height(center)
            );
            for stride in [1, 4, 8] {
                assert!(
                    chunk_mesh_detail(cell, stride).count_vertices() > 100,
                    "{cell:?} LOD {stride}"
                );
            }
        }
        assert_eq!(clearing_density(0.0, 1.0, 0.0), 0.70);
        assert_eq!(clearing_density(0.0, 1.0, 0.9), 0.0);
        assert_eq!(clearing_density(0.1, 0.0, 0.0), 0.1);
    }

    #[test]
    fn grass_density_preserves_budget_with_gradual_clustered_growth() {
        for seed in [0, 7, 28, u32::MAX] {
            let mut total = 0.0;
            let mut neighboring_change = 0.0;
            let mut seed_change = 0.0;
            let mut fringes = 0;
            let mut cores = 0;
            let mut sample_seed = 54;
            for _ in 0..200_000 {
                let p = Vec2::new(unit(&mut sample_seed), unit(&mut sample_seed)) * 4000.0
                    - Vec2::splat(2000.0);
                let density = grass_density(p, seed);
                assert!((0.0..=1.0).contains(&density));
                total += density;
                neighboring_change += (density - grass_density(p + Vec2::X * 0.125, seed)).abs();
                seed_change += (density - grass_density(p, seed ^ 12345)).abs();
                fringes += usize::from((0.05..0.45).contains(&density));
                cores += usize::from(density > 0.65);
            }
            let coverage = total / 200_000.0;
            assert!((coverage - 0.15).abs() < 0.01, "{seed}: {coverage}");
            assert!(neighboring_change / 200_000.0 < 0.04);
            assert!(seed_change / 200_000.0 > 0.1);
            assert!(fringes > 20_000, "growth must fade through sparse fringes");
            assert!(cores > 2000, "dense grass cores must remain");
        }
    }

    #[test]
    fn cold_foothills_grow_blades_where_surface_snow_has_melted() {
        let cells: Vec<_> = (-160..160)
            .flat_map(|x| (-160..160).map(move |z| IVec2::new(x, z)))
            .filter(|cell| {
                let p = (cell.as_vec2() + Vec2::splat(0.5)) * CHUNK as f32;
                crate::world::biome::snow_amount(p) > 0.95
                    && crate::world::biome::surface_snow(p) < 0.05
                    && crate::world::biome::mountain_habitat(p) > 0.9
                    && crate::world::terrain::vegetation_anchor(p, 2.0, 0.5).is_some()
                    && grass_density(p, crate::world::biome::shader_seed().x) > 0.3
            })
            .take(3)
            .collect();
        assert_eq!(cells.len(), 3, "missing cold, thawed foothill sites");
        for cell in cells {
            for stride in [1, 2] {
                assert!(
                    !positions(&chunk_mesh_detail(cell, stride)).is_empty(),
                    "snow-free foothill lost its grass at {cell:?}"
                );
            }
        }
    }

    #[test]
    fn blades_are_finite_curved_non_degenerate_and_repeatable() {
        let cells: Vec<_> = (-100..100)
            .flat_map(|x| (-100..100).map(move |z| IVec2::new(x, z)))
            .filter(|c| {
                crate::world::biome::snow_amount(c.as_vec2() * CHUNK as f32) == 0.0
                    && chunk_mesh(*c).count_vertices() > 0
            })
            .take(3)
            .collect();
        assert_eq!(cells.len(), 3);
        for cell in cells {
            let mesh = chunk_mesh(cell);
            let p = positions(&mesh);
            assert!(!p.is_empty());
            assert!(p.len() <= TUFTS_PER_SIDE as usize * TUFTS_PER_SIDE as usize * 7 * 10);
            assert_eq!(p, positions(&chunk_mesh(cell)));
            let Some(VertexAttributeValues::Float32x4(colors)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("missing root datums");
            };
            assert!(p.iter().zip(colors).all(|(v, color)| {
                Vec3::from(*v).is_finite() && (-0.00001..0.48).contains(&(v[1] - color[3]))
            }));
            let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
            else {
                panic!("missing blade coordinates");
            };
            for ((v, color), uv) in p.iter().zip(colors).zip(uvs) {
                if uv[1] == 0.0 {
                    assert!((v[1] - color[3]).abs() < 0.00001);
                }
            }
            let Some(VertexAttributeValues::Float32x3(n)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("missing normals")
            };
            assert!(
                n.iter()
                    .all(|v| (Vec3::from(*v).length() - 1.0).abs() < 1e-5)
            );
            assert!(mesh.indices().is_some());
            let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
            for tri in indices.chunks_exact(3) {
                let a = Vec3::from(p[tri[0]]);
                let b = Vec3::from(p[tri[1]]);
                let c = Vec3::from(p[tri[2]]);
                assert!((b - a).cross(c - a).length_squared() > 1e-12);
            }
        }
    }

    #[test]
    fn streaming_limits_build_work_and_reclaims_old_meshes() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<GrassMaterial>::default())
            .insert_resource(crate::player::camera::CameraRig::default())
            .init_resource::<GrassChunks>()
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Grass));
        app.update();
        assert_eq!(
            app.world().resource::<GrassChunks>().levels.len(),
            BUILD_PER_FRAME
        );
        assert!(
            app.world().resource::<GrassChunks>().loaded.is_empty(),
            "empty clearing cells must not allocate entities"
        );
        assert_eq!(
            app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
            0
        );
        for point in [
            Vec3::new(0.0, 1.25, 3.15),
            Vec3::new(-84.0, 1.25, 60.0),
            Vec3::new(0.0, 1.25, 3.15),
        ] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = point;
            // Uploads have a measured time budget as well as a job-count
            // cap. Denser chunks may need more frames on a loaded test host.
            for _ in 0..64 {
                app.update();
                let chunks = app.world().resource::<GrassChunks>();
                if chunks.levels.len() == 49 && chunks.pending.is_empty() {
                    break;
                }
            }
            let chunks = app.world().resource::<GrassChunks>();
            assert_eq!(chunks.levels.len(), 49);
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().len(),
                chunks
                    .loaded
                    .values()
                    .filter(|(_, mesh)| mesh.is_some())
                    .count()
            );
            for (entity, handle) in chunks.loaded.values() {
                assert!(app.world().get_entity(*entity).is_ok());
                if let Some(handle) = handle {
                    let mesh = app.world().resource::<Assets<Mesh>>().get(handle).unwrap();
                    assert!(mesh.count_vertices() > 0);
                } else {
                    assert!(app.world().get::<Mesh3d>(*entity).is_none());
                }
            }
        }
    }

    #[test]
    fn obsolete_jobs_remain_bounded_and_cannot_upload_after_teleport() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<GrassMaterial>::default())
            .insert_resource(crate::player::camera::CameraRig::default())
            .init_resource::<GrassChunks>()
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Grass));
        app.update();
        for i in 0..BUILD_PER_FRAME {
            app.world_mut()
                .resource_mut::<GrassChunks>()
                .pending
                .insert(
                    IVec2::new(-1000 + i as i32, -1000),
                    GrassJob {
                        task: crate::world::streaming::BuildTask::pending_for_test(),
                        stride: 1,
                    },
                );
        }
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position = Vec3::splat(5000.0);
        for _ in 0..3 {
            app.update();
            let chunks = app.world().resource::<GrassChunks>();
            assert!(chunks.loaded.is_empty());
            assert_eq!(chunks.pending.len(), BUILD_PER_FRAME);
        }
        for job in app
            .world_mut()
            .resource_mut::<GrassChunks>()
            .pending
            .values_mut()
        {
            job.task.finish_for_test(Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            ));
        }
        app.update();
        let chunks = app.world().resource::<GrassChunks>();
        assert!(chunks.pending.is_empty() && chunks.loaded.is_empty());
        app.update();
        assert_eq!(
            app.world().resource::<GrassChunks>().levels.len(),
            BUILD_PER_FRAME
        );
    }

    #[test]
    fn pausing_freezes_wind_and_resuming_updates_player_bend() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .insert_resource(Assets::<GrassMaterial>::default())
            .insert_resource(crate::app::GameState { paused: false })
            .insert_resource(crate::player::camera::CameraRig::default())
            .add_systems(Startup, setup)
            .add_systems(Update, wind);
        app.update();
        let motion = |app: &App| {
            let handle = &app.world().resource::<GrassAssets>().0;
            app.world()
                .resource::<Assets<GrassMaterial>>()
                .get(handle)
                .unwrap()
                .extension
                .motion
        };
        let first = motion(&app);
        assert!(first.x > 0.0);
        let contacts = |app: &App| {
            let handle = &app.world().resource::<GrassAssets>().0;
            app.world()
                .resource::<Assets<GrassMaterial>>()
                .get(handle)
                .unwrap()
                .extension
                .player_steps
        };
        let first_person = contacts(&app);
        assert!(first_person[0].w > 0.0);
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .camera_mode = crate::player::camera::CameraMode::ThirdPerson;
        app.update();
        assert_eq!(first_person, contacts(&app));
        let first = motion(&app);
        app.world_mut()
            .resource_mut::<crate::app::GameState>()
            .paused = true;
        app.update();
        assert_eq!(first, motion(&app));
        app.world_mut()
            .resource_mut::<crate::app::GameState>()
            .paused = false;
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position
            .x = 2.0;
        app.update();
        assert!(motion(&app).x > first.x);
        assert_eq!(motion(&app).y, 2.0);
    }
}
