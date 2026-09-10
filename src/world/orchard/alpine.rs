//! High-elevation bristlecones: spiral deadwood, wind-flagged living limbs,
//! dense terminal needle brushes and individually layered purple-brown cones.
use super::*;
use crate::world::streaming::{Coordinator, Layer};
use std::collections::HashMap;
const CELL: f32 = 9.0;
const RADIUS: i32 = 7;
const VARIANTS: usize = 8;
const TREES_PER_FRAME: usize = 12;
fn hash(cell: IVec2) -> u64 {
    let mut h = crate::world::biome::world_seed()
        ^ 0x62726973746c65
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}
fn altitude_allowed(y: f32) -> bool {
    y.is_finite() && y > 150.0
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
    // No snow, forest or mountain-habitat mask: all high terrain is eligible.
    // Jitter stays within the cell, preserving at least 4.5m between trunks.
    let p = (cell.as_vec2() + Vec2::new(rng.range(0.25, 0.75), rng.range(0.25, 0.75))) * CELL;
    if !altitude_allowed(crate::world::terrain::height(p)) {
        return None;
    }
    let scale = rng.range(0.8, 1.35);
    let ground = crate::world::terrain::vegetation_anchor(p, 0.40 * scale, 1.2)?;
    if !altitude_allowed(ground) || crate::world::orcs::entrance_overlap(p.extend(0.0).xzy(), 2.5) {
        return None;
    }
    Some(
        Transform::from_xyz(p.x, ground, p.y)
            .with_rotation(Quat::from_rotation_y(rng.range(-0.35, 0.35)))
            .with_scale(Vec3::splat(scale)),
    )
}
fn nearby(p: Vec3) -> impl Iterator<Item = Transform> {
    let cell = (p.xz() / CELL).floor().as_ivec2();
    (-1..=1).flat_map(move |x| (-1..=1).filter_map(move |z| placement(cell + IVec2::new(x, z))))
}
pub(crate) fn player_collides(p: Vec3, radius: f32) -> bool {
    nearby(p).any(|t| p.xz().distance(t.translation.xz()) < radius + 0.42 * t.scale.x)
}
pub(crate) fn camera_obstructed(p: Vec3, radius: f32) -> bool {
    nearby(p).any(|t| {
        let local = t.rotation.inverse() * (p - t.translation) / t.scale.x;
        (0.0..5.0).contains(&local.y)
            && local.xz().distance(center(local.y).xz()) < 0.36 + radius / t.scale.x
    })
}
fn center(y: f32) -> Vec3 {
    Vec3::new(0.10 * y + 0.18 * (y * 1.7).sin(), y, 0.14 * (y * 1.4).sin())
}
fn generate(seed: u64) -> [Geometry; 3] {
    let mut rng = Rng(seed);
    let mut wood = Geometry::default();
    let mut needles = Geometry::default();
    let mut silver = Geometry::default();
    let height = rng.range(3.8, 5.0);
    let trunk: Vec<_> = (0..=12)
        .map(|i| {
            let t = i as f32 / 12.0;
            (center(t * height), 0.34 * (1.0 - t).powf(0.8) + 0.015)
        })
        .collect();
    branch(&mut silver, &trunk, 18, rng.range(0.0, 20.0));
    // A living cinnamon strip spirals around the bleached, fluted heartwood.
    let ribbon: Vec<_> = (0..=18)
        .map(|i| {
            let t = i as f32 / 18.0;
            let a = t * 6.0 + seed as f32;
            (
                center(t * height) + Vec3::new(a.cos(), 0.0, a.sin()) * (0.28 * (1.0 - t) + 0.018),
                0.065 * (1.0 - t) + 0.009,
            )
        })
        .collect();
    branch(&mut wood, &ribbon, 8, 2.0);
    for i in 0..7 {
        let a = i as f32 * TAU / 7.0 + rng.range(-0.2, 0.2);
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        branch(
            &mut silver,
            &[
                (center(0.18), 0.15),
                (d * 0.5, 0.09),
                (d * rng.range(0.8, 1.2) - Vec3::Y * 0.18, 0.012),
            ],
            9,
            a,
        );
    }
    for i in 0..15 {
        let y = 0.9 + i as f32 * (height - 1.2) / 15.0;
        let root = center(y);
        let a = rng.range(-1.15, 1.15);
        let d = Vec3::new(a.cos(), rng.range(0.10, 0.40), a.sin());
        let length = rng.range(1.0, 2.2) * (1.0 - 0.35 * y / height);
        let elbow = root + d * length * 0.55 - Vec3::Y * 0.20;
        let end = root + d * length + Vec3::Y * 0.25;
        let path = [
            (root, 0.10 * (1.0 - y / height) + 0.035),
            (elbow, 0.045),
            (end, 0.009),
        ];
        branch(&mut wood, &path, 9, a);
        if i % 4 == 0 {
            // Bare upwind snags give the crown its weather-beaten silhouette.
            branch(
                &mut silver,
                &[
                    (root, 0.065),
                    (root - Vec3::X * 0.7 + Vec3::Y * 0.2, 0.03),
                    (root - Vec3::X * 1.1 + Vec3::Y * 0.65, 0.002),
                ],
                8,
                a,
            );
        }
        for j in 0..7 {
            let t = 0.38 + j as f32 * 0.085;
            let (base, _) = path_point(&curved_path(&path), t);
            let fan = Vec3::new(0.35, 0.38, (j as f32 - 3.0) * 0.19);
            let tip = base + fan;
            branch(&mut wood, &[(base, 0.013), (tip, 0.003)], 5, a);
            // Radial bottlebrush tufts, with cool blue-green sunlit tips.
            for k in 0..5 {
                let angle = k as f32 * TAU / 5.0;
                let axis = fan.normalize();
                let side = axis.any_orthonormal_vector();
                let radial = side * angle.cos() + axis.cross(side) * angle.sin();
                winter::spray(
                    &mut needles,
                    tip - axis * 0.22,
                    tip + axis * 0.08 + radial * 0.075,
                    &mut rng,
                );
            }
            if j == 5 && i % 2 == 0 {
                cone(&mut wood, tip - Vec3::Y * 0.10, a);
            }
        }
    }
    for c in &mut needles.colors {
        c[0] *= 1.6;
        c[1] *= 1.5;
        c[2] *= 2.0;
    }
    for mesh in [&mut wood, &mut needles, &mut silver] {
        mesh.finish_normals();
    }
    [wood, needles, silver]
}
fn cone(mesh: &mut Geometry, base: Vec3, phase: f32) {
    for row in 0..7 {
        let t = row as f32 / 7.0;
        let radius = 0.065 * (PI * (t * 0.85 + 0.08)).sin();
        for i in 0..8 {
            let a = phase + i as f32 * TAU / 8.0 + row as f32 * 0.4;
            let d = Vec3::new(a.cos(), 0.0, a.sin());
            let side = Vec3::new(-d.z, 0.0, d.x);
            let p = base + Vec3::Y * t * 0.22 + d * radius;
            let color = Vec3::new(0.24, 0.12, 0.16);
            let n = mesh.vertex(p - side * 0.023, Vec2::ZERO, color);
            mesh.vertex(p + side * 0.023, Vec2::X, color);
            mesh.vertex(p + d * 0.018 + Vec3::Y * 0.048, Vec2::Y, color * 1.5);
            mesh.triangle(n, n + 1, n + 2);
        }
    }
}
#[derive(Resource)]
pub(super) struct AlpineAssets {
    shadows: Vec<Handle<Mesh>>,
    models: Vec<[Handle<Mesh>; 3]>,
    distant: Vec<[Handle<Mesh>; 3]>,
    materials: [Handle<StandardMaterial>; 3],
}
#[derive(Component)]
pub(super) struct AlpinePart {
    variant: usize,
    part: usize,
    position: Vec3,
    scale: f32,
    near: bool,
}
#[derive(Resource, Default)]
pub(super) struct Streaming {
    cells: HashMap<IVec2, Entity>,
    center: Option<IVec2>,
    lod_position: Option<Vec3>,
    discovery: crate::world::streaming::Discovery<Option<Transform>>,
}
pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut models = Vec::new();
    let mut distant = Vec::new();
    let mut shadows = Vec::new();
    for i in 0..VARIANTS {
        let parts = generate(hash(IVec2::new(i as i32, -711)));
        let far_needles = winter::distant_needles(&parts[1]);
        let mut shadow = parts[0].clone();
        super::lod::append(&mut shadow, &parts[2], Transform::default());
        super::lod::append(&mut shadow, &far_needles, Transform::default());
        shadows.push(meshes.add(shadow.mesh()));
        let near = parts.map(|p| meshes.add(p.mesh()));
        distant.push([
            near[0].clone(),
            meshes.add(far_needles.mesh()),
            near[2].clone(),
        ]);
        models.push(near);
    }
    let materials = [
        Color::srgb(0.52, 0.30, 0.19),
        Color::WHITE,
        Color::srgb(0.62, 0.65, 0.61),
    ]
    .map(|color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.96,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    });
    commands.insert_resource(AlpineAssets {
        shadows,
        models,
        distant,
        materials,
    });
    commands.init_resource::<Streaming>();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stream(
    mut coordinator: ResMut<Coordinator>,
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    range: Option<Res<crate::rendering::view_distance::Range>>,
    mut previous_radius: Local<i32>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    view: Option<Res<crate::player::camera::CameraView>>,
    assets: Res<AlpineAssets>,
    mut state: ResMut<Streaming>,
    mut parts: Query<(&mut AlpinePart, &mut Mesh3d)>,
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
        state.discovery.update(center, radius);
    }
    state
        .discovery
        .poll(&mut coordinator, Layer::Alpine, placement);
    for _ in 0..TREES_PER_FRAME {
        if state.discovery.front().is_none() {
            break;
        }
        let Some(_installation) = coordinator.install_entities(Layer::Alpine, 1, 0, 5) else {
            break;
        };
        let Some((cell, transform)) = state.discovery.pop() else {
            break;
        };
        let Some(transform) = transform else {
            continue;
        };
        let entity = {
            let variant = hash(cell) as usize % assets.models.len();
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
                    Name::new("Windswept alpine bristlecone"),
                    transform,
                    Visibility::Inherited,
                ))
                .with_children(|tree| {
                    tree.spawn((
                        Name::new("Bristlecone shadow canopy"),
                        Mesh3d(assets.shadows[variant].clone()),
                        MeshMaterial3d(assets.materials[1].clone()),
                        super::lod::shadow_layers(),
                        crate::rendering::plant_lod::ShadowProxy,
                        Transform::default(),
                    ));
                    for (part, (mesh, material)) in model.iter().zip(&assets.materials).enumerate()
                    {
                        tree.spawn((
                            AlpinePart {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpine_altitude_boundary() {
        assert!(!altitude_allowed(150.0));
        assert!(!altitude_allowed(149.999));
        assert!(altitude_allowed(150.001));
        assert!(!altitude_allowed(f32::NAN));
    }

    #[test]
    fn bristlecone_geometry_is_finite_seeded_and_lod_preserves_extent() {
        let first = generate(721);
        let repeat = generate(721);
        let other = generate(922);
        assert_ne!(first[0].positions, other[0].positions);
        for (mesh, same) in first.iter().zip(&repeat) {
            assert_eq!(mesh.positions, same.positions);
            assert!(!mesh.indices.is_empty());
            assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(mesh.normals.iter().flatten().all(|v| v.is_finite()));
            assert!(
                mesh.indices
                    .iter()
                    .all(|i| (*i as usize) < mesh.positions.len())
            );
        }
        let far = winter::distant_needles(&first[1]);
        assert!(far.indices.len() < first[1].indices.len() / 3);
        let extent = |mesh: &Geometry| {
            mesh.positions
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max)
        };
        assert!((extent(&far) - extent(&first[1])).abs() < 0.2);
    }
}
