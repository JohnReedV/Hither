//! Shared procedural plant morphology, deterministically scattered in forest patches.
use crate::world::streaming::{Coordinator, Layer};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::HashMap;

const CHUNK: f32 = 12.0;
const VARIANTS: usize = 6;
const SPECIES: usize = 18; // Nine appearances per biome; two species are shared.
const NAMES: [&str; SPECIES] = [
    "Arching fern",
    "Broadleaf shrub",
    "Bramble",
    "Wood sorrel",
    "Forest lily",
    "Woodland sedge",
    "Bluebell",
    "Bearberry",
    "Clubmoss",
    "Shield fern",
    "Bilberry",
    "Heather",
    "Dwarf willow",
    "Creeping juniper",
    "Cotton sedge",
    "Bunchberry",
    "Bearberry",
    "Clubmoss",
];
pub struct UnderstoryPlugin;
impl Plugin for UnderstoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stream>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                stream
                    .in_set(Layer::Understory)
                    .after(crate::player::movement::move_camera),
            );
    }
}
#[derive(Resource)]
struct Plants {
    meshes: Vec<Handle<Mesh>>,
    material: Handle<StandardMaterial>,
}
#[derive(Resource, Default)]
struct Stream {
    loaded: HashMap<IVec2, Vec<Entity>>,
    discovery: crate::world::streaming::Discovery<Vec<(usize, Transform)>>,
    region: Option<(IVec2, i32)>,
}
struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut n = self.0;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((n ^ (n >> 31)) >> 40) as f32 / 16777216.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}
fn seed(cell: IVec2) -> u64 {
    crate::world::biome::world_seed()
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b9)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b)
        ^ 0x756e646572
}
#[derive(Default)]
struct Geometry {
    p: Vec<[f32; 3]>,
    n: Vec<[f32; 3]>,
    c: Vec<[f32; 4]>,
    i: Vec<u32>,
}
impl Geometry {
    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Vec3) {
        let start = self.p.len() as u32;
        let normal = (b - a).cross(c - a).normalize_or(Vec3::Y);
        for (p, shade) in [(a, 0.72), (b, 1.0), (c, 0.9)] {
            self.p.push(p.to_array());
            self.n.push(normal.to_array());
            self.c.push((color * shade).extend(1.0).to_array());
        }
        self.i.extend([start, start + 1, start + 2]);
    }
    fn stem(&mut self, a: Vec3, b: Vec3, width: f32, color: Vec3) {
        let side = (b - a).normalize_or(Vec3::Y).any_orthonormal_vector() * width;
        self.triangle(a - side, a + side, b + side, color);
        self.triangle(a - side, b + side, b - side, color);
    }
    // Raised, tapered snow ridge following the actual supporting branch.
    fn snow_cap(&mut self, a: Vec3, b: Vec3, width: f32) {
        let side = (b - a).cross(Vec3::Y).normalize_or(Vec3::X);
        let white = Vec3::new(0.84, 0.90, 0.96);
        for section in 0..2 {
            let t0 = section as f32 * 0.5;
            let t1 = t0 + 0.5;
            let p = a.lerp(b, t0) + Vec3::Y * 0.007;
            let q = a.lerp(b, t1) + Vec3::Y * 0.007;
            let w0 = width * if section == 0 { 0.45 } else { 1.0 };
            let w1 = width * if section == 0 { 1.0 } else { 0.25 };
            let rp = p + Vec3::Y * w0 * 0.65;
            let rq = q + Vec3::Y * w1 * 0.65;
            self.triangle(p - side * w0, q - side * w1, rp, white);
            self.triangle(q - side * w1, rq, rp, white);
            self.triangle(rp, rq, p + side * w0, white);
            self.triangle(p + side * w0, rq, q + side * w1, white);
        }
    }
    fn leaf(&mut self, a: Vec3, b: Vec3, width: f32, color: Vec3, snow: bool) {
        let side = (b - a).cross(Vec3::Y).normalize_or(Vec3::X) * width;
        let mid = a.lerp(b, 0.48);
        let ridge = mid + Vec3::Y * width * 0.25;
        let tint = if snow {
            color.lerp(Vec3::new(0.65, 0.72, 0.76), 0.7)
        } else {
            color
        };
        self.triangle(a, mid + side, ridge, tint);
        self.triangle(mid + side, b, ridge, tint);
        self.triangle(a, ridge, mid - side, color);
        self.triangle(mid - side, ridge, b, color);
        if snow {
            self.snow_cap(a.lerp(b, 0.15), a.lerp(b, 0.9), width * 0.85);
        }
    }
    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.p)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.n)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.c)
        .with_inserted_indices(Indices::U32(self.i))
    }
}
fn generate(kind: usize, variant: usize) -> Mesh {
    // Shared species retain the same underlying morphology in both climates.
    let identity = if kind >= 16 { kind - 9 } else { kind };
    let mut rng = Rng(seed(IVec2::new(identity as i32, variant as i32)));
    let mut g = Geometry::default();
    let winter = kind >= 9;
    let green = if winter {
        Vec3::new(0.035, 0.085, 0.055)
    } else {
        Vec3::new(0.055, 0.15, 0.028)
    };
    let wood = Vec3::new(0.10, 0.063, 0.028);
    if matches!(kind,3..=8 | 12..=17) {
        new_growth(&mut g, identity, winter, &mut rng);
    } else if kind == 0 || kind == 9 {
        // Radiating, arching fern fronds with alternating tapered pinnae.
        let count = 7 + (rng.unit() * 4.0) as usize;
        for f in 0..count {
            let angle = f as f32 * std::f32::consts::TAU / count as f32 + rng.range(-0.2, 0.2);
            let direction = Vec3::new(angle.cos(), 0.0, angle.sin());
            let side = direction.cross(Vec3::Y);
            let length = rng.range(0.35, 0.72) * if winter { 0.7 } else { 1.0 };
            let height = rng.range(0.25, 0.52);
            let point = |t: f32| {
                direction * length * t
                    + Vec3::Y * (0.025 + height * (std::f32::consts::PI * t * 0.78).sin())
            };
            for step in 1..=9 {
                let t = step as f32 / 10.0;
                let root = point(t);
                g.stem(point(t - 0.1), root, 0.003, green * 0.7);
                if winter && (4..=7).contains(&step) {
                    g.snow_cap(point(t - 0.1), root, 0.013);
                }
                for sign in [-1.0, 1.0] {
                    let span = length * 0.29 * (1.0 - t).powf(0.6) * rng.range(0.85, 1.15);
                    let end = root + side * sign * span + direction * span * 0.3;
                    g.leaf(
                        root,
                        end,
                        span * 0.20,
                        green * rng.range(0.8, 1.3),
                        winter && step % 4 == 0,
                    );
                }
            }
            g.leaf(point(0.9), point(1.0), 0.012, green, false);
        }
    } else {
        // Branching shrubs: broadleaf / bramble, dwarf bilberry / heather.
        let heather = kind == 11;
        let branches = if heather { 12 } else { 7 };
        for _ in 0..branches {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let direction = Vec3::new(a.cos(), 0.0, a.sin());
            let height = rng.range(0.3, 0.7) * if winter { 0.65 } else { 1.0 };
            let end = direction * rng.range(0.12, 0.35) + Vec3::Y * height;
            let bend = end * 0.45 + Vec3::Y * 0.08;
            g.stem(Vec3::ZERO, bend, 0.008, wood);
            g.stem(bend, end, 0.004, wood);
            if winter {
                g.snow_cap(bend.lerp(end, 0.05), bend.lerp(end, 0.88), 0.022);
                g.snow_cap(bend * 0.65, bend, 0.026);
            }
            for j in 1..=6 {
                let t = j as f32 / 7.0;
                let root = bend.lerp(end, t);
                let side = Quat::from_rotation_y(j as f32 * 2.4) * direction;
                let tip = root + side * rng.range(0.07, 0.18) + Vec3::Y * 0.045;
                g.stem(root, tip, 0.002, wood);
                if winter && !(j + variant).is_multiple_of(3) {
                    g.snow_cap(root, root.lerp(tip, 0.85), 0.012);
                }
                let width = if heather {
                    0.018
                } else if kind == 2 {
                    0.047
                } else {
                    0.035
                };
                g.leaf(
                    root,
                    tip,
                    width,
                    green * rng.range(0.7, 1.3),
                    winter && j % 3 == 0,
                );
                if kind == 2 {
                    g.leaf(
                        root,
                        root.lerp(tip, 0.8) + direction * 0.06,
                        width * 0.7,
                        green,
                        false,
                    );
                }
                if heather && j >= 4 {
                    let pink = Vec3::new(0.23, 0.09, 0.15);
                    g.leaf(tip, tip + Vec3::Y * 0.035, 0.018, pink, false);
                }
            }
        }
    }
    g.mesh()
}
fn new_growth(g: &mut Geometry, kind: usize, winter: bool, rng: &mut Rng) {
    let tau = std::f32::consts::TAU;
    let green = if winter {
        Vec3::new(0.035, 0.10, 0.065)
    } else {
        Vec3::new(0.06, 0.18, 0.035)
    };
    let wood = Vec3::new(0.09, 0.055, 0.027);
    let count = 8 + (rng.unit() * 5.0) as usize;
    for shoot in 0..count {
        let angle = shoot as f32 * tau / count as f32 + rng.range(-0.25, 0.25);
        let d = Vec3::new(angle.cos(), 0.0, angle.sin());
        let root = d * rng.range(0.015, 0.14) + Vec3::Y * 0.015;
        let tint = green * rng.range(0.75, 1.25);
        match kind {
            5 | 14 => {
                // Upright sedges: long arched ribbon leaves, not woody shrubs.
                let h = rng.range(0.3, 0.65);
                let length = rng.range(0.15, 0.30);
                let mut previous = root;
                for step in 1..=5 {
                    let t = step as f32 / 5.0;
                    let tip = root + d * length * t * t + Vec3::Y * h * (t - 0.35 * t * t);
                    g.leaf(previous, tip, 0.012 * (1.1 - t), tint, false);
                    if winter && step == 3 {
                        g.snow_cap(previous, tip, 0.014);
                    }
                    previous = tip;
                }
                if kind == 14 {
                    let top = root + Vec3::Y * h;
                    g.stem(root, top, 0.003, wood);
                    for petal in 0..6 {
                        let a = petal as f32 * tau / 6.0;
                        let side = Vec3::new(a.cos(), 0.0, a.sin()) * 0.022;
                        g.leaf(
                            top - side,
                            top + Vec3::Y * 0.065 + side,
                            0.027,
                            Vec3::new(0.7, 0.69, 0.57),
                            false,
                        );
                    }
                }
            }
            8 | 13 => {
                // Clubmoss fingers versus spreading, blue-green juniper boughs.
                let end = root + d * rng.range(0.18, 0.35) + Vec3::Y * rng.range(0.1, 0.2);
                g.stem(root, end, 0.004, wood);
                if winter {
                    g.snow_cap(root, end, 0.025);
                }
                for branch in 1..=5 {
                    let base = root.lerp(end, branch as f32 / 6.0);
                    let tip = base + Vec3::Y * rng.range(0.08, 0.19) + d * 0.025;
                    g.stem(base, tip, 0.003, tint);
                    for level in 0..3 {
                        for radial in 0..2 {
                            let a = radial as f32 * tau / 2.0 + level as f32;
                            let origin = base.lerp(tip, level as f32 / 3.0);
                            let axis = Vec3::new(a.cos(), 0.6, a.sin());
                            let size = if kind == 13 { 0.07 } else { 0.035 };
                            let color = if kind == 13 {
                                tint * Vec3::new(0.7, 0.85, 1.5)
                            } else {
                                tint
                            };
                            g.leaf(origin, origin + axis * size, 0.007, color, false);
                        }
                    }
                }
            }
            7 | 12 => {
                // Bearberry carpets creep outwards; dwarf willow rises on
                // irregular woody forks with much broader rounded leaves.
                let h = if kind == 7 {
                    rng.range(0.07, 0.14)
                } else {
                    rng.range(0.25, 0.46)
                };
                let end = root + d * rng.range(0.2, 0.4) + Vec3::Y * h;
                g.stem(root, end, 0.005, wood);
                if winter {
                    g.snow_cap(root, end, 0.024);
                }
                for leaf in 1..=6 {
                    let base = root.lerp(end, leaf as f32 / 7.0);
                    let side = Quat::from_rotation_y(if leaf % 2 == 0 { 1.0 } else { -1.0 }) * d;
                    let tip = base + side * rng.range(0.06, 0.11) + Vec3::Y * 0.035;
                    g.leaf(
                        base,
                        tip,
                        if kind == 7 { 0.026 } else { 0.045 },
                        tint,
                        winter && leaf % 3 == 0,
                    );
                    if kind == 7 && leaf == 5 {
                        for berry in 0..3 {
                            let p = tip + Vec3::X * berry as f32 * 0.012;
                            g.leaf(
                                p,
                                p + Vec3::Y * 0.018,
                                0.012,
                                Vec3::new(0.32, 0.035, 0.018),
                                false,
                            );
                        }
                    }
                }
            }
            _ => {
                // Sorrel trifoliate cushions, broad lily rosettes, bluebell
                // stalks and low four-leaf bunchberry whorls.
                let h = match kind {
                    3 => 0.12,
                    4 => 0.27,
                    6 => 0.42,
                    _ => 0.18,
                } * rng.range(0.7, 1.2);
                let top = root + Vec3::Y * h + d * 0.05;
                g.stem(root, top, 0.003, tint);
                if winter {
                    g.snow_cap(root.lerp(top, 0.5), top, 0.018);
                }
                let lobes = if kind == 3 { 3 } else { 4 };
                for lobe in 0..lobes {
                    let a = angle + lobe as f32 * tau / lobes as f32;
                    let axis = Vec3::new(a.cos(), 0.2, a.sin());
                    let size = if kind == 4 { 0.23 } else { 0.09 };
                    g.leaf(
                        top,
                        top + axis * size,
                        size * 0.38,
                        tint,
                        winter && lobe % 2 == 0,
                    );
                }
                if kind == 4 || kind == 6 || kind == 15 {
                    let flower = top + Vec3::Y * if kind == 6 { 0.12 } else { 0.045 };
                    g.stem(top, flower, 0.002, tint);
                    let color = match kind {
                        6 => Vec3::new(0.13, 0.07, 0.36),
                        4 => Vec3::new(0.38, 0.28, 0.08),
                        _ => Vec3::new(0.4, 0.06, 0.025),
                    };
                    for petal in 0..5 {
                        let a = petal as f32 * tau / 5.0;
                        g.leaf(
                            flower,
                            flower + Vec3::new(a.cos() * 0.045, -0.03, a.sin() * 0.045),
                            0.02,
                            color,
                            false,
                        );
                    }
                }
            }
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let bank = (0..SPECIES)
        .flat_map(|k| (0..VARIANTS).map(move |v| generate(k, v)))
        .map(|m| meshes.add(m))
        .collect();
    commands.insert_resource(Plants {
        meshes: bank,
        material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.08,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
    });
}
fn placements(cell: IVec2) -> Vec<(usize, Transform)> {
    let mut rng = Rng(seed(cell));
    let mut result = Vec::new();
    for x in 0..4 {
        for z in 0..4 {
            let p = cell.as_vec2() * CHUNK
                + Vec2::new(x as f32 + rng.unit(), z as f32 + rng.unit()) * 3.0;
            if p.x.abs() < 10.0 && p.y.abs() < 10.0 {
                continue;
            }
            if rng.unit() > crate::world::biome::mountain_habitat(p) {
                continue;
            }
            let forest = crate::world::biome::forest_amount(p);
            let snow = crate::world::biome::snow_amount(p);
            if forest < 0.55 || (0.25..0.65).contains(&snow) {
                continue;
            }
            let patch = crate::world::biome::stand_density(p + Vec2::new(19.0, -31.0));
            if rng.unit() > (0.12 + 0.42 * patch) * (forest - 0.5) * 2.0 {
                continue;
            }
            let winter = snow >= 0.65;
            // Most sites use a patch-local favorite; occasional mixing avoids
            // monotonous colonies while keeping the floor from looking like a sampler.
            let mut colony = Rng(seed((p / 18.0).floor().as_ivec2()) ^ 0x636f6c6f6e79);
            let local = if rng.unit() < 0.6 {
                colony.unit()
            } else {
                rng.unit()
            };
            let kind = (if winter { 9 } else { 0 }) + (local * 9.0) as usize;
            let variant = (rng.unit() * VARIANTS as f32) as usize;
            let position = Vec3::new(p.x, crate::world::biome::terrain_height(p), p.y);
            if crate::world::orchard::wild::player_collides(position, 0.18)
                || crate::world::orchard::winter::player_collides(position, 0.18)
                || crate::world::orchard::alpine::player_collides(position, 0.18)
                || crate::world::orchard::oak::player_collides(position, 0.18)
                || crate::world::orcs::entrance_overlap(position, 1.8)
            {
                continue;
            }
            let rotation = Quat::from_rotation_y(rng.unit() * std::f32::consts::TAU);
            let scale = Vec3::new(
                rng.range(0.75, 1.25),
                rng.range(0.7, 1.3),
                rng.range(0.75, 1.25),
            );
            let Some(ground) =
                crate::world::terrain::vegetation_anchor(p, 0.15 * scale.x.max(scale.z), 0.8)
            else {
                continue;
            };
            result.push((
                kind * VARIANTS + variant,
                Transform::from_translation(position.with_y(ground))
                    .with_rotation(rotation)
                    .with_scale(scale),
            ));
        }
    }
    result
}
fn stream(
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    range: Res<crate::rendering::view_distance::Range>,
    plants: Res<Plants>,
    mut state: ResMut<Stream>,
) {
    let center = (rig.position.xz() / CHUNK).floor().as_ivec2();
    let radius = (range.load / CHUNK).ceil() as i32 + 1;
    if state.region != Some((center, radius)) {
        state.region = Some((center, radius));
        state.loaded.retain(|cell, entities| {
            if (*cell - center).abs().max_element() <= radius {
                return true;
            }
            for entity in entities {
                crate::world::streaming::retire(&mut commands, *entity);
            }
            false
        });
        state.discovery.update(center, radius);
    }
    state
        .discovery
        .poll(&mut coordinator, Layer::Understory, placements);
    for _ in 0..4 {
        if state.discovery.front().is_none() {
            break;
        }
        let entities = state.discovery.front().unwrap().1.len();
        let Some(_installation) = coordinator.install_entities(Layer::Understory, 4, 0, entities)
        else {
            break;
        };
        let Some((cell, sites)) = state.discovery.pop() else {
            break;
        };
        let entities = sites
            .into_iter()
            .map(|(mesh, transform)| {
                commands
                    .spawn((
                        Name::new(NAMES[mesh / VARIANTS]),
                        crate::rendering::plant_lod::Canopy(true),
                        Mesh3d(plants.meshes[mesh].clone()),
                        MeshMaterial3d(plants.material.clone()),
                        transform,
                        NotShadowCaster,
                    ))
                    .id()
            })
            .collect();
        state.loaded.insert(cell, entities);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nine_species_per_forest_with_two_shared_identities() {
        let warm: std::collections::HashSet<_> = NAMES[..9].iter().collect();
        let winter: std::collections::HashSet<_> = NAMES[9..].iter().collect();
        assert_eq!(warm.len(), 9);
        assert_eq!(winter.len(), 9);
        assert_eq!(warm.intersection(&winter).count(), 2);
        assert!(warm.contains(&"Bearberry") && winter.contains(&"Clubmoss"));
    }
    #[test]
    fn all_morphologies_are_finite_bounded_and_varied() {
        for kind in 0..SPECIES {
            let first = generate(kind, 0);
            assert_ne!(
                format!("{:?}", first.attribute(Mesh::ATTRIBUTE_POSITION)),
                format!(
                    "{:?}",
                    generate(kind, 1).attribute(Mesh::ATTRIBUTE_POSITION)
                )
            );
            for variant in 0..VARIANTS {
                let mesh = generate(kind, variant);
                assert!(mesh.count_vertices() > 100 && mesh.count_vertices() < 7000);
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!()
                };
                assert!(
                    p.iter()
                        .all(|p| Vec3::from(*p).is_finite() && p[1] >= -0.01 && p[1] < 1.0)
                );
            }
        }
    }
    #[test]
    fn every_winter_variant_has_bright_geometric_snow() {
        for kind in 0..SPECIES {
            for variant in 0..VARIANTS {
                let mesh = generate(kind, variant);
                let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!()
                };
                let snow_vertices = colors
                    .iter()
                    .filter(|c| c[0] > 0.5 && c[1] > 0.5 && c[2] > 0.5)
                    .count();
                if kind >= 9 {
                    assert!(snow_vertices > 100);
                } else {
                    assert_eq!(snow_vertices, 0);
                }
            }
        }
    }
    #[test]
    fn placement_is_repeatable_forest_only_and_castle_clear() {
        for x in -10..10 {
            for z in -10..10 {
                let cell = IVec2::new(x, z);
                let a = placements(cell);
                assert_eq!(a, placements(cell));
                for (kind, t) in a {
                    assert!(kind < SPECIES * VARIANTS);
                    assert!((t.rotation * Vec3::Y - Vec3::Y).length() < 0.00001);
                    assert!(t.translation.y <= crate::world::terrain::height(t.translation.xz()));
                    assert!(crate::world::biome::forest_amount(t.translation.xz()) >= 0.55);
                    assert!(t.translation.x.abs() >= 10.0 || t.translation.z.abs() >= 10.0);
                }
            }
        }
    }
}
