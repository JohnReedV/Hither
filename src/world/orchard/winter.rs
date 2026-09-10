//! Snow-laden spruce-like trees: tapered leaders, irregular branch whorls,
//! lateral shoots, needle sprays and snow resting on those same shoots.
use super::*;
use crate::world::streaming::{Coordinator, Layer};
#[cfg(test)]
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};

const CELL: f32 = 3.6;
const RADIUS: i32 = 14;
const VARIANTS: usize = 8;
const TREES_PER_FRAME: usize = 24;

fn hash(cell: IVec2) -> u64 {
    let mut x = crate::world::biome::world_seed()
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

fn placement(cell: IVec2) -> Option<Transform> {
    thread_local! {
        static CACHE: std::cell::RefCell<super::placement_cache::PlacementCache<Transform>> = Default::default();
    }
    CACHE.with_borrow_mut(|cache| {
        cache.get_or_insert(crate::world::biome::world_seed(), cell, || {
            placement_uncached(cell)
        })
    })
}

fn placement_uncached(cell: IVec2) -> Option<Transform> {
    let mut rng = Rng(hash(cell));
    let site = super::forest::site_with_alpine(
        crate::world::biome::world_seed() ^ 0x70696e65,
        cell,
        CELL,
        2.2,
        true,
    )?;
    let p = Vec3::new(site.position.x, 0.0, site.position.y);
    let mountain = crate::world::biome::mountain_amount(site.position) > 0.0;
    // Leave the castle and its gate clear in every biome.
    if (p.x.abs() < 10.0 && p.z.abs() < 10.0)
        || (!mountain && crate::world::biome::snow_amount(p.xz()) < 0.60)
        || crate::world::biome::forest_amount(p.xz()) < 0.5
        || crate::world::orcs::entrance_overlap(p, 2.0)
    {
        return None;
    }
    let scale = super::forest::age_scale(site.hash);
    let ground = crate::world::terrain::vegetation_anchor(site.position, 0.24 * scale, 0.8)?;
    if !crate::world::biome::mountain_snow_allowed(
        crate::world::biome::mountain_amount(site.position),
        ground,
    ) {
        return None;
    }
    let p = p.with_y(ground);
    Some(
        Transform::from_translation(p)
            .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
            .with_scale(Vec3::splat(scale)),
    )
}

fn nearby(p: Vec3) -> impl Iterator<Item = Transform> {
    let center = (p.xz() / CELL).floor().as_ivec2();
    (-2..=2).flat_map(move |x| (-2..=2).filter_map(move |z| placement(center + IVec2::new(x, z))))
}
pub(crate) fn player_collides(p: Vec3, radius: f32) -> bool {
    nearby(p).any(|tree| p.xz().distance(tree.translation.xz()) < radius + 0.23 * tree.scale.x)
}
pub(crate) fn camera_obstructed(p: Vec3, radius: f32) -> bool {
    nearby(p).any(|tree| {
        let local = (p - tree.translation) / tree.scale.x;
        let margin = radius / tree.scale.x;
        // Needles are not a solid cone. Keep the structural trunk collision.
        local.y < 7.5 && local.xz().length() < 0.28 + margin
    })
}

struct WinterTree {
    wood: Geometry,
    distant_wood: Geometry,
    needles: Geometry,
    snow: Geometry,
}

fn snow_cap(mesh: &mut Geometry, a: Vec3, b: Vec3, width: f32, phase: f32) {
    let side = (b - a).cross(Vec3::Y).normalize_or(Vec3::X);
    let first = mesh.positions.len() as u32;
    for row in 0..=4 {
        let t = row as f32 / 4.0;
        let envelope = (PI * t).sin().max(0.0).powf(0.45);
        for col in 0..=4 {
            let across = col as f32 * 0.5 - 1.0;
            let edge = 1.0 + 0.13 * (t * 18.0 + phase).sin();
            let p = a.lerp(b, t)
                + side * across * width * envelope * edge
                + Vec3::Y * (0.022 + (1.0 - across * across) * 0.065 * envelope);
            mesh.vertex(
                p,
                Vec2::new(t, across * 0.5 + 0.5),
                Vec3::new(0.84, 0.90, 0.96) * (0.94 + t * 0.06),
            );
            if row > 0 && col > 0 {
                let i = first + (row - 1) * 5 + col - 1;
                mesh.triangle(i, i + 1, i + 5);
                mesh.triangle(i + 1, i + 6, i + 5);
            }
        }
    }
}

pub(super) fn spray(mesh: &mut Geometry, a: Vec3, b: Vec3, rng: &mut Rng) {
    let axis = (b - a).normalize();
    let side = axis.cross(Vec3::Y).normalize_or(Vec3::X);
    // Radial needle pairs, not citrus leaves or crossed billboard sheets.
    for i in 0..18 {
        let t = i as f32 / 18.0;
        let root = a.lerp(b, t);
        for sign in [-1.0, 1.0] {
            let tip = root
                + (side * sign + axis * 0.35 + Vec3::Y * rng.range(-0.3, 0.35)).normalize()
                    * rng.range(0.055, 0.11);
            let color = Vec3::new(0.024, 0.064, 0.040) * rng.range(0.7, 1.35);
            let n = mesh.vertex(root - axis * 0.008, Vec2::ZERO, color * 0.7);
            mesh.vertex(root + Vec3::Y * 0.009, Vec2::new(0.5, 0.0), color);
            mesh.vertex(tip, Vec2::new(0.5, 1.0), color * 1.15);
            mesh.vertex(root + axis * 0.008, Vec2::X, color * 0.7);
            mesh.triangle(n, n + 1, n + 2);
            mesh.triangle(n + 1, n + 3, n + 2);
        }
    }
}

fn generate(seed: u64) -> WinterTree {
    generate_variation(seed, false)
}

fn generate_variation(seed: u64, mountain: bool) -> WinterTree {
    generate_variation_lod(seed, mountain, 0)
}
fn generate_variation_lod(seed: u64, mountain: bool, detail: usize) -> WinterTree {
    let branch = |mesh: &mut Geometry, path: &[(Vec3, f32)], sides: usize, seed: f32| {
        super::conifer_lod::branch(mesh, path, sides, seed, detail)
    };
    let mut rng = Rng(seed);
    let mut wood = Geometry::default();
    let mut needles = Geometry::default();
    let mut snow = Geometry::default();
    let lean = if mountain {
        Vec3::new(rng.range(0.45, 0.85), 0.0, rng.range(-0.18, 0.18))
    } else {
        Vec3::new(rng.range(-0.22, 0.22), 0.0, rng.range(-0.22, 0.22))
    };
    let height_scale = rng.range(0.80, 1.22) * if mountain { 0.72 } else { 1.0 };
    let width_scale = rng.range(0.80, 1.12) * if mountain { 0.80 } else { 1.0 };
    let levels = 9 + (rng.unit() * 5.0) as usize;
    let skirt = rng.range(0.72, 1.25);
    let sweep = rng.range(-0.30, 0.15);
    let snow_cover = if mountain {
        rng.range(0.30, 0.60)
    } else {
        rng.range(0.48, 0.90)
    };
    let crown_phase = rng.range(0.0, TAU);
    branch(
        &mut wood,
        &[
            (Vec3::ZERO, 0.21),
            (Vec3::Y * 2.0 + lean * 0.2, 0.13),
            (Vec3::Y * 4.0 + lean * 0.65, 0.06),
            (Vec3::Y * 5.9 + lean, 0.002),
        ],
        14,
        seed as f32,
    );
    let mut distant_wood = Geometry {
        positions: wood.positions.clone(),
        normals: wood.normals.clone(),
        uvs: wood.uvs.clone(),
        colors: wood.colors.clone(),
        indices: wood.indices.clone(),
    };
    for level in 0..levels {
        let h = skirt + level as f32 * (5.35 - skirt) / levels as f32 + rng.range(-0.07, 0.07);
        let phase = crown_phase + level as f32 * 2.399963 + rng.range(-0.45, 0.45);
        let count = 4 + (rng.unit() * 4.0) as usize;
        for i in 0..count {
            let angle = phase + i as f32 * TAU / count as f32 + rng.range(-0.20, 0.20);
            let d = Vec3::new(angle.cos(), 0.0, angle.sin());
            let across = Vec3::new(-d.z, 0.0, d.x);
            let length = (1.0 - h / 6.0)
                * rng.range(1.55, 2.15)
                * (1.0 + 0.13 * (angle * 2.0 + crown_phase).sin())
                * if mountain {
                    0.82 + 0.30 * angle.cos()
                } else {
                    1.0
                };
            let root = Vec3::Y * h + lean * (h / 5.9);
            let tip = root + d * length + Vec3::Y * (sweep + rng.range(-0.15, 0.14));
            let path = [
                (root, 0.045 * (1.0 - h / 6.5)),
                (root.lerp(tip, 0.55) - Vec3::Y * 0.13, 0.013),
                (tip + Vec3::Y * 0.08, 0.001),
            ];
            branch(&mut wood, &path, 6, angle);
            branch(&mut distant_wood, &path, 4, angle);
            let curve = curved_path(&path);
            let shoots = 5 + (rng.unit() * 4.0) as usize;
            for shoot in 0..shoots {
                let t = 0.18 + shoot as f32 / shoots as f32 * 0.74;
                let base = path_point(&curve, t).0;
                for sign in [-1.0, 1.0] {
                    let spread = length * (1.0 - t) * rng.range(0.33, 0.52) + 0.06;
                    let end = base + (across * sign + d * 0.48) * spread
                        - Vec3::Y * rng.range(0.06, 0.18);
                    branch(&mut wood, &[(base, 0.004), (end, 0.0005)], 4, angle);
                    spray(&mut needles, base, end, &mut rng);
                    if rng.unit()
                        < snow_cover
                            * if mountain {
                                0.8 + 0.4 * angle.cos()
                            } else {
                                1.0
                            }
                    {
                        snow_cap(
                            &mut snow,
                            base.lerp(end, 0.07),
                            base.lerp(end, 0.93),
                            0.06 + spread * 0.07,
                            angle + shoot as f32,
                        );
                    }
                }
            }
            spray(&mut needles, path_point(&curve, 0.72).0, tip, &mut rng);
        }
    }
    spray(
        &mut needles,
        Vec3::Y * 5.45 + lean,
        Vec3::Y * 5.95 + lean,
        &mut rng,
    );
    for geometry in [&mut wood, &mut distant_wood, &mut needles, &mut snow] {
        for p in &mut geometry.positions {
            p[0] *= width_scale;
            p[1] *= height_scale;
            p[2] *= width_scale;
        }
    }
    if mountain {
        // Cooler, waxy alpine needles remain individually modeled.
        for color in &mut needles.colors {
            color[0] *= 1.15;
            color[2] *= 1.45;
        }
    }
    wood.finish_normals();
    distant_wood.finish_normals();
    needles.finish_normals();
    snow.finish_normals();
    WinterTree {
        wood,
        distant_wood,
        needles,
        snow,
    }
}

// Distant models keep every shoot and snow patch, simplifying their surfaces
// instead of dropping whole branches and punching holes in the canopy.
pub(super) fn distant_needles(source: &Geometry) -> Geometry {
    distant_needles_lod(source, 3)
}
pub(super) fn distant_needles_lod(source: &Geometry, stride: usize) -> Geometry {
    let mut mesh = Geometry::default();
    for start in (0..source.positions.len()).step_by(144) {
        // Keep real needle silhouettes and gaps, not giant crossed diamonds.
        // A third of the needle pairs, each reduced to a single triangle with
        // a slightly wider base, retain the feathery outline at 1/6 the cost.
        for pair in (0..18).step_by(stride) {
            for sign in 0..2 {
                let i = start + pair * 8 + sign * 4;
                let a = Vec3::from(source.positions[i]);
                let b = Vec3::from(source.positions[i + 3]);
                let root = (a + b) * 0.5;
                let color = Vec3::from_slice(&source.colors[i + 1][..3]);
                let n = mesh.vertex(
                    root + (a - root) * (1.8 * (stride as f32 / 3.0)),
                    Vec2::ZERO,
                    color * 0.7,
                );
                mesh.vertex(
                    root + (b - root) * (1.8 * (stride as f32 / 3.0)),
                    Vec2::X,
                    color,
                );
                mesh.vertex(
                    Vec3::from(source.positions[i + 2]),
                    Vec2::new(0.5, 1.0),
                    color * 1.15,
                );
                // Reuse the authored blade shading instead of introducing a
                // different normal/color distribution at the distance boundary.
                for (out, original) in [
                    (n as usize, i),
                    (n as usize + 1, i + 3),
                    (n as usize + 2, i + 2),
                ] {
                    mesh.normals[out] = source.normals[original];
                    mesh.colors[out] = source.colors[original];
                }
                mesh.triangle(n, n + 1, n + 2);
            }
        }
    }
    mesh
}

fn distant_snow(source: &Geometry) -> Geometry {
    distant_snow_lod(source, 1)
}
fn distant_snow_lod(source: &Geometry, stride: usize) -> Geometry {
    let mut mesh = Geometry::default();
    let rows: Vec<_> = (0..=4).step_by(stride).collect();
    for start in (0..source.positions.len()).step_by(25) {
        let base = mesh.positions.len() as u32;
        for (row, &r) in rows.iter().enumerate() {
            for (column, c) in [0, 2, 4].into_iter().enumerate() {
                let i = start + r * 5 + c;
                mesh.positions.push(source.positions[i]);
                mesh.normals.push(source.normals[i]);
                mesh.colors.push(source.colors[i]);
                mesh.uvs.push(source.uvs[i]);
                if row > 0 && column > 0 {
                    let a = base + ((row - 1) * 3 + column - 1) as u32;
                    mesh.triangle(a, a + 1, a + 3);
                    mesh.triangle(a + 1, a + 4, a + 3);
                }
            }
        }
    }
    mesh
}

#[derive(Resource)]
pub(super) struct WinterAssets {
    shadows: Vec<Handle<Mesh>>,
    models: Vec<[Handle<Mesh>; 3]>,
    distant: Vec<[Handle<Mesh>; 3]>,
    materials: [Handle<StandardMaterial>; 3],
    far: Vec<[[Handle<Mesh>; 3]; 2]>,
    extents: Vec<f32>,
}
#[derive(Component)]
pub(super) struct WinterPart {
    variant: usize,
    part: usize,
    position: Vec3,
    scale: f32,
    near: bool,
}
type Placements = Vec<(IVec2, Option<Transform>)>;

#[derive(Resource, Default)]
pub(super) struct Streaming {
    cells: HashMap<IVec2, Entity>,
    center: Option<IVec2>,
    lod_position: Option<Vec3>,
    pending: VecDeque<(IVec2, Transform)>,
    evaluated: std::collections::HashSet<IVec2>,
    discovery: crate::world::streaming::WindowScan,
    placement_task: Option<crate::world::streaming::BuildTask<Placements>>,
}

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let started = std::time::Instant::now();
    let trees: Vec<_> = std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..VARIANTS * 2)
            .map(|i| {
                scope.spawn(move || {
                    let seed = hash(IVec2::new((i % VARIANTS) as i32, -917));
                    if i < VARIANTS {
                        generate(seed)
                    } else {
                        generate_variation(seed, true)
                    }
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| job.join().expect("conifer generation"))
            .collect()
    });
    let shadows = trees
        .iter()
        .map(|tree| {
            let mut shadow = tree.distant_wood.clone();
            super::lod::append(
                &mut shadow,
                &distant_needles(&tree.needles),
                Transform::default(),
            );
            // Each snow-covered spray casts a four-triangle silhouette rather
            // than submitting its sculpted surface to both cascades.
            let mut snow = Geometry::default();
            for start in (0..tree.snow.positions.len()).step_by(25) {
                let base = snow.positions.len() as u32;
                for offset in [0, 4, 12, 20, 24] {
                    snow.vertex(
                        Vec3::from(tree.snow.positions[start + offset]),
                        Vec2::ZERO,
                        Vec3::ONE,
                    );
                }
                for face in [[0, 1, 2], [0, 2, 3], [1, 4, 2], [2, 4, 3]] {
                    snow.triangle(base + face[0], base + face[1], base + face[2]);
                }
            }
            snow.finish_normals();
            super::lod::append(&mut shadow, &snow, Transform::default());
            meshes.add(shadow.mesh())
        })
        .collect();
    let extents = trees
        .iter()
        .map(|t| super::conifer_lod::extent([&t.wood, &t.needles, &t.snow]))
        .collect();
    let far = (0..VARIANTS * 2)
        .map(|i| {
            std::array::from_fn(|level| {
                let seed = hash(IVec2::new((i % VARIANTS) as i32, -917));
                let t = generate_variation_lod(seed, i >= VARIANTS, level + 1);
                [
                    meshes.add(t.distant_wood.mesh()),
                    meshes.add(
                        distant_needles_lod(&t.needles, if level == 0 { 6 } else { 9 }).mesh(),
                    ),
                    meshes.add(distant_snow_lod(&t.snow, 2).mesh()),
                ]
            })
        })
        .collect();
    let (models, distant): (Vec<_>, Vec<_>) = trees
        .into_iter()
        .map(|tree| {
            let distant = [
                meshes.add(tree.distant_wood.mesh()),
                meshes.add(distant_needles(&tree.needles).mesh()),
                meshes.add(distant_snow(&tree.snow).mesh()),
            ];
            let near = [
                meshes.add(tree.wood.mesh()),
                meshes.add(tree.needles.mesh()),
                meshes.add(tree.snow.mesh()),
            ];
            (near, distant)
        })
        .unzip();
    let bark = materials.add(material(SurfaceKind::Bark, &mut images));
    let needles = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.92,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let snow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.96, 1.0),
        perceptual_roughness: 0.90,
        reflectance: 0.2,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(WinterAssets {
        shadows,
        models,
        distant,
        materials: [bark, needles, snow],
        far,
        extents,
    });
    commands.init_resource::<Streaming>();
    info!("Winter tree assets ready in {:?}", started.elapsed());
}

#[cfg(test)]
use crate::world::streaming::entering_cells;

#[allow(clippy::too_many_arguments)]
pub(super) fn stream(
    mut coordinator: ResMut<Coordinator>,
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    range: Option<Res<crate::rendering::view_distance::Range>>,
    mut previous_radius: Local<i32>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    view: Option<Res<crate::player::camera::CameraView>>,
    assets: Res<WinterAssets>,
    mut state: ResMut<Streaming>,
    mut parts: Query<(&mut WinterPart, &mut Mesh3d)>,
    children: Query<&Children>,
) {
    let camera_position = view.as_ref().map_or(rig.position, |v| v.position);
    let radius = range
        .as_ref()
        .map_or(RADIUS, |r| ((r.load + 12.0) / CELL).ceil() as i32 + 1);
    let center = (rig.position.xz() / CELL).floor().as_ivec2();
    if detail.as_ref().is_some_and(|d| d.is_changed())
        || state
            .lod_position
            .is_none_or(|p| p.distance_squared(camera_position) > 0.25)
    {
        let previous = state
            .lod_position
            .replace(camera_position)
            .unwrap_or(camera_position);
        let radius = ((detail.as_ref().map_or(12.0, |d| d.0) + 20.0) / CELL).ceil() as i32;
        let current = (camera_position.xz() / CELL).floor().as_ivec2();
        let old = (previous.xz() / CELL).floor().as_ivec2();
        let roots: Vec<_> = if detail.as_ref().is_some_and(|d| d.is_changed()) {
            state.cells.values().copied().collect()
        } else {
            crate::world::streaming::entering_cells(current, radius, None)
                .chain(crate::world::streaming::entering_cells(
                    old,
                    radius,
                    Some((current, radius)),
                ))
                .filter_map(|cell| state.cells.get(&cell).copied())
                .collect()
        };
        for root in roots {
            let Ok(children) = children.get(root) else {
                continue;
            };
            for child in children {
                let Ok((mut part, mut mesh)) = parts.get_mut(*child) else {
                    continue;
                };
                let near = super::lod::near_detail(
                    part.near,
                    part.position.distance(camera_position),
                    part.scale,
                    detail.as_deref(),
                );
                if near != part.near {
                    part.near = near;
                    let bank = if near {
                        &assets.models
                    } else {
                        &assets.distant
                    };
                    mesh.0 = bank[part.variant][part.part].clone();
                }
            }
        }
    }
    if state.center != Some(center) || *previous_radius != radius {
        *previous_radius = radius;
        state.center = Some(center);
        let inside = |cell: IVec2| (cell - center).abs().max_element() <= radius;
        state.cells.retain(|cell, entity| {
            if inside(*cell) {
                true
            } else {
                crate::world::streaming::retire(&mut commands, *entity);
                false
            }
        });
        state.pending.retain(|(cell, _)| inside(*cell));
        state.evaluated.retain(|cell| inside(*cell));
        state.discovery.update(center, radius);
    }
    if let Some(task) = state.placement_task.as_mut()
        && task.is_ready()
    {
        let results = task.take();
        state.placement_task = None;
        for (cell, transform) in results {
            if (cell - center).abs().max_element() <= radius {
                // Includes rejected sites, so movement never reevaluates the forest.
                state.evaluated.insert(cell);
                if let Some(transform) = transform
                    && !state.cells.contains_key(&cell)
                    && !state.pending.iter().any(|(c, _)| *c == cell)
                {
                    state.pending.push_back((cell, transform));
                }
            }
        }
    }
    if state.placement_task.is_none()
        && !state.discovery.is_empty()
        && let Some(permit) = coordinator.worker(Layer::Conifers)
        && let Some(_discovery) = coordinator.install(Layer::Conifers, 1, 0)
    {
        let mut cells = Vec::new();
        for _ in 0..128 {
            let Some(cell) = state.discovery.next() else {
                break;
            };
            if (cell - center).abs().max_element() <= radius && !state.evaluated.contains(&cell) {
                cells.push(cell);
            }
            if cells.len() == 64 {
                break;
            }
        }
        if !cells.is_empty() {
            state.placement_task =
                Some(crate::world::streaming::BuildTask::new(permit, move || {
                    cells
                        .into_iter()
                        .map(|cell| (cell, placement(cell)))
                        .collect()
                }));
        }
    }
    if coordinator.view_changed() {
        state
            .pending
            .make_contiguous()
            .sort_unstable_by_key(|(_, t)| coordinator.visual_rank(t.translation, 12.0));
    }
    for _ in 0..TREES_PER_FRAME {
        if state.pending.is_empty() {
            break;
        }
        let Some(_installation) = coordinator.install_entities(Layer::Conifers, 1, 0, 5) else {
            break;
        };
        let Some((cell, transform)) = state.pending.pop_front() else {
            break;
        };
        let entity = {
            let mountain = crate::world::biome::mountain_amount(transform.translation.xz()) > 0.0;
            let variant = hash(cell) as usize % VARIANTS + if mountain { VARIANTS } else { 0 };
            let position = transform.translation + Vec3::Y * 3.0 * transform.scale.x;
            let near = super::lod::near_detail(
                false,
                position.distance(camera_position),
                transform.scale.x,
                detail.as_deref(),
            );
            let bank = if near {
                &assets.models
            } else {
                &assets.distant
            };
            let model = &bank[variant];
            commands
                .spawn((
                    Name::new(if mountain {
                        "Wind-shaped mountain spruce"
                    } else {
                        "Snow-laden spruce"
                    }),
                    transform,
                    Visibility::Inherited,
                ))
                .with_children(|tree| {
                    tree.spawn((
                        Name::new("Conifer shadow canopy"),
                        Mesh3d(assets.shadows[variant].clone()),
                        MeshMaterial3d(assets.materials[1].clone()),
                        super::lod::shadow_layers(),
                        crate::rendering::plant_lod::ShadowProxy,
                        Transform::default(),
                    ));
                    for (part, (mesh, material)) in model.iter().zip(&assets.materials).enumerate()
                    {
                        tree.spawn((
                            crate::rendering::plant_lod::Canopy(super::conifer_lod::enabled()),
                            crate::rendering::plant_lod::AuthoredLods {
                                levels: std::array::from_fn(|level| {
                                    assets.far[variant][level][part].clone()
                                }),
                                extent: assets.extents[variant],
                            },
                            WinterPart {
                                variant,
                                part,
                                position,
                                scale: transform.scale.x,
                                near,
                            },
                            Mesh3d(mesh.clone()),
                            bevy::light::NotShadowCaster,
                            MeshMaterial3d(material.clone()),
                            Transform::default(),
                        ));
                    }
                })
                .id()
        };
        state.cells.insert(cell, entity);
    }
}

/// Only lowland snow forests contribute logs; mountain trees are excluded.
pub(super) fn log_source(cell: IVec2) -> Option<Vec3> {
    let p = placement(cell)?.translation;
    (p.y <= 50.0 && crate::world::biome::mountain_amount(p.xz()) == 0.0).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mountain_spruce_is_a_shorter_distinct_snowy_variation() {
        let low = generate(721);
        let high = generate_variation(721, true);
        let repeat = generate_variation(721, true);
        let height = |g: &Geometry| g.positions.iter().map(|p| p[1]).fold(0.0_f32, f32::max);
        assert!(height(&high.wood) < height(&low.wood) * 0.85);
        assert_ne!(high.wood.positions, low.wood.positions);
        assert_eq!(high.wood.positions, repeat.wood.positions);
        assert!(!high.snow.indices.is_empty());
        for mesh in [&high.wood, &high.needles, &high.snow, &high.distant_wood] {
            assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(
                mesh.indices
                    .iter()
                    .all(|i| (*i as usize) < mesh.positions.len())
            );
        }
        let mut found = false;
        for x in -150..150 {
            for z in -150..150 {
                if let Some(t) = placement(IVec2::new(x * 5, z * 5))
                    && crate::world::biome::mountain_amount(t.translation.xz()) > 0.0
                {
                    assert!(t.translation.y > 200.0);
                    found = true;
                }
            }
        }
        assert!(found, "mountain spruce must have viable alpine sites");
    }

    #[test]
    fn mountain_snow_trees_never_spawn_below_cutoff() {
        let mut low_mountain_cells = 0;
        for x in -100..100 {
            for z in -100..100 {
                let cell = IVec2::new(x * 5, z * 5);
                if let Some(tree) = placement(cell) {
                    assert!(crate::world::biome::mountain_snow_allowed(
                        crate::world::biome::mountain_amount(tree.translation.xz()),
                        tree.translation.y,
                    ));
                }
                let Some(site) = super::super::forest::site(
                    crate::world::biome::world_seed() ^ 0x70696e65,
                    cell,
                    CELL,
                    2.2,
                ) else {
                    continue;
                };
                if crate::world::biome::mountain_amount(site.position) > 0.0
                    && crate::world::terrain::height(site.position) <= 200.0
                {
                    low_mountain_cells += 1;
                    assert!(placement(cell).is_none());
                }
            }
        }
        assert!(low_mountain_cells > 0);
    }

    #[test]
    fn incremental_windows_match_full_placement_across_moves_resizes_and_teleports() {
        let mut previous = None;
        let mut accepted = HashMap::new();
        for (center, radius) in [
            (IVec2::new(30, 35), 6),
            (IVec2::new(31, 35), 6),
            (IVec2::new(31, 34), 6),
            (IVec2::new(32, 33), 6),
            (IVec2::new(32, 33), 9),
            (IVec2::new(32, 33), 4),
            (IVec2::new(-50, -30), 7),
            (IVec2::new(30, 35), 6),
        ] {
            accepted.retain(|cell: &IVec2, _| (*cell - center).abs().max_element() <= radius);
            for cell in entering_cells(center, radius, previous) {
                if let Some(t) = placement(cell) {
                    assert!(accepted.insert(cell, t).is_none());
                }
            }
            let full: HashMap<_, _> = entering_cells(center, radius, None)
                .filter_map(|cell| placement(cell).map(|t| (cell, t)))
                .collect();
            assert_eq!(accepted, full);
            previous = Some((center, radius));
        }
        // At a 260m view distance, a one-cell move evaluates one edge, rather
        // than all 24,649 sites. Radius reductions need no new placement work.
        assert_eq!(
            entering_cells(IVec2::X, 78, Some((IVec2::ZERO, 78))).count(),
            157
        );
        assert_eq!(
            entering_cells(IVec2::ZERO, 40, Some((IVec2::ZERO, 78))).count(),
            0
        );
    }

    #[test]
    fn streaming_reuses_assets_and_revisits_exactly() {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(crate::player::camera::CameraRig::default())
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Conifers));
        app.update();
        assert!(app.world().resource::<Streaming>().cells.len() <= TREES_PER_FRAME);
        for _ in 0..40 {
            app.update();
        }
        let original: HashSet<_> = app
            .world()
            .resource::<Streaming>()
            .cells
            .keys()
            .copied()
            .collect();
        let meshes = app.world().resource::<Assets<Mesh>>().len();
        assert_eq!(meshes, VARIANTS * 2 * (4 * 3 + 1));
        for p in [
            Vec3::new(700.0, 1.25, -500.0),
            Vec3::new(-300.0, 1.25, 800.0),
            crate::player::camera::CameraRig::default().position,
        ] {
            let previous: Vec<_> = app
                .world()
                .resource::<Streaming>()
                .cells
                .values()
                .copied()
                .collect();
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = p;
            app.update();
            assert!(app.world().resource::<Streaming>().cells.len() <= TREES_PER_FRAME);
            for _ in 0..40 {
                app.update();
            }
            assert!(
                app.world().resource::<Streaming>().cells.len()
                    <= ((RADIUS * 2 + 1).pow(2)) as usize
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), meshes);
            let world = app.world_mut();
            let tree_count = world.resource::<Streaming>().cells.len();
            assert_eq!(
                world
                    .query_filtered::<&WinterPart, With<bevy::light::NotShadowCaster>>()
                    .iter(world)
                    .count(),
                tree_count * 3
            );
            assert_eq!(
                world
                    .query::<(&Name, &bevy::camera::visibility::RenderLayers)>()
                    .iter(world)
                    .filter(|(name, layers)| name.as_str() == "Conifer shadow canopy"
                        && *layers == &super::super::lod::shadow_layers())
                    .count(),
                tree_count
            );
            assert!(previous.iter().all(|e| app.world().get_entity(*e).is_err()));
            // One root, three visible children and one shadow-only child.
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<Transform>>()
                    .iter(app.world())
                    .count(),
                app.world().resource::<Streaming>().cells.len() * 5
            );
        }
        let revisited: HashSet<_> = app
            .world()
            .resource::<Streaming>()
            .cells
            .keys()
            .copied()
            .collect();
        assert_eq!(original, revisited);
    }

    #[test]
    fn snow_caps_face_up_and_castle_stays_clear() {
        let mut snow = Geometry::default();
        snow_cap(&mut snow, Vec3::ZERO, Vec3::Z, 0.1, 0.0);
        snow.finish_normals();
        assert!(snow.normals.iter().filter(|n| n[1] > 0.0).count() > 15);
        for x in -1..=0 {
            for z in -1..=0 {
                if let Some(t) = placement(IVec2::new(x, z)) {
                    assert!(t.translation.x.abs() >= 8.0 || t.translation.z.abs() >= 8.0);
                }
            }
        }
    }

    #[test]
    fn distant_needles_preserve_authored_shading() {
        let source = generate(519).needles;
        let far = distant_needles(&source);
        let mut out = 0;
        for shoot in (0..source.positions.len()).step_by(144) {
            for pair in (0..18).step_by(3) {
                for side in 0..2 {
                    for offset in [0, 3, 2] {
                        let source_index = shoot + pair * 8 + side * 4 + offset;
                        assert_eq!(far.normals[out], source.normals[source_index]);
                        assert_eq!(far.colors[out], source.colors[source_index]);
                        out += 1;
                    }
                }
            }
        }
    }
    #[test]
    fn winter_trees_have_finite_geometry_and_different_shapes() {
        let a = generate(519);
        let b = generate(876);
        assert_ne!(a.wood.positions, b.wood.positions);
        for tree in [a, b] {
            let far_needles = distant_needles(&tree.needles);
            let far_snow = distant_snow(&tree.snow);
            assert_eq!(far_needles.indices.len() * 6, tree.needles.indices.len());
            assert_eq!(far_snow.indices.len() * 2, tree.snow.indices.len());
            assert!(tree.distant_wood.indices.len() < tree.wood.indices.len() / 2);
            for geometry in [
                &tree.wood,
                &tree.needles,
                &tree.snow,
                &tree.distant_wood,
                &far_needles,
                &far_snow,
            ] {
                assert!(!geometry.indices.is_empty());
                assert!(
                    geometry
                        .positions
                        .iter()
                        .all(|p| Vec3::from(*p).is_finite())
                );
                assert!(geometry.normals.iter().all(|n| Vec3::from(*n).is_finite()));
                assert!(
                    geometry
                        .indices
                        .iter()
                        .all(|i| (*i as usize) < geometry.positions.len())
                );
            }
            assert!(tree.needles.positions.len() > 10000);
            assert!(tree.snow.positions.iter().all(|p| p[1] > 0.1));
        }
    }
    #[test]
    fn boreal_interiors_are_close_growing_stands_not_isolated_scatter() {
        let (mut sites, mut trees, mut close_neighbors) = (0, 0, 0);
        for x in -100..100 {
            for z in -100..100 {
                let cell = IVec2::new(x, z);
                let center = (cell.as_vec2() + Vec2::splat(0.5)) * CELL;
                // Mountain transitions now intentionally exclude snowy trees
                // below 200; this test measures unchanged lowland boreal density.
                if crate::world::biome::mountain_amount(center) > 0.0
                    || crate::world::biome::snow_amount(center) < 0.99
                    || crate::world::biome::forest_amount(center) < 0.99
                    || crate::world::biome::stand_density(center) < 0.8
                {
                    continue;
                }
                sites += 1;
                if let Some(tree) = placement(cell) {
                    trees += 1;
                    let mut has_neighbor = false;
                    for dx in -1..=1 {
                        for dz in -1..=1 {
                            if dx == 0 && dz == 0 {
                                continue;
                            }
                            if let Some(other) = placement(cell + IVec2::new(dx, dz)) {
                                let distance = tree.translation.distance(other.translation);
                                assert!(distance >= 2.2);
                                has_neighbor |= distance < 5.5;
                            }
                        }
                    }
                    close_neighbors += usize::from(has_neighbor);
                }
            }
        }
        assert!(sites > 100);
        assert!(
            trees > sites / 2,
            "stands should not be sparse: {trees}/{sites}"
        );
        assert!(
            close_neighbors > trees * 8 / 10,
            "most trees need close neighbors"
        );
    }
    #[test]
    fn placement_is_repeatable_and_snow_only_with_trunk_collision() {
        let mut count = 0;
        for x in -40..40 {
            for z in -40..60 {
                let cell = IVec2::new(x, z);
                let p = placement(cell);
                assert_eq!(p, placement(cell));
                if let Some(tree) = p {
                    count += 1;
                    assert!((tree.rotation * Vec3::Y - Vec3::Y).length() < 0.00001);
                    let root = tree.translation;
                    for i in 0..16 {
                        let a = i as f32 * TAU / 16.0;
                        let foot = root.xz() + Vec2::new(a.cos(), a.sin()) * 0.21 * tree.scale.x;
                        assert!(root.y <= crate::world::terrain::height(foot) + 0.001);
                    }
                    assert!(
                        crate::world::biome::mountain_amount(tree.translation.xz()) > 0.0
                            || crate::world::biome::snow_amount(tree.translation.xz()) >= 0.60
                    );
                    assert!(crate::world::biome::forest_amount(tree.translation.xz()) >= 0.5);
                    assert!(player_collides(tree.translation + Vec3::Y, 0.28));
                    assert!(camera_obstructed(tree.translation + Vec3::Y, 0.1));
                }
            }
        }
        assert!(count > 50);
    }
}
