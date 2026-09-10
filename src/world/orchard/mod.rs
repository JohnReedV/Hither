//! Botanical mesh detail over the SDF courtyard. The same triangles are used
//! for rendering and crosshair occlusion; foliage is not an invisible sphere.
use std::{
    f32::consts::{PI, TAU},
    sync::OnceLock,
};

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::world::orchard::fruit::APPLE_RADIUS;
#[cfg(test)]
use crate::world::orchard::fruit::{MAX_APPLES, OrchardState};

pub(crate) mod alpine;
mod apple_tree;
mod conifer_lod;
mod forest;
mod lod;
pub(crate) mod logs;
pub(crate) mod oak;
mod orange_tree;
mod placement_cache;
pub(crate) mod wild;
pub(crate) mod winter;

pub struct OrchardPlugin;

impl Plugin for OrchardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                oak::setup,
                wild::setup,
                winter::setup,
                alpine::setup,
                logs::setup,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                logs::stream.in_set(crate::world::streaming::Layer::Logs),
                wild::stream
                    .in_set(crate::world::streaming::Layer::Citrus)
                    .after(fruit::pick_fruit),
                winter::stream.in_set(crate::world::streaming::Layer::Conifers),
                oak::stream.in_set(crate::world::streaming::Layer::Oaks),
                alpine::stream.in_set(crate::world::streaming::Layer::Alpine),
            )
                .after(crate::player::movement::move_camera),
        );
    }
}

use crate::rendering::geometry::{Geometry, RayMesh};
use crate::world::random::Rng;

/// Continuous bent, tapering tubes; roots and branch bases overlap inside the
/// parent wood. Bark relief is geometry, not just a flat brown material.
fn curved_path(path: &[(Vec3, f32)]) -> Vec<(Vec3, f32)> {
    let mut curved = Vec::new();
    for segment in 0..path.len() - 1 {
        let p0 = path[segment.saturating_sub(1)].0;
        let p1 = path[segment].0;
        let p2 = path[segment + 1].0;
        let p3 = path[(segment + 2).min(path.len() - 1)].0;
        for step in 0..4 {
            let t = step as f32 / 4.0;
            let point = 0.5
                * ((2.0 * p1)
                    + (-p0 + p2) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t);
            curved.push((point, path[segment].1 * (1.0 - t) + path[segment + 1].1 * t));
        }
    }
    curved.push(*path.last().unwrap());
    curved
}

fn branch(mesh: &mut Geometry, path: &[(Vec3, f32)], sides: usize, seed: f32) {
    let curved = curved_path(path);
    let path = &curved;
    let base = mesh.positions.len() as u32;
    let mut length = 0.0;
    let mut previous_right: Option<Vec3> = None;
    for (i, &(center, radius)) in path.iter().enumerate() {
        let tangent =
            (path[(i + 1).min(path.len() - 1)].0 - path[i.saturating_sub(1)].0).normalize();
        // Transport the ring frame along the curve; choosing an unrelated
        // perpendicular at each ring can flip it and twist bark into ribbons.
        let right = previous_right
            .map(|r| (r - tangent * r.dot(tangent)).normalize_or(tangent.any_orthonormal_vector()))
            .unwrap_or_else(|| tangent.any_orthonormal_vector());
        previous_right = Some(right);
        let forward = tangent.cross(right);
        if i > 0 {
            length += center.distance(path[i - 1].0);
        }
        for s in 0..=sides {
            let u = s as f32 / sides as f32;
            let angle = u * TAU;
            let ridge = 1.0
                + 0.065 * (angle * 9.0 + seed + length * 0.7).sin()
                + 0.025 * (angle * 17.0 + length * 2.2).sin();
            let p = center + (right * angle.cos() + forward * angle.sin()) * radius * ridge;
            mesh.vertex(
                p,
                Vec2::new(u * radius * 12.0, length * 1.5),
                Vec3::splat(0.8 + 0.12 * (angle * 5.0 + seed).sin()),
            );
            if i > 0 && s > 0 {
                let a = base + ((i - 1) * (sides + 1) + s - 1) as u32;
                let b = a + (sides + 1) as u32;
                mesh.triangle(a, a + 1, b);
                mesh.triangle(a + 1, b + 1, b);
            }
        }
    }
    // Small terminal disks, avoiding open ends on twigs and apple stems.
    let end = path.last().unwrap();
    let cap = mesh.vertex(end.0, Vec2::ZERO, Vec3::splat(0.65));
    let ring = base + ((path.len() - 1) * (sides + 1)) as u32;
    for s in 0..sides as u32 {
        mesh.triangle(cap, ring + s, ring + s + 1);
    }
}

/// Curved, finely serrated leaf blade with a raised midrib and tapered tip.
fn leaf(mesh: &mut Geometry, base: Vec3, axis: Vec3, length: f32, roll: f32, color: Vec3) {
    let direction = axis.normalize();
    let side = Quat::from_axis_angle(direction, roll) * direction.any_orthonormal_vector();
    let normal = side.cross(direction);
    let start = mesh.positions.len() as u32;
    const ROWS: usize = 8;
    for row in 0..=ROWS {
        let t = row as f32 / ROWS as f32;
        let width = (PI * t).sin().max(0.0).powf(0.85)
            * length
            * 0.235
            * if row % 2 == 0 { 0.92 } else { 1.0 };
        let center = base + direction * (t * length) + normal * (t * t * length * 0.16);
        for lane in 0..3 {
            let x = lane as f32 - 1.0;
            let p = center
                + side * (width * x)
                + normal * (1.0 - x.abs()) * length * 0.045 * (PI * t).sin();
            mesh.vertex(p, Vec2::new(lane as f32 * 0.5, t), color);
            if row > 0 && lane > 0 {
                let a = start + ((row - 1) * 3 + lane - 1) as u32;
                mesh.triangle(a, a + 1, a + 3);
                mesh.triangle(a + 1, a + 4, a + 3);
            }
        }
    }
}

struct Tree {
    distant_wood: Option<Geometry>,
    wood: Geometry,
    leaves: Geometry,
    rays: RayMesh,
    fruit_spurs: Vec<[Vec3; 2]>,
    camera_bounds: Vec<(Vec3, f32)>,
}

/// Sample the same piecewise centerline that branch() actually renders.
fn path_point(path: &[(Vec3, f32)], t: f32) -> (Vec3, f32) {
    let f = t.clamp(0.0, 1.0) * (path.len() - 1) as f32;
    let i = (f as usize).min(path.len() - 2);
    let u = f - i as f32;
    (
        path[i].0.lerp(path[i + 1].0, u),
        path[i].1 * (1.0 - u) + path[i + 1].1 * u,
    )
}

#[cfg(test)]
fn tree() -> &'static Tree {
    static TREE: OnceLock<Tree> = OnceLock::new();
    TREE.get_or_init(|| {
        // Separate from the growth RNG: generating geometry must not consume
        // fruit-growth rolls. A debug seed reproduces a reported tree exactly.
        let seed = std::env::var("HITHER_TREE_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                if cfg!(test) {
                    return 0x6170706c655f7472;
                }
                crate::world::biome::world_seed() ^ 0x6170706c655f7472
            });
        info!("Generating apple tree with seed {seed}");
        generate_tree(seed)
    })
}

fn generate_tree(seed: u64) -> Tree {
    apple_tree::generate(seed)
}

fn generate_orange_tree(seed: u64) -> Tree {
    orange_tree::generate(seed)
}

fn apple() -> &'static Geometry {
    static APPLE: OnceLock<Geometry> = OnceLock::new();
    APPLE.get_or_init(|| {
        let mut mesh = Geometry::default();
        const RINGS: usize = 48;
        const SIDES: usize = 80;
        for row in 0..=RINGS {
            let v = row as f32 / RINGS as f32;
            let theta = v * PI;
            let y = theta.cos();
            for col in 0..=SIDES {
                let u = col as f32 / SIDES as f32;
                let a = u * TAU;
                let lobes = 1.0 + 0.028 * (a * 5.0).cos() * y.abs().powi(3);
                let r = theta.sin() * APPLE_RADIUS * (1.0 + 0.075 * y) * lobes;
                let height = APPLE_RADIUS
                    * (y * 0.96 - 0.19 * (-theta * theta / 0.07).exp()
                        + 0.10 * (-(PI - theta).powi(2) / 0.09).exp());
                mesh.vertex(
                    Vec3::new(r * a.cos(), height, r * a.sin()),
                    Vec2::new(u, v),
                    Vec3::ONE,
                );
                if row > 0 && col > 0 {
                    let a = ((row - 1) * (SIDES + 1) + col - 1) as u32;
                    let b = a + (SIDES + 1) as u32;
                    mesh.triangle(a, a + 1, b);
                    mesh.triangle(a + 1, b + 1, b);
                }
            }
        }
        mesh.finish_normals();
        // Weld shading across the wrap, without merging UV coordinates.
        for row in 0..=RINGS {
            let a = row * (SIDES + 1);
            let b = a + SIDES;
            let normal =
                (Vec3::from(mesh.normals[a]) + Vec3::from(mesh.normals[b])).normalize_or(Vec3::Y);
            mesh.normals[a] = normal.to_array();
            mesh.normals[b] = normal.to_array();
        }
        mesh
    })
}

fn apple_rays() -> &'static RayMesh {
    static APPLE: OnceLock<RayMesh> = OnceLock::new();
    APPLE.get_or_init(|| RayMesh::new(apple().triangles()))
}

fn apple_rotation(center: Vec3) -> Quat {
    Quat::from_rotation_y((center.x * 17.13 + center.z * 23.7 + center.y * 7.0).sin() * PI)
}

pub(crate) fn apple_hit(origin: Vec3, direction: Vec3, center: Vec3) -> Option<f32> {
    let inverse = apple_rotation(center).inverse();
    apple_rays().hit(inverse * (origin - center), inverse * direction, 70.0)
}

#[cfg(test)]
pub(crate) fn tree_occludes(origin: Vec3, direction: Vec3, limit: f32) -> bool {
    tree().rays.hit(origin, direction, limit).is_some()
}

#[cfg(test)]
pub(crate) fn hanging_position(branch_choice: u32, along: f32, drop: f32) -> Vec3 {
    tree().hanging_position(branch_choice, along, drop)
}

#[cfg(test)]
fn fruit_anchor(center: Vec3) -> Option<Vec3> {
    tree().fruit_anchor(center)
}

#[cfg(test)]
pub(crate) fn fruit_has_clearance(center: Vec3) -> bool {
    tree().fruit_has_clearance(center)
}

#[cfg(test)]
impl Tree {
    fn hanging_position(&self, branch_choice: u32, along: f32, drop: f32) -> Vec3 {
        let [a, b] = self.fruit_spurs[branch_choice as usize % self.fruit_spurs.len()];
        a.lerp(b, along) - Vec3::Y * (0.25 + 0.13 * drop)
    }

    fn fruit_anchor(&self, center: Vec3) -> Option<Vec3> {
        self.fruit_spurs
            .iter()
            .filter_map(|[a, b]| {
                let delta = b.xz() - a.xz();
                let t = ((center.xz() - a.xz()).dot(delta) / delta.length_squared().max(1e-12))
                    .clamp(0.0, 1.0);
                let p = a.lerp(*b, t);
                let drop = p.y - center.y;
                (p.xz().distance_squared(center.xz()) < 1e-8 && (0.24..0.39).contains(&drop))
                    .then_some(p)
            })
            .min_by(|a, b| a.y.total_cmp(&b.y))
    }

    fn fruit_has_clearance(&self, center: Vec3) -> bool {
        if !(1.5..=2.85).contains(&center.y) || center.xz().length() < 0.5 {
            return false;
        }
        // Fruit hangs below the shoot with room around its body, not embedded in
        // leaves/wood. No forced fallback may place an invalid fruit when crowded.
        // A conservative sphere encloses the whole lobed apple. Unlike six
        // probe rays, this also catches diagonal leaves clipping its shoulders.
        !self.rays.overlaps_sphere(center, APPLE_RADIUS * 1.12)
    }
}

// Deterministic surface maps are authored here, without photos or downloads.
// Colors are sRGB; roughness and normal data use linear textures.
fn noise(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h ^ (h >> 16)) as f32 / u32::MAX as f32
}

#[derive(Clone, Copy)]
enum SurfaceKind {
    Bark,
    Leaf,
    Apple,
    Orange,
    CitrusLeaf,
    CitrusBark,
}

fn surface(kind: SurfaceKind, u: f32, v: f32) -> (Vec3, f32, f32) {
    match kind {
        SurfaceKind::CitrusLeaf => {
            let (color, _, height) = surface(SurfaceKind::Leaf, u, v);
            (color * Vec3::new(0.62, 0.79, 1.20), 0.40, height * 0.55)
        }
        SurfaceKind::CitrusBark => {
            let fine = noise((u * 110.0) as i32, (v * 170.0) as i32);
            let grain = (u * TAU * 14.0 + (v * 9.0).sin() * 0.25).sin();
            (
                Vec3::new(0.30, 0.31, 0.26) * (0.75 + fine * 0.22),
                0.88,
                grain * 0.025 + fine * 0.015,
            )
        }
        SurfaceKind::Orange => {
            let pores = noise((u * 380.0) as i32, (v * 190.0) as i32);
            let mottle = noise((u * 38.0) as i32, (v * 19.0) as i32);
            (
                Vec3::new(0.95, 0.34 + mottle * 0.14, 0.012) * (0.90 + pores * 0.10),
                0.48 + pores * 0.12,
                pores * 0.12,
            )
        }
        SurfaceKind::Bark => {
            let grain =
                (u * TAU * 18.0 + (v * TAU * 2.0).sin() * 0.8 + (v * TAU * 7.0).sin() * 0.2).sin();
            let cracks = (1.0 - grain.abs()).powi(10);
            let fleck = noise((u * 320.0) as i32, (v * 620.0) as i32);
            let scales = noise((u * 68.0) as i32, (v * 95.0) as i32);
            let tone = 0.38 + 0.30 * scales + 0.19 * fleck - 0.23 * cracks;
            (
                Vec3::new(0.40, 0.33, 0.25) * tone,
                0.94,
                tone * 0.5 + grain * 0.12,
            )
        }
        SurfaceKind::Leaf => {
            let x = (u - 0.5).abs();
            let rib = (-x * 170.0).exp();
            let vein = ((v * 12.0 - x * 7.0) * PI).sin().abs();
            let veins = (1.0 - vein).powi(20) * (1.0 - x * 1.3);
            let mottle = noise((u * 90.0) as i32, (v * 150.0) as i32);
            let color = Vec3::new(0.22, 0.39, 0.085) * (0.83 + 0.24 * mottle)
                + Vec3::new(0.11, 0.12, 0.035) * (rib * 0.7 + veins * 0.25);
            (color, 0.72, rib * 0.22 + veins * 0.065)
        }
        SurfaceKind::Apple => {
            let angle = u * TAU;
            let bands = (0.5 + 0.5 * (angle * 23.0 + (v * 16.0).sin() * 0.6).sin()).powi(3)
                * (0.5 + 0.5 * (angle * 7.0 + v * 17.0).sin());
            let blush = 0.5 + 0.5 * (angle - 1.2).cos();
            let mottling = (angle * 11.0 + (v * 24.0).sin()).sin() * (v * 21.0 + angle.sin()).sin();
            let warm = (bands * 0.14 + (1.0 - blush) * 0.48 + mottling * 0.09).clamp(0.0, 1.0);
            let red = Vec3::new(0.49, 0.024, 0.032);
            let gold = Vec3::new(0.72, 0.32, 0.075);
            let mut color = red.lerp(gold, warm);
            let fine = noise((u * 720.0) as i32, (v * 400.0) as i32);
            let cell = Vec2::new(u * 155.0, v * 92.0);
            let seed = noise(cell.x.floor() as i32, cell.y.floor() as i32);
            let dot = (cell.fract() - Vec2::splat(0.25 + seed * 0.45)).length();
            let lenticel = seed > 0.68 && dot < 0.08;
            if lenticel {
                color = color.lerp(Vec3::new(0.86, 0.67, 0.37), 0.65);
            }
            let end = (v.min(1.0 - v) / 0.065).clamp(0.0, 1.0);
            color = Vec3::new(0.25, 0.16, 0.055).lerp(color, end);
            (
                color * (0.94 + 0.12 * fine),
                0.32 + 0.07 * fine + if lenticel { 0.1 } else { 0.0 },
                fine * 0.025,
            )
        }
    }
}

fn material(kind: SurfaceKind, images: &mut Assets<Image>) -> StandardMaterial {
    let (width, height) = match kind {
        SurfaceKind::Bark => (512, 1024),
        SurfaceKind::Leaf => (256, 512),
        SurfaceKind::Apple => (1024, 512),
        SurfaceKind::Orange => (512, 256),
        SurfaceKind::CitrusLeaf => (128, 256),
        SurfaceKind::CitrusBark => (256, 512),
    };
    let mut color = Vec::new();
    let mut rough = Vec::new();
    let mut normals = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            let (c, r, h) = surface(kind, u, v);
            color.extend([c.x, c.y, c.z, 1.0].map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8));
            rough.extend([255, (r * 255.0) as u8, 0, 255]);
            let dx = surface(kind, (u + 1.0 / width as f32).fract(), v).2 - h;
            let dy = surface(kind, u, (v + 1.0 / height as f32).fract()).2 - h;
            let n = Vec3::new(-dx * 2.0, -dy * 2.0, 1.0).normalize() * 0.5 + Vec3::splat(0.5);
            normals.extend([n.x, n.y, n.z, 1.0].map(|c| (c * 255.0) as u8));
        }
    }
    let mut add = |data, srgb, filter| {
        let mut image = Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            if srgb {
                TextureFormat::Rgba8UnormSrgb
            } else {
                TextureFormat::Rgba8Unorm
            },
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
        crate::rendering::mipmaps::generate(&mut image, filter);
        images.add(image)
    };
    StandardMaterial {
        base_color_texture: Some(add(color, true, crate::rendering::mipmaps::Filter::Color)),
        metallic_roughness_texture: Some(add(
            rough,
            false,
            crate::rendering::mipmaps::Filter::Linear,
        )),
        normal_map_texture: Some(add(
            normals,
            false,
            crate::rendering::mipmaps::Filter::Normal,
        )),
        perceptual_roughness: 1.0,
        reflectance: 0.24,
        double_sided: matches!(kind, SurfaceKind::Leaf | SurfaceKind::CitrusLeaf),
        cull_mode: if matches!(kind, SurfaceKind::Leaf | SurfaceKind::CitrusLeaf) {
            None
        } else {
            Some(bevy::render::render_resource::Face::Back)
        },
        ..default()
    }
}

fn simple_apple_leaves(source: &Geometry) -> Geometry {
    let mut result = Geometry::default();
    for leaf in 0..source.positions.len() / 27 {
        let first = result.positions.len() as u32;
        for offset in [1, 12, 25, 14] {
            let i = leaf * 27 + offset;
            result.positions.push(source.positions[i]);
            result.normals.push(source.normals[i]);
            result.uvs.push(source.uvs[i]);
            result.colors.push(source.colors[i]);
        }
        result
            .indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    result
}

#[cfg(test)]
#[derive(Component)]
struct AppleVisual {
    slot: usize,
    position: Option<Vec3>,
}

#[cfg(test)]
#[derive(Resource)]
struct OrchardMaterials {
    wood: Handle<StandardMaterial>,
    leaf: Handle<StandardMaterial>,
}

#[cfg(test)]
fn sync_apples(
    mut commands: Commands,
    orchard: Res<OrchardState>,
    materials: Res<OrchardMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut apples: Query<(Entity, &mut AppleVisual, &mut Transform, &mut Visibility)>,
) {
    for (entity, mut apple, mut transform, mut visible) in &mut apples {
        let next = orchard.apples.get(apple.slot).copied();
        if apple.position == next {
            continue;
        }
        apple.position = next;
        commands.entity(entity).despawn_children();
        *visible = if next.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some(center) = next {
            let rotation = apple_rotation(center);
            *transform = Transform::from_translation(center).with_rotation(rotation);
            let mut wood = Geometry::default();
            let mut leaves = Geometry::default();
            let top = Vec3::Y * (APPLE_RADIUS * 0.77);
            let stem = Vec3::new(0.013, 0.205, -0.01);
            let attachment = fruit_anchor(center).expect("Fruit must hang below its branch");
            let tip = rotation.inverse() * (attachment - center);
            branch(
                &mut wood,
                &[
                    (top, 0.010),
                    (Vec3::new(0.006, 0.157, 0.0), 0.008),
                    (stem, 0.005),
                ],
                8,
                0.0,
            );
            // A fine fruiting stalk actually joins the crown; fruit never floats.
            branch(
                &mut wood,
                &[
                    (stem, 0.004),
                    (stem.lerp(tip, 0.5) + Vec3::Y * 0.06, 0.005),
                    (tip, 0.007),
                ],
                6,
                0.0,
            );
            for i in 0..5 {
                let a = i as f32 * TAU / 5.0;
                leaf(
                    &mut wood,
                    Vec3::new(0.0, -0.120, 0.0),
                    Vec3::new(a.cos(), -0.2, a.sin()),
                    0.026,
                    0.0,
                    Vec3::splat(0.55),
                );
            }
            leaf(
                &mut leaves,
                stem,
                Vec3::new(1.0, 0.2, 0.5),
                0.13,
                0.7,
                Vec3::ONE,
            );
            wood.finish_normals();
            leaves.finish_normals();
            commands.entity(entity).with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.add(wood.mesh())),
                    MeshMaterial3d(materials.wood.clone()),
                    Transform::default(),
                ));
                parent.spawn((
                    Mesh3d(meshes.add(leaves.mesh())),
                    MeshMaterial3d(materials.leaf.clone()),
                    Transform::default(),
                ));
            });
        }
    }
}

#[cfg(test)]
mod tests;

pub(crate) mod fruit;
