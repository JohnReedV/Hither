//! Seeded alpine rock assemblies. Shared high/low meshes, bounded chunk streaming,
//! and render-independent triangle queries keep outcrops solid after unloading.
use crate::world::streaming::{Coordinator, Layer};
use crate::{
    rendering::geometry::Geometry,
    world::{biome, random::Rng, terrain},
};
use bevy::prelude::*;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    sync::{Arc, OnceLock},
};
pub(crate) mod contact;
const CELL: f32 = 32.0;
const VARIANTS: usize = 5;
const KINDS: usize = 8;
const NAMES: [&str; KINDS] = [
    "Granite boulder",
    "Layered stone ledge",
    "Broken crag",
    "Shale talus",
    "Weathered tor",
    "Blade outcrop",
    "Alpine cushion",
    "Alpine tussock",
];

pub struct GeologyPlugin;
impl Plugin for GeologyPlugin {
    fn build(&self, app: &mut App) {
        contact::plugin(app);
        app.init_resource::<Stream>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                stream
                    .in_set(Layer::Rocks)
                    .after(crate::player::movement::move_camera),
            );
    }
}
struct Model {
    geometry: Geometry,
    parts: Vec<std::ops::Range<usize>>,
    low: Geometry,
}
fn models() -> &'static Vec<Model> {
    static BANK: OnceLock<Vec<Model>> = OnceLock::new();
    BANK.get_or_init(|| {
        (0..KINDS * VARIANTS)
            .map(|i| generate(i / VARIANTS, i % VARIANTS))
            .collect()
    })
}
fn hash(cell: IVec2) -> u64 {
    biome::world_seed()
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9)
        ^ 0x67656f6c6f6779
}
fn triangle(g: &mut Geometry, a: Vec3, b: Vec3, c: Vec3, color: Vec3) {
    let n = (b - a).cross(c - a).normalize_or(Vec3::Y);
    let start = g.positions.len() as u32;
    for p in [a, b, c] {
        let uv = if n.y.abs() > 0.6 {
            p.xz()
        } else if n.x.abs() > n.z.abs() {
            Vec2::new(p.z, p.y)
        } else {
            Vec2::new(p.x, p.y)
        };
        g.vertex(p, uv * 1.7, color);
        *g.normals.last_mut().unwrap() = n.to_array();
    }
    g.triangle(start, start + 1, start + 2);
}
// Subdivide an octahedron, then deform its shared vertices before splitting
// shading seams. All triangles remain watertight, including the buried base.
fn stone(g: &mut Geometry, offset: Vec3, scale: Vec3, seed: u64, angular: f32, level: u32) {
    let points = [Vec3::Y, -Vec3::Y, Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z];
    let mut faces = Vec::new();
    for i in 0..4 {
        let a = points[2 + i];
        let b = points[2 + (i + 1) % 4];
        faces.push([points[0], b, a]);
        faces.push([points[1], a, b]);
    }
    for _ in 0..level {
        faces = faces
            .into_iter()
            .flat_map(|[a, b, c]| {
                let ab = (a + b).normalize();
                let bc = (b + c).normalize();
                let ca = (c + a).normalize();
                [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
            })
            .collect();
    }
    let phase = (seed % 10007) as f32 * 0.137;
    let lean = Quat::from_euler(
        EulerRot::XYZ,
        phase.sin() * 0.16,
        (phase * 0.71).sin() * 0.25,
        (phase * 1.13).cos() * 0.18,
    );
    let deform = |n: Vec3| {
        let block = n.signum() * n.abs().powf(angular);
        let fracture = (n.x * 7.1 + n.z * 4.3 + phase).sin() * (n.y * 5.7 - phase).cos();
        let chips = (n.x * 19.0 - n.y * 13.1 + n.z * 7.7 + phase).sin();
        offset
            + lean
                * ((block * (1.0 + fracture * 0.13 + chips * 0.035)
                    + Vec3::new(n.y * 0.13, n.z * 0.06, 0.0))
                    * scale)
    };
    for [a, b, c] in faces {
        let [a, b, c] = [a, b, c].map(deform);
        let center = (a + b + c) / 3.0;
        let shade = 0.86 + 0.06 * (center.dot(Vec3::new(2.3, 0.7, 1.2)) + phase).sin();
        let up = (b - a).cross(c - a).normalize_or(Vec3::Y).y;
        let lichen = biome::smooth_range(0.55, 0.90, up)
            * biome::smooth_range(0.1, 0.8, (center.x * 3.7 + center.z * 5.3 + phase).sin())
            * 0.22;
        let color = Vec3::splat(shade).lerp(Vec3::new(0.49, 0.52, 0.28), lichen);
        triangle(g, a, b, c, color);
    }
}
// Smooth only weathered, shallow facet joins. Major chipped edges retain
// their original face normals; topology and collision positions are untouched.
fn weather_normals(g: &mut Geometry, start: usize) {
    let mut shared: HashMap<[u32; 3], Vec<Vec3>> = HashMap::new();
    for i in start..g.positions.len() {
        shared
            .entry(g.positions[i].map(f32::to_bits))
            .or_default()
            .push(Vec3::from(g.normals[i]));
    }
    for i in start..g.positions.len() {
        let n = Vec3::from(g.normals[i]);
        let average = shared[&g.positions[i].map(f32::to_bits)]
            .iter()
            .filter(|v| v.dot(n) > 0.82)
            .copied()
            .sum::<Vec3>()
            .normalize_or(n);
        g.normals[i] = n.lerp(average, 0.75).normalize_or(n).to_array();
    }
}
fn generate(kind: usize, variant: usize) -> Model {
    let seed = 0x726f636bu64 + kind as u64 * 731 + variant as u64 * 197;
    let mut rng = Rng(seed);
    let mut pieces = Vec::new();
    match kind {
        0 => {
            pieces.push((Vec3::ZERO, Vec3::new(1.0, 0.8, 0.86), 0.65));
            pieces.push((
                Vec3::new(0.55, -0.4, 0.4),
                Vec3::new(0.48, 0.37, 0.55),
                0.58,
            ));
        }
        1 => {
            for i in 0..4 {
                pieces.push((
                    Vec3::new(
                        rng.range(-0.22, 0.22),
                        i as f32 * 0.26 - 0.4,
                        rng.range(-0.12, 0.12),
                    ),
                    Vec3::new(rng.range(0.85, 1.25), 0.22, rng.range(0.65, 0.85)),
                    0.32,
                ));
            }
        }
        2 => {
            for _ in 0..5 {
                pieces.push((
                    Vec3::new(
                        rng.range(-0.65, 0.65),
                        rng.range(-0.2, 0.25),
                        rng.range(-0.55, 0.55),
                    ),
                    Vec3::new(
                        rng.range(0.3, 0.65),
                        rng.range(0.6, 1.3),
                        rng.range(0.35, 0.7),
                    ),
                    0.48,
                ));
            }
        }
        3 => {
            for _ in 0..9 {
                pieces.push((
                    Vec3::new(
                        rng.range(-1.2, 1.2),
                        rng.range(-0.22, 0.05),
                        rng.range(-0.9, 0.9),
                    ),
                    Vec3::new(
                        rng.range(0.17, 0.5),
                        rng.range(0.10, 0.26),
                        rng.range(0.2, 0.48),
                    ),
                    0.38,
                ));
            }
        }
        4 => {
            for i in 0..3 {
                pieces.push((
                    Vec3::new(
                        rng.range(-0.18, 0.18),
                        i as f32 * 0.7 - 0.4,
                        rng.range(-0.15, 0.15),
                    ),
                    Vec3::new(1.0 - i as f32 * 0.16, 0.55, 0.77 - i as f32 * 0.1),
                    0.73,
                ));
            }
        }
        5 => {
            for i in 0..3 {
                pieces.push((
                    Vec3::new(i as f32 * 0.45 - 0.45, 0.0, rng.range(-0.2, 0.2)),
                    Vec3::new(0.20, rng.range(0.8, 1.5), 0.8),
                    0.30,
                ));
            }
        }
        _ => {}
    }
    let mut geometry = Geometry::default();
    let mut low = Geometry::default();
    let mut parts = Vec::new();
    for (i, (p, s, angular)) in pieces.into_iter().enumerate() {
        let start = geometry.indices.len();
        let first_vertex = geometry.positions.len();
        let first_low = low.positions.len();
        stone(&mut geometry, p, s, seed + i as u64 * 67, angular, 3);
        parts.push(start..geometry.indices.len());
        stone(&mut low, p, s, seed + i as u64 * 67, angular, 2);
        weather_normals(&mut geometry, first_vertex);
        weather_normals(&mut low, first_low);
    }
    if kind >= 6 {
        // Low woody cushions and wind-bent tussocks, not full-size summit trees.
        for _ in 0..if kind == 6 { 35 } else { 24 } {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let r = rng.unit().sqrt() * 0.7;
            let a = Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
            let b = a + Vec3::new(0.12, rng.range(0.16, 0.48) * (1.0 - r * 0.5), 0.08);
            let side =
                Vec3::new(-angle.sin(), 0.0, angle.cos()) * if kind == 6 { 0.08 } else { 0.025 };
            let color = if kind == 6 {
                Vec3::new(0.18, 0.27, 0.085)
            } else {
                Vec3::new(0.42, 0.35, 0.16)
            } * rng.range(0.65, 1.1);
            triangle(&mut geometry, a - side, a + side, b, color);
            triangle(&mut geometry, a + side, a - side, b, color);
            triangle(&mut low, a - side, a + side, b, color);
            triangle(&mut low, a + side, a - side, b, color);
        }
    }
    Model {
        geometry,
        parts,
        low,
    }
}
#[derive(Clone)]
struct Instance {
    model: usize,
    transform: Transform,
    inverse: Mat4,
    low: Vec3,
    high: Vec3,
    small: bool,
}
impl Instance {
    fn new(model: usize, transform: Transform, small: bool) -> Self {
        let matrix = transform.to_matrix();
        let (low, high) = models()[model]
            .geometry
            .positions
            .iter()
            .map(|p| matrix.transform_point3(Vec3::from(*p)))
            .fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(a, b), p| (a.min(p), b.max(p)),
            );
        Self {
            model,
            transform,
            inverse: matrix.inverse(),
            low,
            high,
            small,
        }
    }
    // Ray in model space has a world-metre parameter, even with nonuniform scale.
    fn intervals(&self, at: Vec2) -> Vec<(f32, f32)> {
        if at.x < self.low.x || at.x > self.high.x || at.y < self.low.z || at.y > self.high.z {
            return Vec::new();
        }
        let origin = self.inverse.transform_point3(Vec3::new(at.x, 0.0, at.y));
        let direction = self.inverse.transform_vector3(Vec3::Y);
        let model = &models()[self.model];
        model
            .parts
            .iter()
            .filter_map(|part| {
                let mut low = f32::INFINITY;
                let mut high = f32::NEG_INFINITY;
                for face in model.geometry.indices[part.clone()].chunks_exact(3) {
                    let [a, b, c] = [face[0], face[1], face[2]]
                        .map(|i| Vec3::from(model.geometry.positions[i as usize]));
                    let e = b - a;
                    let f = c - a;
                    let cross = direction.cross(f);
                    let det = e.dot(cross);
                    if det.abs() < 1e-7 {
                        continue;
                    }
                    let t = origin - a;
                    let u = t.dot(cross) / det;
                    let q = t.cross(e);
                    let v = direction.dot(q) / det;
                    if u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 {
                        let h = f.dot(q) / det;
                        low = low.min(h);
                        high = high.max(h);
                    }
                }
                (low.is_finite() && high - low > 0.001).then_some((low, high))
            })
            .collect()
    }
}
fn placements(cell: IVec2) -> Vec<Instance> {
    let mut rng = Rng(hash(cell));
    let mut result = Vec::new();
    // Coherent colonies, separated by untouched slopes; no regular rock grid.
    let region = (cell.as_vec2() / 5.0).floor().as_ivec2();
    let mut region_rng = Rng(hash(region) ^ 0x7061746368);
    let density = region_rng.range(0.25, 0.9);
    let palette = (region_rng.unit() * 3.0) as usize;
    for i in 0..12 {
        let p = (cell.as_vec2() + Vec2::new(rng.unit(), rng.unit())) * CELL;
        let mountain = biome::mountain_amount(p);
        if mountain < 0.45
            || rng.unit() > density
            || crate::world::orcs::terrain_clearance(p) < 0.999
        {
            continue;
        }
        let gradient = terrain::gradient(p);
        let slope = gradient.length();
        let ground = terrain::height(p);
        if ground < 20.0 {
            continue;
        }
        let small = i >= 3;
        let roll = rng.unit();
        let kind = if small {
            if slope < 0.75 && ground < 175.0 && biome::surface_snow(p) < 0.25 && roll > 0.5 {
                6 + usize::from(roll > 0.75)
            } else {
                3
            }
        } else if slope > 1.1 {
            match palette {
                0 => {
                    if roll < 0.75 {
                        2
                    } else {
                        1
                    }
                }
                1 => {
                    if roll < 0.80 {
                        1
                    } else {
                        5
                    }
                }
                _ => {
                    if roll < 0.7 {
                        4
                    } else {
                        2
                    }
                }
            }
        } else if roll < 0.45 {
            0
        } else if roll < 0.75 {
            2
        } else {
            4
        };
        if small && slope > 1.0 {
            continue;
        }
        let model =
            kind * VARIANTS + (rng.unit() * VARIANTS as f32).min((VARIANTS - 1) as f32) as usize;
        let size = if kind >= 6 {
            rng.range(0.45, 1.1)
        } else if small {
            rng.range(0.35, 1.3)
        } else if kind == 1 || kind == 5 {
            2.5 + rng.unit().powi(2) * 5.5
        } else {
            rng.range(1.0, 4.3)
        };
        let yaw = if slope > 1.1 {
            gradient.x.atan2(gradient.y) + rng.range(-0.2, 0.2)
        } else {
            rng.range(0.0, std::f32::consts::TAU)
        };
        let scale = Vec3::new(
            size * rng.range(0.85, 1.3),
            size * rng.range(0.7, 1.15),
            size * rng.range(0.65, 1.1),
        );
        let rotation = Quat::from_rotation_y(yaw)
            * Quat::from_rotation_x(if kind < 6 {
                rng.range(-0.16, 0.16)
            } else {
                0.0
            });
        // Vegetation uses the entire footprint; cliff assemblies are anchored
        // deep into the face, with a real exposed underside and ledge above it.
        let anchor = if kind >= 6 {
            let Some(y) = terrain::vegetation_anchor(p, size * 0.75, 0.85) else {
                continue;
            };
            y
        } else {
            ground
                - size
                    * if kind == 3 {
                        0.10
                    } else if slope > 1.1 {
                        0.7
                    } else {
                        0.48
                    }
        };
        let transform = Transform::from_xyz(p.x, anchor, p.y)
            .with_rotation(rotation)
            .with_scale(scale);
        result.push(Instance::new(model, transform, small));
    }
    // Fallen fragments accumulate at the foot of the larger formations.
    // They use the same material, and are only streamed in the nearby detail band.
    let formations: Vec<_> = result.iter().filter(|r| !r.small).cloned().collect();
    for parent in formations {
        let center = parent.transform.translation.xz();
        let footprint = (parent.high - parent.low).xz() * 0.5;
        for _ in 0..3 {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let p = center + Vec2::new(angle.cos(), angle.sin()) * footprint * rng.range(0.8, 1.3);
            if biome::mountain_amount(p) < 0.45
                || terrain::gradient(p).length() > 1.0
                || crate::world::orcs::terrain_clearance(p) < 0.999
            {
                continue;
            }
            let size = rng.range(0.3, 0.85);
            let model =
                3 * VARIANTS + (rng.unit() * VARIANTS as f32).min((VARIANTS - 1) as f32) as usize;
            let pose = Transform::from_xyz(p.x, terrain::height(p) - size * 0.10, p.y)
                .with_scale(Vec3::splat(size))
                .with_rotation(Quat::from_rotation_y(angle));
            result.push(Instance::new(model, pose, true));
        }
    }
    result
}
#[derive(Default)]
struct PlacementCache {
    entries: HashMap<IVec2, (u64, Arc<Vec<Instance>>)>,
    clock: u64,
}
impl PlacementCache {
    fn get(&mut self, cell: IVec2, build: impl FnOnce() -> Vec<Instance>) -> Arc<Vec<Instance>> {
        self.clock += 1;
        if let Some((used, value)) = self.entries.get_mut(&cell) {
            *used = self.clock;
            return value.clone();
        }
        let value = Arc::new(build());
        if self.entries.len() == 256 {
            let oldest = *self
                .entries
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .unwrap()
                .0;
            self.entries.remove(&oldest);
        }
        self.entries.insert(cell, (self.clock, value.clone()));
        value
    }
}
thread_local! { static CELLS: RefCell<PlacementCache> = RefCell::new(PlacementCache::default()); }
fn cached(cell: IVec2) -> Arc<Vec<Instance>> {
    CELLS.with_borrow_mut(|cache| cache.get(cell, || placements(cell)))
}
fn nearby(at: Vec2, mut visit: impl FnMut(&Instance)) {
    if biome::mountain_amount(at) < 0.2 {
        return;
    }
    let c = (at / CELL).floor().as_ivec2();
    for x in -1..=1 {
        for z in -1..=1 {
            for rock in cached(c + IVec2::new(x, z)).iter() {
                if rock.model / VARIANTS < 6 {
                    visit(rock);
                }
            }
        }
    }
}
pub(crate) fn support(p: Vec3, radius: f32) -> Option<f32> {
    let feet = p.y - crate::player::movement::PLAYER_EYE_HEIGHT;
    let mut best = f32::NEG_INFINITY;
    nearby(p.xz(), |rock| {
        for at in crate::world::floor::footprint(p.xz(), radius * 0.7) {
            for (_, top) in rock.intervals(at) {
                if top <= feet + 0.201 {
                    best = best.max(top);
                }
            }
        }
    });
    best.is_finite().then_some(best)
}
pub(crate) fn collides(p: Vec3, radius: f32, height: f32) -> bool {
    volume_collision(p, radius, height, 0.205)
}
pub(crate) fn camera_obstructed(p: Vec3, radius: f32) -> bool {
    volume_collision(p - Vec3::Y * radius, radius, radius * 2.0, 0.0)
}
fn volume_collision(p: Vec3, radius: f32, height: f32, step: f32) -> bool {
    let mut hit = false;
    nearby(p.xz(), |rock| {
        if hit || p.y > rock.high.y + 0.01 || p.y + height < rock.low.y {
            return;
        }
        for at in crate::world::floor::footprint(p.xz(), radius) {
            for (bottom, top) in rock.intervals(at) {
                if bottom < p.y + height && top > p.y + step {
                    hit = true;
                    return;
                }
            }
        }
    });
    hit
}
#[derive(Resource)]
struct Bank {
    meshes: Vec<[Handle<Mesh>; 2]>,

    plants: Handle<StandardMaterial>,
}
#[derive(Component)]
struct RockLod {
    index: usize,
    detailed: bool,
    small: bool,
}
#[derive(Resource, Default)]
struct Stream {
    loaded: HashMap<IVec2, Vec<Entity>>,
    details: HashMap<IVec2, Vec<Entity>>,
    discovery: crate::world::streaming::Discovery<Arc<Vec<Instance>>>,
    detail_discovery: crate::world::streaming::Discovery<Arc<Vec<Instance>>>,
    region: Option<(IVec2, i32)>,
    detail_region: Option<(IVec2, i32)>,
    lod_position: Option<Vec3>,
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Climate, stone color and contact blending are evaluated in world space by
    // the terrain shader. No differently tinted duplicates or separate textures.
    let handles = models()
        .iter()
        .map(|model| {
            let mut high = model.geometry.mesh();
            let mut low = model.low.mesh();
            high.insert_attribute(
                contact::INDEX,
                (0..model.geometry.positions.len() as u32).collect::<Vec<_>>(),
            );
            low.insert_attribute(
                contact::INDEX,
                (model.geometry.positions.len() as u32
                    ..(model.geometry.positions.len() + model.low.positions.len()) as u32)
                    .collect::<Vec<_>>(),
            );
            [meshes.add(high), meshes.add(low)]
        })
        .collect();
    let plants = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        reflectance: 0.02,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(Bank {
        meshes: handles,
        plants,
    });
}
#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn stream(
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    view: Option<Res<crate::player::camera::CameraView>>,
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    range: Res<crate::rendering::view_distance::Range>,
    bank: Res<Bank>,
    surfaces: Res<crate::rendering::sdf::SdfMaterialHandle>,
    mut state: ResMut<Stream>,
    mut lods: Query<(&Transform, &mut Mesh3d, &mut Visibility, &mut RockLod)>,
) {
    let camera_position = view.as_ref().map_or(rig.position, |v| v.position);
    let detail_limit = detail.as_ref().map_or(90.0, |d| d.0).min(range.load);
    let detail_changed = detail.as_ref().is_some_and(|d| d.is_changed());
    let center = (rig.position.xz() / CELL).floor().as_ivec2();
    let radius = (range.load / CELL).ceil() as i32 + 1;
    let detail_radius = if detail_limit <= 0.0 {
        -1
    } else {
        (((detail_limit + 4.0) / CELL).ceil() as i32 + 1).min(radius)
    };
    if state.region != Some((center, radius))
        || state.detail_region != Some((center, detail_radius))
    {
        state.detail_region = Some((center, detail_radius));
        state.region = Some((center, radius));
        state.loaded.retain(|c, entities| {
            if (*c - center).abs().max_element() <= radius {
                return true;
            }
            for e in entities {
                crate::world::streaming::retire(&mut commands, *e);
            }
            false
        });
        state.details.retain(|c, entities| {
            if (*c - center).abs().max_element() <= detail_radius {
                return true;
            }
            for e in entities {
                crate::world::streaming::retire(&mut commands, *e);
            }
            false
        });
        state.discovery.update(center, radius);
        if detail_limit <= 0.0 {
            state.detail_discovery = default();
        } else {
            state.detail_discovery.update(center, detail_radius);
        }
    }
    state.discovery.poll(&mut coordinator, Layer::Rocks, cached);
    state
        .detail_discovery
        .poll(&mut coordinator, Layer::Rocks, cached);
    for i in 0..12 {
        let small = state.detail_discovery.front().is_some()
            && (i % 2 == 0 || state.discovery.front().is_none());
        let discovery = if small {
            &state.detail_discovery
        } else {
            &state.discovery
        };
        let Some((_, instances)) = discovery.front() else {
            break;
        };
        let count = instances
            .iter()
            .filter(|instance| instance.small == small)
            .count();
        let Some(_installation) = coordinator.install_entities(Layer::Rocks, 4, 0, count) else {
            break;
        };
        let (cell, instances) = if small {
            state.detail_discovery.pop()
        } else {
            state.discovery.pop()
        }
        .unwrap();
        let mut entities = Vec::new();
        for instance in instances.iter() {
            if instance.small != small {
                continue;
            }
            let distance = instance.transform.translation.distance(camera_position);
            let detailed = distance < detail_limit;
            let index = instance.model;
            let mut entity = commands.spawn((
                Name::new(NAMES[instance.model / VARIANTS]),
                Mesh3d(bank.meshes[index][usize::from(!detailed)].clone()),
                instance.transform,
                if instance.small && (detail_limit <= 0.0 || distance > detail_limit) {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                RockLod {
                    index,
                    detailed,
                    small: instance.small,
                },
            ));
            if index / VARIANTS >= 6 {
                entity.insert(MeshMaterial3d(bank.plants.clone()));
            } else {
                entity.insert(MeshMaterial3d(surfaces.2.clone()));
            }
            let entity = entity.id();
            if index / VARIANTS < 6 {
                contact::enqueue(&mut commands, entity);
            }
            entities.push(entity);
        }
        if small {
            state.details.insert(cell, entities);
        } else {
            state.loaded.insert(cell, entities);
        }
    }
    if !detail_changed && state.lod_position == Some(camera_position) {
        return;
    }
    let previous = state
        .lod_position
        .replace(camera_position)
        .unwrap_or(camera_position);
    // Visit only cells that can contain a near-detail transition. Far cells
    // keep their existing low meshes; a settings change revisits all cells.
    for (cell, entities) in state.loaded.iter().chain(state.details.iter()) {
        let at = (cell.as_vec2() + Vec2::splat(0.5)) * CELL;
        let reach = detail_limit + CELL * 2.0;
        if !detail_changed
            && at.distance(camera_position.xz()) > reach
            && at.distance(previous.xz()) > reach
        {
            continue;
        }
        for &entity in entities {
            let Ok((transform, mut mesh, mut visibility, mut lod)) = lods.get_mut(entity) else {
                continue;
            };

            let d = transform.translation.distance(camera_position);
            let detailed = d < detail_limit * if lod.detailed { 1.0 } else { 0.95 };
            if detailed != lod.detailed {
                lod.detailed = detailed;
                mesh.0 = bank.meshes[lod.index][usize::from(!detailed)].clone();
            }
            let visible = if lod.small && (detail_limit <= 0.0 || d > detail_limit) {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            if *visibility != visible {
                *visibility = visible;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_cache_evicts_only_the_least_recent_cell() {
        let mut cache = PlacementCache::default();
        for x in 0..256 {
            cache.get(IVec2::new(x, 0), Vec::new);
        }
        let retained = cache.get(IVec2::ZERO, || panic!("cache hit rebuilt"));
        cache.get(IVec2::new(256, 0), Vec::new);
        assert_eq!(cache.entries.len(), 256);
        assert!(!cache.entries.contains_key(&IVec2::X));
        assert!(Arc::ptr_eq(
            &retained,
            &cache.get(IVec2::ZERO, || panic!("hot entry lost"))
        ));
        assert!(cache.entries.contains_key(&IVec2::new(2, 0)));
    }
    #[test]
    fn rock_library_has_distinct_watertight_variants_and_cheaper_lods() {
        for (index, model) in models().iter().enumerate() {
            let g = &model.geometry;
            assert!(g.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(
                g.normals
                    .iter()
                    .all(|n| (Vec3::from(*n).length() - 1.0).abs() < 0.001)
            );
            if index / VARIANTS >= 6 {
                continue;
            }
            assert!(model.low.indices.len() * 3 <= g.indices.len());
            for part in &model.parts {
                let mut edges = HashMap::new();
                for tri in g.indices[part.clone()].chunks_exact(3) {
                    for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                        let a = g.positions[a as usize].map(f32::to_bits);
                        let b = g.positions[b as usize].map(f32::to_bits);
                        let edge = if a < b { (a, b) } else { (b, a) };
                        *edges.entry(edge).or_insert(0) += 1;
                    }
                }
                assert!(edges.values().all(|n| *n == 2), "open rock model {index}");
            }
            if index % VARIANTS > 0 {
                assert_ne!(g.positions, models()[index - 1].geometry.positions);
            }
        }
    }
    #[test]
    fn ray_support_matches_transformed_visible_triangles() {
        for index in 0..6 * VARIANTS {
            let transform = Transform::from_xyz(-173.4, 80.3, 291.7)
                .with_scale(Vec3::new(3.7, 1.9, 2.8))
                .with_rotation(Quat::from_rotation_y(1.7));
            let instance = Instance::new(index, transform, false);
            let matrix = transform.to_matrix();
            let g = &models()[index].geometry;
            let mut checked = 0;
            for face in g.indices.chunks_exact(3).step_by(7) {
                let [a, b, c] = [face[0], face[1], face[2]]
                    .map(|i| matrix.transform_point3(Vec3::from(g.positions[i as usize])));
                if (b - a).cross(c - a).normalize_or(Vec3::Y).y < 0.3 {
                    continue;
                }
                let p = (a + b + c) / 3.0;
                assert!(
                    instance
                        .intervals(p.xz())
                        .iter()
                        .any(|(_, top)| *top >= p.y - 0.003),
                    "missing support on {index} at {p}"
                );
                checked += 1;
            }
            assert!(checked > 5);
            assert!(
                instance
                    .intervals(instance.high.xz() + Vec2::ONE)
                    .is_empty()
            );
        }
    }
    #[test]
    fn placements_are_repeatable_clustered_and_keep_clearings_clear() {
        let mut kinds = [0; KINDS];
        let mut empty = 0;
        for x in -30..30 {
            for z in -30..30 {
                let cell = IVec2::new(x, z);
                let placed = placements(cell);
                if placed.is_empty() {
                    empty += 1;
                }
                for item in &placed {
                    let p = item.transform.translation.xz();
                    assert!(biome::mountain_amount(p) >= 0.45);
                    assert_eq!(crate::world::orcs::terrain_clearance(p), 1.0);
                    assert!(item.low.is_finite() && item.high.is_finite());
                    kinds[item.model / VARIANTS] += 1;
                }
                if x % 11 == 0 && z % 7 == 0 {
                    let again = placements(cell);
                    assert_eq!(placed.len(), again.len());
                    for (a, b) in placed.iter().zip(again) {
                        assert_eq!(a.model, b.model);
                        assert_eq!(a.transform, b.transform);
                    }
                }
            }
        }
        assert!(empty > 100);
        assert!(
            kinds.iter().all(|n| *n > 0),
            "missing alpine families: {kinds:?}"
        );
        assert!(placements(IVec2::ZERO).is_empty());
    }
    #[test]
    fn exposed_rocks_support_players_block_bodies_and_survive_cache_eviction() {
        let mut checked = 0;
        'cells: for x in -30..30 {
            for z in -30..30 {
                for rock in placements(IVec2::new(x, z)) {
                    if rock.model / VARIANTS >= 6 {
                        continue;
                    }
                    let at = rock.transform.translation.xz();
                    let Some(top) = rock
                        .intervals(at)
                        .iter()
                        .map(|(_, top)| *top)
                        .max_by(f32::total_cmp)
                    else {
                        continue;
                    };
                    if top < terrain::height(at) + 0.6 {
                        continue;
                    }
                    let p = Vec3::new(at.x, top, at.y);
                    let eye = p + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
                    let before = support(eye, 0.0).unwrap();
                    assert!((before - top).abs() < 0.01);
                    assert!(collides(p - Vec3::Y * 0.3, 0.1, 1.4));
                    assert!(camera_obstructed(p - Vec3::Y * 0.05, 0.08));
                    CELLS.with_borrow_mut(|cache| *cache = PlacementCache::default());
                    assert_eq!(support(eye, 0.0), Some(before));
                    checked += 1;
                    if checked >= 20 {
                        break 'cells;
                    }
                }
            }
        }
        assert_eq!(checked, 20);
    }

    #[test]
    fn streaming_reuses_assets_and_bounds_small_detail_to_nearby_cells() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Stream>()
            .insert_resource(crate::rendering::sdf::SdfMaterialHandle(
                default(),
                default(),
                default(),
            ))
            .init_resource::<crate::player::camera::CameraRig>()
            .insert_resource(crate::rendering::view_distance::Range { load: 64.0 })
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Rocks));
        app.update();
        let assets = app.world().resource::<Assets<Mesh>>().len();
        let mut rocks = 0;
        for position in [
            Vec3::new(592.0, 30.0, 1344.0),
            Vec3::new(-1500.0, 200.0, 850.0),
            Vec3::ZERO,
        ] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = position;
            for _ in 0..64 {
                app.update();
                if app.world().resource::<Stream>().discovery.is_empty()
                    && app.world().resource::<Stream>().detail_discovery.is_empty()
                {
                    break;
                }
            }
            let state = app.world().resource::<Stream>();
            assert!(state.loaded.len() <= 49 && state.details.len() <= 49);
            assert!((state.discovery.is_empty() && state.detail_discovery.is_empty()));
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), assets);
            let mut materials = app.world_mut().query::<(
                &RockLod,
                Option<&MeshMaterial3d<crate::rendering::sdf::SdfMaterial>>,
                Option<&MeshMaterial3d<StandardMaterial>>,
            )>();
            for (lod, shared, plant) in materials.iter(app.world()) {
                if lod.index / VARIANTS < 6 {
                    assert!(shared.is_some() && plant.is_none());
                    rocks += 1;
                } else {
                    assert!(shared.is_none() && plant.is_some());
                }
            }
        }
        // Editing detail distance while stationary must resize the small-rock
        // window without duplicating or rebuilding the retained world layer.
        let loaded = app.world().resource::<Stream>().loaded.clone();
        for (detail, expected_radius) in [(16.0, 2), (0.0, -1), (64.0, 3)] {
            app.insert_resource(crate::rendering::view_distance::Detail(detail));
            for _ in 0..64 {
                app.update();
            }
            let state = app.world().resource::<Stream>();
            assert_eq!(state.detail_region.unwrap().1, expected_radius);
            assert_eq!(state.loaded, loaded);
            if detail == 0.0 {
                assert!(state.details.is_empty());
            }
            assert!((state.discovery.is_empty() && state.detail_discovery.is_empty()));
        }
        assert!(rocks > 10);
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 1);
        assert_eq!(assets, KINDS * VARIANTS * 2);
    }
}
