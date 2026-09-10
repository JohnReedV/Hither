//! Sparse, per-site fallen timber. Branch topology is generated per log, never
//! selected from a fixed branch bank. Streaming owns and releases unique meshes.
use super::*;
use crate::world::streaming::{Coordinator, Layer};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Species {
    Orange,
    Apple,
    Spruce,
}

fn mix(mut h: u64) -> u64 {
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}
fn seed(cell: IVec2, winter: bool) -> u64 {
    mix(crate::world::biome::world_seed()
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b)
        ^ if winter {
            0x737072756365
        } else {
            0x66616c6c656e
        })
}
fn selected(seed: u64) -> bool {
    mix(seed ^ 0x64656e73697479).is_multiple_of(50)
}
fn mossy(seed: u64, species: Species) -> bool {
    species == Species::Orange && mix(seed ^ 0x6d6f7373).is_multiple_of(50)
}
#[derive(Clone)]
struct Log {
    seed: u64,
    species: Species,
    transform: Transform,
    length: f32,
    radius: f32,
    form: usize,
}
fn dimensions(seed: u64, species: Species) -> (f32, f32, usize) {
    let mut rng = Rng(mix(seed ^ 0x7368617065));
    let form = usize::from(rng.unit() > 0.5);
    let (length, radius) = match species {
        Species::Orange => (2.0, 0.20),
        Species::Apple => (2.25, 0.25),
        Species::Spruce => (3.2, 0.24),
    };
    (
        length * rng.range(0.85, 1.2) * if form == 0 { 1.0 } else { 0.70 },
        radius * rng.range(0.85, 1.15) * if form == 0 { 1.0 } else { 1.22 },
        form,
    )
}
fn placement(cell: IVec2, winter: bool) -> Option<Log> {
    let seed = seed(cell, winter);
    if !selected(seed) {
        return None;
    }
    let (tree, species) = if winter {
        (winter::log_source(cell)?, Species::Spruce)
    } else {
        wild::log_source(cell)?
    };
    let (length, radius, form) = dimensions(seed, species);
    let mut rng = Rng(mix(seed ^ 0x706f7365));
    let yaw = rng.range(0.0, TAU);
    let side = Vec2::new(-yaw.sin(), -yaw.cos());
    // Lay the log beside its source tree, not through the standing trunk.
    let center = tree.xz() + side * rng.range(1.20, 1.65);
    if center.x.abs() < 11.0 && center.y.abs() < 11.0 {
        return None;
    }
    let direction = Vec2::new(yaw.cos(), -yaw.sin());
    let a = center - direction * length * 0.5;
    let b = center + direction * length * 0.5;
    let ha = crate::world::terrain::height(a);
    let hb = crate::world::terrain::height(b);
    if (hb - ha).abs() > length * 0.45 {
        return None;
    }
    let mut ground = 0.0_f32;
    for i in 0..=8 {
        let t = i as f32 / 8.0;
        let p = a.lerp(b, t);
        if !winter && !wild::log_habitat(p, species) {
            return None;
        }
        let h = crate::world::terrain::height(p);
        if !h.is_finite()
            || h > 50.0
            || crate::world::orcs::entrance_overlap(Vec3::new(p.x, h, p.y), radius + 0.3)
        {
            return None;
        }
        let delta = h - (ha + (hb - ha) * t);
        if delta.abs() > radius * 0.8 {
            return None;
        }
        ground = ground.max(delta);
    }
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(((hb - ha) / length).atan());
    Some(Log {
        seed,
        species,
        length,
        radius,
        form,
        transform: Transform::from_translation(Vec3::new(
            center.x,
            (ha + hb) * 0.5 + radius * 0.80 + ground,
            center.y,
        ))
        .with_rotation(rotation),
    })
}

fn centerline(t: f32, length: f32, form: usize) -> Vec3 {
    Vec3::new(
        (t - 0.5) * length,
        (t * PI).sin() * 0.025,
        (t * PI).sin() * if form == 0 { 0.035 } else { 0.10 },
    )
}
fn ring_point(t: f32, angle: f32, log: &Log) -> Vec3 {
    let phase = (log.seed % 1024) as f32;
    let ridge = 1.0
        + 0.025 * (angle * 13.0 + phase + t * 2.0).sin()
        + 0.010 * (angle * 23.0 + t * 7.0).sin();
    let r = log.radius * (1.0 - 0.22 * t) * ridge;
    let jag = (angle * 7.0 + phase).sin() * 0.035 + (angle * 17.0).sin() * 0.018;
    centerline(t, log.length, log.form)
        + Vec3::new(
            jag * (2.0 * t - 1.0).powi(7),
            angle.cos() * r,
            angle.sin() * r,
        )
}
fn generate(log: &Log) -> [Geometry; 3] {
    let mut bark = Geometry::default();
    let mut wood = Geometry::default();
    let mut moss = Geometry::default();
    const SIDES: usize = 48;
    const ROWS: usize = 24;
    for row in 0..=ROWS {
        let t = row as f32 / ROWS as f32;
        for col in 0..=SIDES {
            let u = col as f32 / SIDES as f32;
            let p = ring_point(t, u * TAU, log);
            let weather = 0.70 + 0.22 * (u * 37.0 + t * 12.0).sin().abs();
            bark.vertex(
                p,
                Vec2::new(u * log.radius * 12.0, t * log.length * 1.5),
                Vec3::splat(weather),
            );
            if row > 0 && col > 0 {
                let a = ((row - 1) * (SIDES + 1) + col - 1) as u32;
                let b = a + (SIDES + 1) as u32;
                bark.triangle(a, a + 1, b);
                bark.triangle(a + 1, b + 1, b);
            }
        }
    }
    let tone = match log.species {
        Species::Orange => Vec3::new(0.57, 0.40, 0.21),
        Species::Apple => Vec3::new(0.48, 0.29, 0.16),
        Species::Spruce => Vec3::new(0.66, 0.49, 0.29),
    };
    // Separate vertices preserve the broken end grain's sharp bark rim. Dense
    // concentric bands make irregular annual rings, radial checks and sapwood.
    for end in [0.0, 1.0] {
        let base = wood.positions.len() as u32;
        let center = centerline(end, log.length, log.form);
        const RINGS: usize = 40;
        for row in 0..=RINGS {
            let r = row as f32 / RINGS as f32;
            for col in 0..=SIDES {
                let a = col as f32 / SIDES as f32 * TAU;
                let rim = ring_point(end, a, log);
                let p = center.lerp(rim, r)
                    + Vec3::X * (end * 2.0 - 1.0) * 0.008 * (a * 11.0 + r * 24.0).sin() * r;
                let grain = 0.78 + 0.16 * (r * 115.0 + (a * 3.0).sin() * 0.7).sin();
                let check =
                    (a * 7.0 + (log.seed % 31) as f32).sin().abs() < 0.055 && r > 0.32 && r < 0.96;
                let c = tone
                    * if check {
                        0.32
                    } else {
                        grain + if r > 0.85 { 0.14 } else { 0.0 }
                    };
                wood.vertex(p, Vec2::new(r * a.cos(), r * a.sin()), c);
                if row > 0 && col > 0 {
                    let a = base + ((row - 1) * (SIDES + 1) + col - 1) as u32;
                    let b = a + (SIDES + 1) as u32;
                    if end == 0.0 {
                        wood.triangle(a, a + 1, b);
                        wood.triangle(a + 1, b + 1, b);
                    } else {
                        wood.triangle(a, b, a + 1);
                        wood.triangle(a + 1, b, b + 1);
                    }
                }
            }
        }
    }
    let mut rng = Rng(mix(log.seed ^ 0x6272616e63686573));
    // Some trunks are clean. Others retain 1–3 individually grown broken limbs.
    let count = if rng.unit() < 0.40 {
        0
    } else {
        1 + (rng.unit() * 2.99) as usize
    };
    for _ in 0..count {
        let t = rng.range(0.14, 0.86);
        let a = rng.range(-1.5, 1.5);
        let root = ring_point(t, a, log);
        let dir = Vec3::new(rng.range(-0.6, 0.6), a.cos(), a.sin()).normalize();
        let length = rng.range(0.19, 0.72);
        let radius = log.radius * rng.range(0.15, 0.28);
        let elbow = root + dir * length * 0.55 + Vec3::X * rng.range(-0.12, 0.12);
        let tip = root + dir * length;
        branch(
            &mut bark,
            &[
                (root - dir * radius, radius),
                (elbow, radius * 0.64),
                (tip, radius * 0.30),
            ],
            9,
            rng.range(0.0, TAU),
        );
        if rng.unit() < 0.35 {
            branch(
                &mut bark,
                &[
                    (elbow, radius * 0.45),
                    (
                        elbow + Vec3::new(rng.range(-0.20, 0.20), 0.14, rng.range(-0.20, 0.20)),
                        radius * 0.07,
                    ),
                ],
                6,
                rng.range(0.0, TAU),
            );
        }
    }
    // Splintered shorter form: exposed longitudinal fibers at the fractured end.
    if log.form == 1 {
        for _ in 0..9 {
            let a = rng.range(0.0, TAU);
            let base = ring_point(1.0, a, log) * Vec3::new(1.0, 0.85, 0.85);
            let tip = base + Vec3::X * rng.range(0.04, 0.18);
            let first = wood.vertex(base + Vec3::Y * 0.018, Vec2::ZERO, tone * 0.8);
            wood.vertex(base + Vec3::Z * 0.025, Vec2::X, tone);
            wood.vertex(tip, Vec2::Y, tone * 1.1);
            wood.triangle(first, first + 1, first + 2);
            wood.triangle(first + 2, first + 1, first);
        }
    }
    if mossy(log.seed, log.species) {
        // Contiguous ragged mats follow the bark relief. Fine lifted facets
        // break up the silhouette without reading as round decals or leaves.
        const ROWS: usize = 72;
        const COLS: usize = 28;
        let phase = (log.seed % 79) as f32;
        for row in 0..=ROWS {
            let t = 0.08 + 0.84 * row as f32 / ROWS as f32;
            for col in 0..=COLS {
                let a = -1.10 + 2.20 * col as f32 / COLS as f32;
                let fuzz = rng.unit();
                let normal = Vec3::new(0.0, a.cos(), a.sin());
                let p = ring_point(t, a, log) + normal * (0.005 + fuzz * 0.008);
                let color = Vec3::new(0.035, 0.070, 0.010) * (0.65 + fuzz * 0.65);
                moss.vertex(p, Vec2::new(t, a), color);
                let edge = (t * 22.0 + phase + a * 2.0).sin()
                    + 0.45 * (a * 9.0 - t * 13.0).sin()
                    + 0.20 * (t * 95.0 + a * 17.0).sin();
                if row > 0 && col > 0 && edge > 0.05 + a.abs() * 0.45 {
                    let b = ((row - 1) * (COLS + 1) + col - 1) as u32;
                    let c = b + (COLS + 1) as u32;
                    moss.triangle(b, b + 1, c);
                    moss.triangle(b + 1, c + 1, c);
                }
            }
        }
    }
    bark.finish_normals();
    wood.finish_normals();
    moss.finish_normals();
    [bark, wood, moss]
}

#[derive(Resource)]
pub(super) struct LogAssets {
    bark: [Handle<StandardMaterial>; 2],
    detail: Handle<StandardMaterial>,
}
type ChunkEntries = Vec<(Entity, Vec<Handle<Mesh>>)>;

#[derive(Resource, Default)]
pub(super) struct Streaming {
    center: Option<(IVec2, i32)>,
    chunks: HashMap<IVec2, ChunkEntries>,
    pending: VecDeque<IVec2>,
    discovery: crate::world::streaming::WindowScan,
    jobs: HashMap<IVec2, crate::world::streaming::BuildTask<BuiltLogs>>,
}
pub(super) fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let citrus = materials.add(material(SurfaceKind::CitrusBark, &mut images));
    let bark = materials.add(material(SurfaceKind::Bark, &mut images));
    let detail = materials.add(StandardMaterial {
        perceptual_roughness: 0.98,
        reflectance: 0.12,
        ..default()
    });
    commands.insert_resource(LogAssets {
        bark: [citrus, bark],
        detail,
    });
    commands.init_resource::<Streaming>();
}
pub(super) fn stream(
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    range: Res<crate::rendering::view_distance::Range>,
    assets: Res<LogAssets>,
    mut state: ResMut<Streaming>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    const SIZE: f32 = 15.0;
    let center = (rig.position.xz() / SIZE).floor().as_ivec2();
    let radius = ((range.load + 5.0) / SIZE).ceil() as i32;
    if state.center != Some((center, radius)) {
        state.center = Some((center, radius));
        state.chunks.retain(|cell, entries| {
            if (*cell - center).abs().max_element() <= radius {
                return true;
            }
            for (entity, handles) in entries {
                crate::world::streaming::retire(&mut commands, *entity);
                for h in handles {
                    meshes.remove(h.id());
                }
            }
            false
        });
        state
            .pending
            .retain(|cell| (*cell - center).abs().max_element() <= radius);
        state.discovery.update(center, radius);
    }
    state
        .jobs
        .retain(|cell, task| (*cell - center).abs().max_element() <= radius || !task.is_ready());
    if state.pending.len() < 16
        && let Some(_scope) = coordinator.install(Layer::Logs, 1, 0)
    {
        for _ in 0..16 {
            let Some(cell) = state.discovery.next() else {
                break;
            };
            if (cell - center).abs().max_element() <= radius
                && !state.chunks.contains_key(&cell)
                && !state.pending.contains(&cell)
            {
                state.pending.push_back(cell);
            }
        }
    }
    let mut ready = Vec::new();
    for (&cell, task) in &mut state.jobs {
        if task.is_ready() && (cell - center).abs().max_element() <= radius {
            ready.push(cell);
        }
    }
    for chunk in ready {
        let task = state.jobs.get_mut(&chunk).unwrap();
        let Some((_, geometry)) = task.ready_ref().unwrap().last() else {
            state.jobs.remove(&chunk);
            state.chunks.entry(chunk).or_default();
            continue;
        };
        let bytes = geometry
            .iter()
            .map(|(_, mesh)| crate::world::streaming::mesh_bytes(mesh))
            .sum();
        let Some(_installation) =
            coordinator.install_entities(Layer::Logs, 4, bytes, geometry.len() + 1)
        else {
            break;
        };
        let logs = [task.ready_mut().unwrap().pop().unwrap()];
        if task.ready_ref().unwrap().is_empty() {
            state.jobs.remove(&chunk);
        }
        let mut entries = Vec::new();
        for (log, geometry) in logs {
            let mut handles = Vec::new();
            let entity = commands
                .spawn((
                    Name::new(format!(
                        "{:?} fallen log / form {}",
                        log.species,
                        log.form + 1
                    )),
                    log.transform,
                    Visibility::Inherited,
                ))
                .with_children(|parent| {
                    for (i, mesh) in geometry {
                        let handle = meshes.add(mesh);
                        handles.push(handle.clone());
                        let material = if i == 0 {
                            assets.bark[usize::from(log.species != Species::Orange)].clone()
                        } else {
                            assets.detail.clone()
                        };
                        parent.spawn((
                            Mesh3d(handle),
                            MeshMaterial3d(material),
                            Transform::default(),
                        ));
                    }
                })
                .id();
            entries.push((entity, handles));
        }
        state.chunks.entry(chunk).or_default().extend(entries);
    }
    while state.jobs.len() < 2 {
        let Some(chunk) = state.pending.pop_front() else {
            break;
        };
        if state.chunks.contains_key(&chunk) || state.jobs.contains_key(&chunk) {
            continue;
        }
        let Some(permit) = coordinator.worker(Layer::Logs) else {
            state.pending.push_front(chunk);
            break;
        };
        state.jobs.insert(
            chunk,
            crate::world::streaming::BuildTask::new(permit, move || build_chunk(chunk)),
        );
    }
}

type BuiltLogs = Vec<(Log, Vec<(usize, Mesh)>)>;
fn build_chunk(chunk: IVec2) -> BuiltLogs {
    const SIZE: f32 = 15.0;
    let mut result = Vec::new();
    for winter in [false, true] {
        let spacing = if winter { 3.6 } else { 3.0 };
        let min = (chunk.as_vec2() * SIZE / spacing).ceil().as_ivec2();
        let max = ((chunk + IVec2::ONE).as_vec2() * SIZE / spacing)
            .ceil()
            .as_ivec2();
        for x in min.x..max.x {
            for z in min.y..max.y {
                let Some(log) = placement(IVec2::new(x, z), winter) else {
                    continue;
                };
                let meshes = generate(&log)
                    .into_iter()
                    .enumerate()
                    .filter(|(_, geometry)| !geometry.indices.is_empty())
                    .map(|(i, geometry)| (i, geometry.mesh()))
                    .collect();
                result.push((log, meshes));
            }
        }
    }
    result
}

/// Share stateless placement between standing support and collision, including
/// logs whose visible meshes have not streamed in yet.
fn nearby<T>(point: Vec3, query: impl FnOnce(&[Log]) -> T) -> T {
    // Walking and camera sweeps revisit the same cells many times per frame.
    // Keep terrain/habitat queries out of repeated collision probes.
    thread_local! {
        static NEARBY: std::cell::RefCell<([IVec2; 2], Vec<Log>)> = const {
            std::cell::RefCell::new(([IVec2::MAX; 2], Vec::new()))
        };
    }
    let centers = [
        (point.xz() / 3.0).floor().as_ivec2(),
        (point.xz() / 3.6).floor().as_ivec2(),
    ];
    NEARBY.with_borrow_mut(|cache| {
        if cache.0 != centers {
            cache.0 = centers;
            cache.1.clear();
            for (index, winter) in [false, true].into_iter().enumerate() {
                for x in -2..=2 {
                    for z in -2..=2 {
                        if let Some(log) = placement(centers[index] + IVec2::new(x, z), winter) {
                            cache.1.push(log);
                        }
                    }
                }
            }
        }
        query(&cache.1)
    })
}

impl Log {
    /// A smooth swept trunk, with a rounded player base. Subtracting the
    /// player's radius keeps feet on the wood instead of floating above it.
    /// Use world-space centers so tilted and curved trunks agree in both queries.
    fn contacts(&self, point: Vec3, radius: f32) -> impl Iterator<Item = (f32, f32)> + '_ {
        (0..=48).filter_map(move |i| {
            let t = i as f32 / 48.0;
            let axis = self
                .transform
                .transform_point(centerline(t, self.length, self.form));
            let wood_radius = self.radius * (1.0 - 0.22 * t);
            let reach = wood_radius + radius;
            let remaining = reach * reach - axis.xz().distance_squared(point.xz());
            (remaining >= 0.0).then(|| {
                let extent = remaining.sqrt();
                (axis.y - extent + radius, axis.y + extent - radius)
            })
        })
    }

    fn support(&self, point: Vec3, radius: f32) -> Option<f32> {
        self.contacts(point, radius)
            .map(|(_, top)| top)
            .max_by(f32::total_cmp)
    }

    fn blocks(&self, feet: Vec3, radius: f32, height: f32) -> bool {
        self.contacts(feet, radius)
            .any(|(bottom, top)| feet.y < top - 0.002 && feet.y + height > bottom + 0.002)
    }
}

pub(crate) fn support(point: Vec3, radius: f32) -> Option<f32> {
    let feet = point.y - crate::player::movement::PLAYER_EYE_HEIGHT;
    nearby(point, |logs| {
        logs.iter()
            .filter_map(|log| log.support(point, radius))
            .filter(|top| *top <= feet + 0.201)
            .max_by(f32::total_cmp)
    })
}

pub(crate) fn collides(point: Vec3, radius: f32, height: f32) -> bool {
    nearby(point, |logs| {
        logs.iter().any(|log| {
            if height == 0.0 {
                // Camera probes are spheres, not standing bodies.
                log.contacts(point, radius)
                    .any(|(bottom, top)| point.y > bottom - radius && point.y < top + radius)
            } else {
                log.blocks(point, radius, height)
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(seed: u64, species: Species) -> Log {
        let (length, radius, form) = dimensions(seed, species);
        Log {
            seed,
            species,
            length,
            radius,
            form,
            transform: Transform::default(),
        }
    }
    #[test]
    fn trunk_support_and_body_collision_agree_across_shapes_and_slopes() {
        let radius = crate::player::movement::PLAYER_RADIUS;
        for species in [Species::Orange, Species::Apple, Species::Spruce] {
            for seed in 0..12 {
                for tilt in [-0.4, 0.0, 0.4] {
                    let mut log = sample(seed, species);
                    log.transform = Transform::from_xyz(30.0, 5.0, -20.0)
                        .with_rotation(Quat::from_rotation_y(0.7) * Quat::from_rotation_z(tilt));
                    for i in 0..=40 {
                        let t = i as f32 / 40.0;
                        let axis = log
                            .transform
                            .transform_point(centerline(t, log.length, log.form));
                        let top = log.support(axis, radius).unwrap();
                        let feet = axis.with_y(top);
                        assert!(
                            !log.blocks(feet, radius, 1.25),
                            "standing feet must be clear"
                        );
                        assert!(
                            log.blocks(feet - Vec3::Y * 0.03, radius, 1.25),
                            "wood must be solid"
                        );
                        assert!(!log.blocks(feet + Vec3::Y, radius, 1.25));
                    }
                }
            }
        }
    }

    #[test]
    fn player_can_walk_across_a_placed_log() {
        use crate::player::movement::{
            PLAYER_EYE_HEIGHT, PLAYER_RADIUS, move_player_with_collisions, world_support_height,
        };
        let mut crossed = false;
        for log in (-90..90)
            .flat_map(|x| (-90..90).filter_map(move |z| placement(IVec2::new(x, z), false)))
        {
            let across = (log.transform.rotation * Vec3::Z).normalize();
            let center = log.transform.translation;
            let start = center - across * 1.0;
            let ground = crate::world::terrain::height(start.xz());
            let mut eye = start.with_y(ground + PLAYER_EYE_HEIGHT);
            let mut rose = false;
            for _ in 0..100 {
                eye = move_player_with_collisions(eye, across * 0.02);
                eye.y = world_support_height(eye, PLAYER_RADIUS) + PLAYER_EYE_HEIGHT;
                rose |= eye.y - PLAYER_EYE_HEIGHT > crate::world::terrain::height(eye.xz()) + 0.15;
            }
            if rose && (eye - start).dot(across) > 1.9 {
                crossed = true;
                break;
            }
        }
        assert!(
            crossed,
            "player must step onto, cross, and leave a fallen trunk"
        );
    }

    #[test]
    fn streaming_releases_meshes_and_collision_matches_logs() {
        let log = (-90..90)
            .flat_map(|x| (-90..90).map(move |z| IVec2::new(x, z)))
            .find_map(|cell| placement(cell, false))
            .unwrap();
        assert!(collides(log.transform.translation, 0.1, 0.0));
        assert!(!collides(
            log.transform.translation + Vec3::Y * 4.0,
            0.1,
            0.0
        ));
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(crate::rendering::view_distance::Range { load: 16.0 })
            .insert_resource(crate::player::camera::CameraRig {
                position: log.transform.translation,
                ..default()
            })
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Logs));
        for _ in 0..15 {
            app.update();
        }
        let old: Vec<_> = app
            .world()
            .resource::<Streaming>()
            .chunks
            .values()
            .flatten()
            .flat_map(|(_, handles)| handles.iter().map(|h| h.id()))
            .collect();
        assert!(!old.is_empty());
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position += Vec3::splat(1000.0);
        app.update();
        let meshes = app.world().resource::<Assets<Mesh>>();
        assert!(old.iter().all(|id| !meshes.contains(*id)));
        assert!(app.world().resource::<Streaming>().chunks.is_empty());
        assert_eq!(app.world().resource::<Streaming>().jobs.len(), 2);
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position = log.transform.translation;
        for _ in 0..15 {
            app.update();
        }
        let count: usize = app
            .world()
            .resource::<Streaming>()
            .chunks
            .values()
            .flatten()
            .map(|(_, handles)| handles.len())
            .sum();
        assert_eq!(count, old.len());
    }

    #[test]
    fn independent_sparse_density_and_rare_citrus_moss() {
        let mut logs = 0;
        let mut moss = 0;
        for seed in 0..1_000_000 {
            if selected(seed) {
                logs += 1;
                moss += usize::from(mossy(seed, Species::Orange));
            }
            assert!(!mossy(seed, Species::Apple));
            assert!(!mossy(seed, Species::Spruce));
        }
        assert!((19_400..20_600).contains(&logs), "{logs}");
        let ratio = moss as f32 / logs as f32;
        assert!((0.017..0.023).contains(&ratio), "{ratio}");
    }
    #[test]
    fn unique_repeatable_finite_geometry_and_both_forms_for_each_species() {
        for species in [Species::Orange, Species::Apple, Species::Spruce] {
            let mut forms = [false; 2];
            let mut counts = std::collections::HashSet::new();
            for seed in 0..32 {
                let log = sample(seed, species);
                forms[log.form] = true;
                let meshes = generate(&log);
                let repeat = generate(&log);
                counts.insert(meshes[0].positions.len());
                for (mesh, other) in meshes.iter().zip(&repeat) {
                    assert_eq!(mesh.positions, other.positions);
                    assert_eq!(mesh.indices, other.indices);
                    assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
                    assert!(mesh.normals.iter().flatten().all(|v| v.is_finite()));
                    assert!(
                        mesh.indices
                            .iter()
                            .all(|i| (*i as usize) < mesh.positions.len())
                    );
                }
                assert_eq!(!meshes[2].indices.is_empty(), mossy(seed, species));
            }
            assert!(forms.into_iter().all(|v| v));
            assert!(
                counts.len() > 4,
                "dead branch topology must vary across individual logs"
            );
        }
    }
    #[test]
    fn accepted_logs_follow_species_and_exclude_high_ground() {
        let mut trees = 0;
        let mut logs = 0;
        for x in -90..90 {
            for z in -90..90 {
                let cell = IVec2::new(x, z);
                if wild::log_source(cell).is_some() {
                    trees += 1;
                }
                if let Some(log) = placement(cell, false) {
                    logs += 1;
                    assert_eq!(log.species, wild::log_source(cell).unwrap().1);
                    for i in 0..=8 {
                        let p = log.transform.transform_point(centerline(
                            i as f32 / 8.0,
                            log.length,
                            log.form,
                        ));
                        assert!(wild::log_habitat(p.xz(), log.species));
                    }
                    assert!(log.transform.translation.y < 51.0);
                    assert_eq!(log.transform, placement(cell, false).unwrap().transform);
                }
                if let Some(log) = placement(cell, true) {
                    assert_eq!(log.species, Species::Spruce);
                    assert!(winter::log_source(cell).unwrap().y <= 50.0);
                }
            }
        }
        let ratio = logs as f32 / trees as f32;
        assert!((0.015..0.024).contains(&ratio), "{logs}/{trees} = {ratio}");
    }
}

#[cfg(test)]
mod visual_review {
    use super::*;
    /// Native material/mesh contact sheet; run explicitly under an X server.
    #[test]
    #[ignore]
    fn log_material_contact_sheet() {
        fn scene(
            mut commands: Commands,
            mut meshes: ResMut<Assets<Mesh>>,
            mut materials: ResMut<Assets<StandardMaterial>>,
            mut images: ResMut<Assets<Image>>,
        ) {
            commands.spawn((
                Camera3d::default(),
                Transform::from_xyz(6.6, 7.8, 10.8).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
            ));
            commands.spawn((
                DirectionalLight {
                    illuminance: 14000.0,
                    shadow_maps_enabled: true,
                    ..default()
                },
                Transform::from_xyz(3.0, 8.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Plane3d::default().mesh().size(30.0, 30.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.16, 0.18, 0.13),
                    perceptual_roughness: 1.0,
                    ..default()
                })),
            ));
            let bark = [
                materials.add(material(SurfaceKind::CitrusBark, &mut images)),
                materials.add(material(SurfaceKind::Bark, &mut images)),
            ];
            let detail = materials.add(StandardMaterial {
                perceptual_roughness: 0.98,
                ..default()
            });
            for (row, species) in [Species::Orange, Species::Apple, Species::Spruce]
                .into_iter()
                .enumerate()
            {
                for form in 0..2 {
                    let seed = (0..10000)
                        .find(|s| {
                            dimensions(*s, species).2 == form
                                && (row != 0 || form != 1 || mossy(*s, species))
                        })
                        .unwrap();
                    let (length, radius, form) = dimensions(seed, species);
                    let log = Log {
                        seed,
                        species,
                        length,
                        radius,
                        form,
                        transform: Transform::from_xyz(
                            (form as f32 - 0.5) * 4.0,
                            radius * 0.85,
                            (row as f32 - 1.0) * 2.0,
                        ),
                    };
                    for (part, geometry) in generate(&log).into_iter().enumerate() {
                        if geometry.indices.is_empty() {
                            continue;
                        }
                        commands.spawn((
                            Mesh3d(meshes.add(geometry.mesh())),
                            MeshMaterial3d(if part == 0 {
                                bark[usize::from(species != Species::Orange)].clone()
                            } else {
                                detail.clone()
                            }),
                            log.transform,
                            Visibility::Inherited,
                        ));
                    }
                }
            }
        }
        App::new()
            .add_plugins(
                DefaultPlugins
                    .set(bevy::winit::WinitPlugin {
                        run_on_any_thread: true,
                    })
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            title: "Fallen log material review".into(),
                            resolution: (1280, 900).into(),
                            ..default()
                        }),
                        ..default()
                    }),
            )
            .add_systems(Startup, scene)
            .run();
    }
}
