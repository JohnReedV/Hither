//! Broad, gnarled oaks: shared seeded morphology, stateless placement and
//! bounded streaming. Giant solitary trees belong exclusively to open plains.
//! No meshes are rebuilt while walking.
use super::*;
use crate::world::streaming::{Coordinator, Layer};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
};

// A quarter of suitable regions retain their solitary landmark.
pub(crate) const CELL: f32 = 288.0;
const CANDIDATES: i32 = 12;
const MARGIN: f32 = 48.0;
const VIEW_DISTANCE: f32 = 420.0;
const CACHE_CAPACITY: usize = 128;
const VARIANTS: usize = 8;

pub(crate) fn region_enabled(cell: IVec2, seed: u64) -> bool {
    hash_for_seed(cell, seed ^ 0x726172655f6f616b) & 3 == 0
}

fn size_multiplier(sample: f32) -> f32 {
    // All sizes remain possible, with the largest ancient oaks less common.
    1.0 + 4.0 * sample.clamp(0.0, 1.0).powf(1.5)
}

fn detail_distance(scale: f32) -> f32 {
    (28.0 * scale).clamp(28.0, 100.0)
}

fn hash(cell: IVec2) -> u64 {
    hash_for_seed(cell, crate::world::biome::world_seed())
}

fn hash_for_seed(cell: IVec2, seed: u64) -> u64 {
    let mut h = seed
        ^ 0x6f616b
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}

fn candidate(cell: IVec2, index: i32, seed: u64) -> (u64, Vec3) {
    let h = hash_for_seed(
        cell * CANDIDATES + IVec2::new(index / CANDIDATES, index % CANDIDATES),
        seed,
    );
    let mut rng = Rng(h);
    let step = (CELL - MARGIN * 2.0) / CANDIDATES as f32;
    let p = cell.as_vec2() * CELL
        + Vec2::splat(MARGIN)
        + (Vec2::new((index / CANDIDATES) as f32, (index % CANDIDATES) as f32)
            + Vec2::new(rng.range(0.15, 0.85), rng.range(0.15, 0.85)))
            * step;
    (h, Vec3::new(p.x, 0.0, p.y))
}

fn eligible(p: Vec3, seed: u64) -> bool {
    !(p.x.abs() < 17.0 && p.z.abs() < 17.0)
        && crate::world::biome::snow_for_seed(p.xz(), seed) <= 0.08
        && crate::world::biome::forest_for_seed(p.xz(), seed) <= 0.05
        && crate::world::biome::mountain_for_seed(p.xz(), seed) == 0.0
}

fn placement_for_seed(cell: IVec2, seed: u64) -> Option<Transform> {
    if !region_enabled(cell, seed) {
        return None;
    }
    // Stratified candidates find actual plains within a mixed-biome region.
    // Lowest seeded priority wins: no exploration-order dependence or clusters.
    let (h, p) = (0..CANDIDATES * CANDIDATES)
        .map(|i| candidate(cell, i, seed))
        .filter(|(_, p)| eligible(*p, seed))
        .min_by_key(|(h, _)| *h)?;
    let mut rng = Rng(h ^ 0x6f616b5f73697a65);
    let base_scale = rng.range(0.95, 1.12);
    let mut size_rng = Rng(hash_for_seed(cell, seed ^ 0x73697a655f6f616b));
    let scale = base_scale * size_multiplier(size_rng.unit());
    Some(
        Transform::from_translation(p)
            .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
            .with_scale(Vec3::splat(scale)),
    )
}

#[derive(Default)]
struct PlacementCache {
    cells: HashMap<IVec2, Option<Transform>>,
    order: VecDeque<IVec2>,
}
impl PlacementCache {
    fn get(&mut self, cell: IVec2) -> Option<Transform> {
        if let Some(value) = self.cells.get(&cell) {
            return *value;
        }
        let value = placement_for_seed(cell, crate::world::biome::world_seed());
        if self.cells.len() == CACHE_CAPACITY {
            self.cells.remove(&self.order.pop_front().unwrap());
        }
        self.cells.insert(cell, value);
        self.order.push_back(cell);
        value
    }
}
type RootCurves = Vec<Vec<(Vec3, f32)>>;
thread_local! {
    // Collision and streaming share deterministic sites without rescanning the
    // biome on every movement substep. Per-thread caches are strictly bounded.
    static PLACEMENTS: RefCell<PlacementCache> = RefCell::default();
    static ROOT_CURVES: RefCell<HashMap<u64, RootCurves>> = RefCell::default();
}
fn placement(cell: IVec2) -> Option<Transform> {
    PLACEMENTS.with_borrow_mut(|cache| cache.get(cell))
}

fn nearby(p: Vec2) -> impl Iterator<Item = Transform> {
    let c = (p / CELL).floor().as_ivec2();
    (-1..=1).flat_map(move |x| (-1..=1).filter_map(move |z| placement(c + IVec2::new(x, z))))
}
pub(crate) fn preview_tree() -> Option<Transform> {
    for r in 0..400_i32 {
        for x in -r..=r {
            for z in -r..=r {
                if x.abs().max(z.abs()) != r {
                    continue;
                }
                if let Some(t) = placement(IVec2::new(x, z))
                    && t.scale.x >= 0.95
                {
                    println!("Solitary oak preview: {}", t.translation);
                    return Some(t);
                }
            }
        }
    }
    None
}
#[cfg(test)]
fn preview_position() -> Option<Vec3> {
    preview_tree().map(|t| t.translation)
}
pub(crate) fn player_collides(p: Vec3, radius: f32) -> bool {
    nearby(p.xz()).any(|t| {
        if p.xz().distance(t.translation.xz()) > radius + 2.7 * t.scale.x {
            return false;
        }
        let cell = (t.translation.xz() / CELL).floor().as_ivec2();
        let variant = hash(cell) as usize % VARIANTS;
        let seed = hash(IVec2::new(variant as i32, -818));
        root_collision(p, radius, &t, seed)
    })
}

// Use the very same tapered, curved paths for visible roots and collision.
fn root_paths(seed: u64) -> [[(Vec3, f32); 3]; 7] {
    let mut rng = Rng(seed);
    let phase = rng.range(0.0, TAU);
    // Match the eight crown/trunk samples preceding roots in generate().
    for _ in 0..8 {
        rng.unit();
    }
    std::array::from_fn(|i| {
        let angle = phase + i as f32 * TAU / 7.0;
        let d = Vec3::new(angle.cos(), 0.0, angle.sin());
        [
            (Vec3::Y * 0.75, 0.30),
            (d * 0.9 + Vec3::Y * 0.1, 0.23),
            (d * rng.range(1.7, 2.3) - Vec3::Y * 0.07, 0.01),
        ]
    })
}

fn root_collision(p: Vec3, radius: f32, transform: &Transform, seed: u64) -> bool {
    let local = transform.rotation.inverse() * (p - transform.translation) / transform.scale.x;
    if local.xz().length() < 0.95 + radius / transform.scale.x {
        return true;
    }
    let feet = p.y - crate::player::movement::PLAYER_EYE_HEIGHT;
    root_surface(p, radius, transform, seed).is_some_and(|height| feet + 0.015 < height)
}

pub(crate) fn support_height(p: Vec3, radius: f32) -> f32 {
    nearby(p.xz())
        .filter_map(|t| {
            if p.xz().distance(t.translation.xz()) > radius + 2.7 * t.scale.x {
                return None;
            }
            let cell = (t.translation.xz() / CELL).floor().as_ivec2();
            let variant = hash(cell) as usize % VARIANTS;
            root_surface(p, radius, &t, hash(IVec2::new(variant as i32, -818)))
        })
        .fold(crate::world::biome::terrain_height(p.xz()), f32::max)
}

fn root_surface(p: Vec3, radius: f32, transform: &Transform, seed: u64) -> Option<f32> {
    let scale = transform.scale.x;
    let local = transform.rotation.inverse() * (p - transform.translation) / scale;
    let margin = radius / scale;
    ROOT_CURVES.with_borrow_mut(|cache| {
        if !cache.contains_key(&seed) && cache.len() >= VARIANTS {
            cache.clear();
        }
        let curves = cache.entry(seed).or_insert_with(|| {
            root_paths(seed)
                .iter()
                .map(|path| curved_path(path))
                .collect()
        });
        curves
            .iter()
            .flat_map(|path| {
                path.windows(2).filter_map(|segment| {
                    let (a, ar) = segment[0];
                    let (b, br) = segment[1];
                    let ab = b.xz() - a.xz();
                    let t = ((local.xz() - a.xz()).dot(ab) / ab.length_squared().max(1e-8))
                        .clamp(0.0, 1.0);
                    let center = a.lerp(b, t);
                    // Include the bark's geometric ridges, but leave spaces between roots open.
                    let wood_radius = (ar + (br - ar) * t) * 1.09;
                    let lateral = (local.xz().distance(center.xz()) - margin).max(0.0);
                    (lateral < wood_radius).then(|| {
                        (center.y + (wood_radius * wood_radius - lateral * lateral).sqrt()) * scale
                            + transform.translation.y
                    })
                })
            })
            .reduce(f32::max)
    })
}
pub(crate) fn camera_obstructed(p: Vec3, radius: f32) -> bool {
    nearby(p.xz()).any(|t| {
        let q = (p - t.translation) / t.scale.x;
        q.y < 4.6 && q.xz().length() < 0.95 + radius / t.scale.x
    })
}

/// A folded blade with rounded paired lobes; explicitly not the serrated
/// apple leaf or pointed, narrow citrus leaf. 33 vertices, 40 triangles.
fn leaf(mesh: &mut Geometry, root: Vec3, direction: Vec3, length: f32, rng: &mut Rng) {
    let axis = direction.normalize_or(Vec3::Y);
    let side = axis.cross(Vec3::Y).normalize_or(Vec3::X);
    let normal = side.cross(axis).normalize();
    let start = mesh.positions.len() as u32;
    let green = Vec3::new(0.075, 0.16, 0.026) * rng.range(0.70, 1.25);
    for row in 0..=10 {
        let t = row as f32 / 10.0;
        let lobe = 0.62 + 0.38 * (t * PI * 10.0).cos();
        let width = (PI * t).sin().max(0.0).powf(0.55) * length * 0.37 * lobe;
        for col in 0..3 {
            let across = col as f32 - 1.0;
            let p = root
                + axis * length * t
                + side * across * width
                + normal * length * (0.08 * (PI * t).sin() - across.abs() * 0.07);
            mesh.vertex(p, Vec2::new(col as f32 * 0.5, t), green * (0.8 + t * 0.2));
            if row > 0 && col > 0 {
                let i = start + (row - 1) * 3 + col - 1;
                mesh.triangle(i, i + 3, i + 1);
                mesh.triangle(i + 1, i + 3, i + 4);
            }
        }
    }
}

fn generate(seed: u64) -> (Geometry, Geometry) {
    let mut rng = Rng(seed);
    let mut wood = Geometry::default();
    let mut leaves = Geometry::default();
    let phase = rng.range(0.0, TAU);
    let spreading_arms = 7 + (rng.unit() * 5.0) as usize;
    let leaders = 3 + (rng.unit() * 3.0) as usize;
    let crown_width = rng.range(1.0, 1.28);
    let crown_height = rng.range(1.0, 1.22);
    let crown_bias = Vec3::new(rng.range(-0.65, 0.65), 0.0, rng.range(-0.65, 0.65));
    let lean = Vec3::new(rng.range(-0.35, 0.35), 0.0, rng.range(-0.35, 0.35));
    branch(
        &mut wood,
        &[
            (Vec3::Y * -0.08, 0.84),
            (Vec3::Y * 1.3, 0.69),
            (Vec3::Y * 2.7 + lean, 0.54),
            (Vec3::Y * 4.3 + lean, 0.28),
            (Vec3::Y * 6.8 + lean * 2.0, 0.025),
        ],
        24,
        phase,
    );
    for (i, path) in root_paths(seed).iter().enumerate() {
        let angle = phase + i as f32 * TAU / 7.0;
        rng.unit(); // Preserve the crown's existing random sequence.
        branch(&mut wood, path, 10, angle);
    }
    for i in 0..spreading_arms + leaders {
        let angle = phase + i as f32 * 2.399963 + rng.range(-0.38, 0.38);
        let d = Vec3::new(angle.cos(), 0.0, angle.sin());
        let side = Vec3::new(-d.z, 0.0, d.x);
        let root = Vec3::Y * rng.range(2.25, 4.0) + lean;
        // Broad lower scaffold arms surround shorter, taller central leaders:
        // a continuous domed crown, not a ring of equally tall forked poles.
        let (reach, height) = if i < spreading_arms {
            (rng.range(3.8, 6.2), rng.range(5.5, 7.5))
        } else {
            (rng.range(1.1, 3.0), rng.range(7.8, 8.8))
        };
        let tip = d * reach * crown_width + Vec3::Y * height * crown_height + crown_bias;
        let bend = side * rng.range(-0.9, 0.9);
        let path = [
            (root, 0.32),
            (
                root + d * rng.range(1.1, 1.9) + Vec3::Y * rng.range(0.15, 0.8) + bend * 0.35,
                0.24,
            ),
            (root.lerp(tip, rng.range(0.55, 0.75)) + bend, 0.14),
            (tip, 0.022),
        ];
        branch(&mut wood, &path, 14, angle);
        let curve = curved_path(&path);
        let forks = 7 + (rng.unit() * 5.0) as usize;
        for j in 0..forks {
            let t = 0.32 + (j as f32 + rng.range(0.1, 0.8)) / forks as f32 * 0.61;
            let base = path_point(&curve, t).0;
            let sign = if j % 2 == 0 { 1.0 } else { -1.0 };
            let end = base
                + side * sign * rng.range(0.7, 2.5)
                + d * rng.range(-0.15, 1.5)
                + Vec3::Y * rng.range(-0.15, 1.4);
            let fork = [
                (base, 0.065),
                (
                    base.lerp(end, rng.range(0.35, 0.65)) - Vec3::Y * rng.range(0.05, 0.4),
                    0.035,
                ),
                (end, 0.005),
            ];
            branch(&mut wood, &fork, 7, angle + j as f32);
            let fork_curve = curved_path(&fork);
            let shoots = 7 + (rng.unit() * 4.0) as usize;
            for k in 0..shoots {
                let b = path_point(&fork_curve, 0.18 + (k as f32 + 0.5) / shoots as f32 * 0.78).0;
                let a = rng.range(0.0, TAU);
                let shoot = Vec3::new(a.cos(), rng.range(0.1, 0.9), a.sin()).normalize();
                let e = b + shoot * rng.range(0.5, 0.95);
                branch(&mut wood, &[(b, 0.006), (e, 0.001)], 4, a);
                for n in 0..18 {
                    let along = 0.12 + n as f32 / 18.0 * 0.88;
                    let a = a + n as f32 * 2.399963;
                    let direction = Vec3::new(a.cos(), rng.range(-0.25, 0.65), a.sin());
                    let length = rng.range(0.22, 0.34);
                    leaf(&mut leaves, b.lerp(e, along), direction, length, &mut rng);
                }
            }
        }
    }
    wood.finish_normals();
    leaves.finish_normals();
    (wood, leaves)
}

#[derive(Resource)]
pub(super) struct OakAssets {
    models: Vec<[Handle<Mesh>; 2]>,
    distant_leaves: Vec<Handle<Mesh>>,
    materials: [Handle<StandardMaterial>; 2],
}
#[derive(Component)]
pub(super) struct OakFoliage {
    variant: usize,
    position: Vec3,
    scale: f32,
    source: Handle<Mesh>,
}

// Keep every leaf and the full canopy, but collapse each distant folded blade
// to two triangles. This cuts foliage triangles by 20x without bald LOD crowns.
fn distant_foliage(leaves: &Geometry) -> Geometry {
    let mut mesh = Geometry::default();
    for start in (0..leaves.positions.len()).step_by(33) {
        let widest = (1..10)
            .max_by(|a, b| {
                let width = |row: usize| {
                    Vec3::from(leaves.positions[start + row * 3])
                        .distance_squared(Vec3::from(leaves.positions[start + row * 3 + 2]))
                };
                width(*a).total_cmp(&width(*b))
            })
            .unwrap();
        let n = mesh.positions.len() as u32;
        for i in [
            start + 1,
            start + widest * 3,
            start + 31,
            start + widest * 3 + 2,
        ] {
            mesh.vertex(
                Vec3::from(leaves.positions[i]),
                Vec2::from(leaves.uvs[i]),
                Vec4::from(leaves.colors[i]).truncate(),
            );
            *mesh.normals.last_mut().unwrap() = leaves.normals[i];
        }
        mesh.triangle(n, n + 1, n + 2);
        mesh.triangle(n, n + 2, n + 3);
    }
    mesh
}
#[derive(Resource, Default)]
pub(super) struct Streaming {
    center: Option<IVec2>,
    position: Option<Vec3>,
    cells: HashMap<IVec2, Entity>,
    deferred: bool,
}

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let started = std::time::Instant::now();
    let trees: Vec<_> = std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..VARIANTS)
            .map(|i| scope.spawn(move || generate(hash(IVec2::new(i as i32, -818)))))
            .collect();
        jobs.into_iter()
            .map(|job| job.join().expect("oak generation"))
            .collect()
    });
    let distant_leaves = trees
        .iter()
        .map(|(_, leaves)| meshes.add(distant_foliage(leaves).mesh()))
        .collect();
    let models = trees
        .into_iter()
        .map(|(wood, leaves)| [meshes.add(wood.mesh()), meshes.add(leaves.mesh())])
        .collect();
    let bark = materials.add(material(SurfaceKind::Bark, &mut images));
    let foliage = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.92,
        reflectance: 0.15,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(OakAssets {
        models,
        distant_leaves,
        materials: [bark, foliage],
    });
    commands.init_resource::<Streaming>();
    info!("Oak assets ready in {:?}", started.elapsed());
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stream(
    mut coordinator: ResMut<Coordinator>,
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    range: Option<Res<crate::rendering::view_distance::Range>>,
    mut previous_distance: Local<f32>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    view: Option<Res<crate::player::camera::CameraView>>,
    assets: Res<OakAssets>,
    mut state: ResMut<Streaming>,
    mut foliage: Query<(&mut OakFoliage, &mut Mesh3d)>,
) {
    let camera_position = view.as_ref().map_or(rig.position, |v| v.position);
    let distance = range.as_ref().map_or(VIEW_DISTANCE, |r| r.load);
    let radius = ((distance + 80.0) / CELL).ceil() as i32 + 1;
    let center = (rig.position.xz() / CELL).floor().as_ivec2();
    if !state.deferred
        && state.center == Some(center)
        && !detail.as_ref().is_some_and(|d| d.is_changed())
        && *previous_distance == distance
        && state
            .position
            .is_some_and(|p| p.distance_squared(camera_position) < 4.0)
    {
        return;
    }
    state.center = Some(center);
    *previous_distance = distance;
    state.position = Some(camera_position);
    for (mut part, mut mesh) in &mut foliage {
        let handle = if part.position.distance(camera_position)
            < detail.as_ref().map_or_else(
                || detail_distance(part.scale),
                |d| {
                    if d.0 > 0.0 {
                        d.0 + 14.0 * part.scale
                    } else {
                        0.0
                    }
                },
            ) {
            assets.models[part.variant][1].clone()
        } else {
            assets.distant_leaves[part.variant].clone()
        };
        if part.source != handle {
            part.source = handle.clone();
            mesh.0 = handle;
        }
    }
    state.deferred = false;
    let mut wanted = HashSet::new();
    for x in -radius..=radius {
        for z in -radius..=radius {
            let cell = center + IVec2::new(x, z);
            let Some(transform) = placement(cell) else {
                continue;
            };
            if transform.translation.xz().distance(rig.position.xz())
                > distance + 14.0 * transform.scale.x
            {
                continue;
            }
            wanted.insert(cell);
            let _installation = if !state.cells.contains_key(&cell) {
                coordinator.install_entities(Layer::Oaks, 4, 0, 5)
            } else {
                None
            };
            if !state.cells.contains_key(&cell) && _installation.is_none() {
                state.deferred = true;
                continue;
            }
            state.cells.entry(cell).or_insert_with(|| {
                let variant = hash(cell) as usize % VARIANTS;
                let model = &assets.models[variant];
                commands
                    .spawn((Name::new("Spreading oak"), transform, Visibility::Inherited))
                    .with_children(|parent| {
                        for (part, mesh) in model.iter().enumerate() {
                            parent.spawn((
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(assets.materials[part].clone()),
                                Transform::default(),
                                super::lod::shadow_layers(),
                                crate::rendering::plant_lod::ShadowProxy,
                            ));
                        }
                        parent.spawn((
                            bevy::light::NotShadowCaster,
                            Mesh3d(model[0].clone()),
                            MeshMaterial3d(assets.materials[0].clone()),
                            Transform::default(),
                        ));
                        let mesh = if transform.translation.distance(camera_position)
                            < detail.as_ref().map_or_else(
                                || detail_distance(transform.scale.x),
                                |d| {
                                    if d.0 > 0.0 {
                                        d.0 + 14.0 * transform.scale.x
                                    } else {
                                        0.0
                                    }
                                },
                            ) {
                            model[1].clone()
                        } else {
                            assets.distant_leaves[variant].clone()
                        };
                        parent.spawn((
                            bevy::light::NotShadowCaster,
                            crate::rendering::plant_lod::Canopy(true),
                            OakFoliage {
                                variant,
                                position: transform.translation,
                                scale: transform.scale.x,
                                source: mesh.clone(),
                            },
                            Mesh3d(mesh),
                            MeshMaterial3d(assets.materials[1].clone()),
                            Transform::default(),
                        ));
                    })
                    .id()
            });
        }
    }
    state.cells.retain(|cell, entity| {
        if wanted.contains(cell) {
            true
        } else {
            crate::world::streaming::retire(&mut commands, *entity);
            false
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_is_bounded_preserves_mesh_assets_and_restores_trees() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(crate::player::camera::CameraRig::default())
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Oaks));
        app.update();
        let original: HashSet<_> = app
            .world()
            .resource::<Streaming>()
            .cells
            .keys()
            .copied()
            .collect();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), VARIANTS * 3);
        let mut saw_tree = false;
        for p in [
            preview_position().unwrap(),
            Vec3::new(700.0, 1.25, -500.0),
            crate::player::camera::CameraRig::default().position,
        ] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = p;
            app.update();
            let count = app.world().resource::<Streaming>().cells.len();
            saw_tree |= count > 0;
            assert!(count <= 25);
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), VARIANTS * 3);
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, (
                        With<Transform>,
                        Without<crate::rendering::plant_lod::ShadowProxy>
                    )>()
                    .iter(app.world())
                    .count(),
                count * 3
            );
        }
        assert!(saw_tree);
        assert_eq!(
            original,
            app.world()
                .resource::<Streaming>()
                .cells
                .keys()
                .copied()
                .collect()
        );
        let tree = preview_position().unwrap();
        let region = (tree.xz() / CELL).floor().as_ivec2();
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position = tree + Vec3::X * 250.0;
        app.update();
        assert!(
            app.world()
                .resource::<Streaming>()
                .cells
                .contains_key(&region),
            "landmark must render well beyond the old 80 m window"
        );
        for distance in [35.0, 25.0] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = tree + Vec3::X * distance;
            app.update();
            assert_eq!(app.world().resource::<Streaming>().center, Some(region));
            let world = app.world_mut();
            let mut query = world.query::<(&OakFoliage, &Mesh3d)>();
            let (part, mesh) = query
                .iter(world)
                .find(|(part, _)| part.position == tree)
                .unwrap();
            let assets = world.resource::<OakAssets>();
            assert_eq!(
                mesh.0,
                if distance < detail_distance(part.scale) {
                    assets.models[part.variant][1].clone()
                } else {
                    assets.distant_leaves[part.variant].clone()
                }
            );
            let authored = mesh.0.clone();
            // Simulate the shared selector choosing a simplified displayed mesh.
            let mut query = world.query::<(&OakFoliage, &mut Mesh3d)>();
            query
                .iter_mut(world)
                .find(|(part, _)| part.position == tree)
                .unwrap()
                .1
                .0 = Handle::default();
            world.resource_mut::<Streaming>().deferred = true;
            app.update();
            let world = app.world_mut();
            let mut query = world.query::<(&OakFoliage, &Mesh3d)>();
            assert_eq!(
                query
                    .iter(world)
                    .find(|(part, _)| part.position == tree)
                    .unwrap()
                    .1
                    .0,
                Handle::default()
            );
            let mut query = world.query::<(&OakFoliage, &mut Mesh3d)>();
            query
                .iter_mut(world)
                .find(|(part, _)| part.position == tree)
                .unwrap()
                .1
                .0 = authored;
        }
    }
    #[test]
    fn giant_oaks_are_rare_and_exclusively_in_plains() {
        for seed in [0, 1, 2, 721, 1973, u64::MAX] {
            let mut giants = 0;
            let mut eligible_regions = 0;
            for x in -6..=6 {
                for z in -6..=6 {
                    let cell = IVec2::new(x, z);
                    let suitable = (0..CANDIDATES * CANDIDATES)
                        .any(|i| eligible(candidate(cell, i, seed).1, seed));
                    eligible_regions += usize::from(suitable && region_enabled(cell, seed));
                    let tree = placement_for_seed(cell, seed);
                    assert_eq!(tree, placement_for_seed(cell, seed));
                    assert_eq!(
                        tree.is_some(),
                        suitable && region_enabled(cell, seed),
                        "no random drought in suitable region {cell:?}, seed {seed}"
                    );
                    if let Some(t) = tree {
                        giants += 1;
                        assert!(eligible(t.translation, seed));
                        assert!((0.95..=5.6).contains(&t.scale.x));
                        let local = t.translation.xz() - cell.as_vec2() * CELL;
                        assert!(local.min_element() >= MARGIN);
                        assert!(local.max_element() <= CELL - MARGIN);
                        // Border margins give adjacent solitary oaks at least
                        // 96 m of growing room, including negative coordinates.
                        for offset in [IVec2::X, IVec2::Y, IVec2::ONE] {
                            if let Some(other) = placement_for_seed(cell + offset, seed) {
                                assert!(t.translation.distance(other.translation) >= MARGIN * 2.0);
                            }
                        }
                    }
                }
            }
            assert_eq!(giants, eligible_regions);
            assert!((5..80).contains(&giants), "seed {seed}: {giants}");
        }
    }
    #[test]
    fn player_lands_jumps_and_walks_on_root_surface() {
        let tree = preview_tree().unwrap();
        let cell = (tree.translation.xz() / CELL).floor().as_ivec2();
        let variant = hash(cell) as usize % VARIANTS;
        let seed = hash(IVec2::new(variant as i32, -818));
        let path = root_paths(seed)[0];
        let p = tree.transform_point(path[1].0.lerp(path[2].0, 0.6));
        let height = support_height(p, crate::player::movement::PLAYER_RADIUS);
        assert!(height > 0.0);
        let mut rig = crate::player::camera::CameraRig {
            position: Vec3::new(
                p.x,
                height + 2.0 + crate::player::movement::PLAYER_EYE_HEIGHT,
                p.z,
            ),
            jump_height: height + 2.0,
            grounded: false,
            ..default()
        };
        let mut keys = ButtonInput::<KeyCode>::default();
        for _ in 0..120 {
            crate::player::movement::update_jump(&mut rig, &keys, 1.0 / 60.0);
        }
        assert!(rig.grounded);
        assert!((rig.jump_height - height).abs() < 0.001);
        assert!(!player_collides(
            Vec3::new(
                p.x,
                height + crate::player::movement::PLAYER_EYE_HEIGHT,
                p.z
            ),
            crate::player::movement::PLAYER_RADIUS
        ));
        keys.press(KeyCode::Space);
        crate::player::movement::update_jump(&mut rig, &keys, 1.0 / 60.0);
        assert!(!rig.grounded);
        assert!(rig.jump_height > height);

        // Climb a gentle part of the same root without pressing jump.
        let direction = tree.rotation * (path[1].0 - path[2].0).with_y(0.0).normalize();
        let start = Vec3::new(
            p.x,
            height + crate::player::movement::PLAYER_EYE_HEIGHT,
            p.z,
        );
        let end = crate::player::movement::move_player_with_collisions(start, direction * 0.4);
        assert!(end.xz().distance(start.xz()) > 0.1);
        assert!(end.y > start.y);
    }

    #[test]
    fn roots_block_players_at_all_sizes_and_rotations_but_not_clear_gaps() {
        for seed in [1, 721, 9918] {
            for scale in [0.95, 2.5, 5.6] {
                for yaw in [0.0, 1.7, 3.4] {
                    let transform = Transform::from_xyz(-120.0, 0.0, 75.0)
                        .with_scale(Vec3::splat(scale))
                        .with_rotation(Quat::from_rotation_y(yaw));
                    for path in root_paths(seed) {
                        let local = path[1].0.lerp(path[2].0, 0.55);
                        let ground = transform.transform_point(Vec3::new(local.x, 0.0, local.z));
                        let player = ground + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
                        assert!(root_collision(player, 0.28, &transform, seed));
                        assert!(!root_collision(
                            player + Vec3::Y * (scale + 1.0),
                            0.28,
                            &transform,
                            seed
                        ));
                    }
                    let phase = Rng(seed).range(0.0, TAU) + TAU / 14.0;
                    let gap = Vec3::new(phase.cos(), 0.0, phase.sin()) * 1.9;
                    assert!(!root_collision(
                        transform.transform_point(gap)
                            + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
                        0.28,
                        &transform,
                        seed
                    ));
                }
            }
        }
    }

    #[test]
    fn placement_cache_is_bounded_and_evicted_sites_replay_exactly() {
        let mut cache = PlacementCache::default();
        let first = cache.get(IVec2::ZERO);
        for x in 0..300 {
            cache.get(IVec2::new(x, -3));
        }
        assert_eq!(cache.cells.len(), CACHE_CAPACITY);
        assert_eq!(cache.order.len(), CACHE_CAPACITY);
        assert_eq!(cache.get(IVec2::ZERO), first);
        let tree = preview_position().unwrap();
        assert!(player_collides(tree + Vec3::Y, 0.28));
        assert!(camera_obstructed(tree + Vec3::Y, 0.1));
        assert!(!player_collides(tree + Vec3::X * 7.0 + Vec3::Y, 0.28));
    }
    #[test]
    fn quarter_the_suitable_regions_remain_and_sizes_span_one_to_five_times() {
        assert_eq!(size_multiplier(0.0), 1.0);
        assert_eq!(size_multiplier(1.0), 5.0);
        let (mut suitable, mut previously_kept, mut kept) = (0, 0, 0);
        let (mut smallest, mut largest) = (5.0_f32, 1.0_f32);
        for seed in [1, 2, 721, u64::MAX] {
            for x in -15..15 {
                for z in -15..15 {
                    let cell = IVec2::new(x, z);
                    let valid = (0..CANDIDATES * CANDIDATES)
                        .any(|i| eligible(candidate(cell, i, seed).1, seed));
                    suitable += usize::from(valid);
                    previously_kept += usize::from(
                        valid && hash_for_seed(cell, seed ^ 0x726172655f6f616b) & 1 == 0,
                    );
                    if let Some(t) = placement_for_seed(cell, seed) {
                        kept += 1;
                        let (h, _) = (0..CANDIDATES * CANDIDATES)
                            .map(|i| candidate(cell, i, seed))
                            .filter(|(_, p)| eligible(*p, seed))
                            .min_by_key(|(h, _)| *h)
                            .unwrap();
                        let base = Rng(h ^ 0x6f616b5f73697a65).range(0.95, 1.12);
                        let multiplier = t.scale.x / base;
                        assert!((1.0..=5.0).contains(&multiplier));
                        smallest = smallest.min(multiplier);
                        largest = largest.max(multiplier);
                    }
                }
            }
        }
        let rate = kept as f32 / suitable as f32;
        assert!((0.22..0.28).contains(&rate), "{kept}/{suitable}");
        assert!(
            (0.44..0.56).contains(&(kept as f32 / previously_kept as f32)),
            "{kept}/{previously_kept}"
        );
        assert!(smallest < 1.05 && largest > 4.95);
    }
    #[test]
    fn oak_geometry_is_large_lobed_and_seeded() {
        let (wood, leaves) = generate(123);
        assert_ne!(wood.positions, generate(456).0.positions);
        assert!(leaves.positions.len() > 150000);
        assert_ne!(
            wood.positions.len(),
            generate(456).0.positions.len(),
            "branch topology must vary, not just orientation"
        );
        assert!(wood.positions.iter().any(|p| p[1] > 8.0));
        let distant = distant_foliage(&leaves);
        assert_eq!(distant.positions.len() / 4, leaves.positions.len() / 33);
        assert_eq!(distant.indices.len() * 20, leaves.indices.len());
        assert!(distant.positions.iter().all(|p| Vec3::from(*p).is_finite()));
        for g in [wood, leaves] {
            assert!(g.positions.iter().all(|p| Vec3::from(*p).is_finite()));
            assert!(g.normals.iter().all(|p| Vec3::from(*p).is_finite()));
            assert!(g.indices.iter().all(|i| (*i as usize) < g.positions.len()));
        }
    }
}
