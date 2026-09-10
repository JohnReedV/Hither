//! Shared weathered PBR torch and a view-independent, ray-marched living flame.
use super::*;
use bevy::{light::NotShadowCaster, render::render_resource::AsBindGroup, shader::ShaderRef};

pub(super) struct TorchPlugin;
impl Plugin for TorchPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FlameMaterial>::default())
            .add_systems(Update, animate_fire);
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct FlameMaterial {
    #[uniform(0)]
    motion: Vec4,
}
impl Material for FlameMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/torch.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/torch.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Default)]
pub(super) struct TorchArt {
    parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    volume: Handle<Mesh>,
    fire: Handle<FlameMaterial>,
}

// The uniform clock is shared; world-space phase offsets keep dens independent.
#[derive(Component)]
struct FireLight {
    phase: f32,
}

fn shaft_center(y: f32) -> Vec3 {
    Vec3::new(-y * 0.15, y, 0.)
}
fn shaft_radius(t: f32) -> f32 {
    0.046 + t * 0.024
}

fn wood_mesh() -> Mesh {
    let mut g = Geometry::default();
    const SIDES: usize = 96;
    const ROWS: usize = 64;
    for row in 0..=ROWS {
        let t = row as f32 / ROWS as f32;
        let y = -1.02 + t * 0.91;
        for side in 0..=SIDES {
            let u = side as f32 / SIDES as f32;
            let a = u * TAU;
            let ridges = 0.0016 * (a * 19. + t * 3.).sin()
                + 0.0011 * (a * 37. - t * 5.).sin()
                + 0.0017 * (a * 7. + t * 8.).sin();
            let radius = shaft_radius(t) + ridges;
            g.vertex(
                shaft_center(y) + Vec3::new(a.cos(), 0., a.sin()) * radius,
                Vec2::new(u, t),
                Vec3::ONE,
            );
            if row < ROWS && side < SIDES {
                let a = (row * (SIDES + 1) + side) as u32;
                let b = a + (SIDES + 1) as u32;
                g.triangle(a, b, a + 1);
                g.triangle(a + 1, b, b + 1);
            }
        }
    }
    // A worn end cap, with its own radial UVs and split rim normal.
    for (y, t, sign) in [(-1.02, 0., -1.), (-0.11, 1., 1.)] {
        let c = g.vertex(shaft_center(y), Vec2::splat(0.5), Vec3::splat(0.65));
        for i in 0..=SIDES {
            let a = i as f32 / SIDES as f32 * TAU;
            let radial = Vec3::new(a.cos(), 0., a.sin());
            let v = g.vertex(
                shaft_center(y) + radial * shaft_radius(t),
                Vec2::new(a.cos(), a.sin()) * 0.45 + Vec2::splat(0.5),
                Vec3::splat(0.65),
            );
            if i > 0 {
                if sign > 0. {
                    g.triangle(c, v, v - 1);
                } else {
                    g.triangle(c, v - 1, v);
                }
            }
        }
    }
    g.finish_normals();
    g.mesh()
}

// Long, bent fibres, pores, splits and elliptical knots; no external texture assets.
fn wood_sample(u: f32, v: f32) -> (Vec3, f32, f32) {
    let bend = 0.006 * (v * 19.).sin() + 0.003 * (v * 51. + u * TAU * 3.).sin();
    let mut x = u + bend;
    let mut knot = 0.;
    for (ku, kv) in [(0.23, 0.36), (0.69, 0.73), (0.86, 0.16)] {
        let dx = (u - ku + 0.5).rem_euclid(1.) - 0.5;
        let dy = v - kv;
        let r = ((dx * 23.).powi(2) + (dy * 11.).powi(2)).sqrt();
        let envelope = (-r * r * 1.3).exp();
        x += dx.signum() * 0.026 * envelope;
        knot += envelope * (0.5 + 0.5 * (r * 24.).sin());
    }
    let grain = (x * TAU * 78. + (v * 11.).sin()).sin();
    let fine = (x * TAU * 241. + (v * 83.).sin() * 0.7).sin();
    let pores = (grain * 0.5 + 0.5).powi(18);
    let splits = ((x * TAU * 29.).sin() * 0.5 + 0.5).powi(70)
        * (0.5 + 0.5 * (v * 41. + u * 25.).sin()).powi(2);
    let fleck = ((u * 18237. + v * 27183.).sin() * 1527.31).fract().abs();
    let wear = 0.5 + 0.5 * (u * TAU * 5. + v * 8.).sin();
    let char = ((v - 0.78) / 0.2).clamp(0., 1.).powi(2);
    let shade = (0.77 + grain * 0.12 + fine * 0.05 + fleck * 0.09
        - pores * 0.22
        - splits * 0.52
        - knot * 0.27)
        .max(0.10);
    let rgb = Vec3::new(0.34, 0.205, 0.10).lerp(Vec3::new(0.51, 0.405, 0.285), wear * 0.65)
        * shade
        * (1. - char * 0.84);
    (
        rgb,
        grain * 0.10 + fine * 0.025 - pores * 0.23 - splits * 0.55 - knot * 0.13,
        0.73 + pores * 0.15 + char * 0.10,
    )
}

fn wood_material(images: &mut Assets<Image>) -> StandardMaterial {
    const W: usize = 1024;
    const H: usize = 2048;
    let samples: Vec<_> = (0..W * H)
        .map(|i| wood_sample((i % W) as f32 / W as f32, (i / W) as f32 / (H - 1) as f32))
        .collect();
    let mut color = Vec::with_capacity(W * H * 4);
    let mut normal = Vec::with_capacity(W * H * 4);
    let mut rough = Vec::with_capacity(W * H * 4);
    for y in 0..H {
        for x in 0..W {
            let (rgb, h, r) = samples[y * W + x];
            let dx = samples[y * W + (x + 1) % W].1 - h;
            let dy = samples[((y + 1).min(H - 1)) * W + x].1 - h;
            let n = Vec3::new(-dx * 2.4, -dy * 2.4, 1.).normalize() * 0.5 + Vec3::splat(0.5);
            color.extend([rgb.x, rgb.y, rgb.z, 1.].map(|v| (v * 255.) as u8));
            normal.extend([n.x, n.y, n.z, 1.].map(|v| (v * 255.) as u8));
            rough.extend([255, (r.min(1.) * 255.) as u8, 0, 255]);
        }
    }
    use crate::rendering::mipmaps::{self, Filter};
    let mut add = |data, filter| {
        let mut image = Image::new(
            Extent3d {
                width: W as u32,
                height: H as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            if matches!(filter, Filter::Color) {
                TextureFormat::Rgba8UnormSrgb
            } else {
                TextureFormat::Rgba8Unorm
            },
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
        if !(std::env::var_os("HITHER_PROFILE_FOREST").is_some()
            && std::env::var_os("HITHER_PROFILE_NO_ORC_MIPS").is_some())
        {
            mipmaps::generate(&mut image, filter);
        }
        images.add(image)
    };
    StandardMaterial {
        base_color_texture: Some(add(color, Filter::Color)),
        normal_map_texture: Some(add(normal, Filter::Normal)),
        metallic_roughness_texture: Some(add(rough, Filter::Linear)),
        perceptual_roughness: 1.,
        reflectance: 0.22,
        ..default()
    }
}

// Revolved cross section gives each band real thickness and rolled, catching edges.
fn band(g: &mut Geometry, center: Vec3, r: f32, height: f32) {
    let profile = [
        (r - 0.005, -height * 0.5),
        (r + 0.003, -height * 0.5),
        (r + 0.007, -height * 0.5 + 0.005),
        (r + 0.007, height * 0.5 - 0.005),
        (r + 0.003, height * 0.5),
        (r - 0.005, height * 0.5),
        (r - 0.005, -height * 0.5),
    ];
    let base = g.positions.len() as u32;
    for (j, &(radius, y)) in profile.iter().enumerate() {
        for i in 0..=64 {
            let a = i as f32 / 64. * TAU;
            let hammered = 1. + 0.006 * (a * 17.).sin();
            g.vertex(
                center + Vec3::new(a.cos() * radius * hammered, y, a.sin() * radius * hammered),
                Vec2::new(i as f32 / 64. * 3., j as f32 / 6.),
                Vec3::ONE,
            );
            if j < profile.len() - 1 && i < 64 {
                let a = base + (j * 65 + i) as u32;
                g.triangle(a, a + 65, a + 1);
                g.triangle(a + 1, a + 65, a + 66);
            }
        }
    }
    for i in 0..8 {
        let a = i as f32 / 8. * TAU;
        add_shape(
            g,
            Sphere::new(0.009).mesh().uv(12, 8),
            Transform::from_translation(center + Vec3::new(a.cos(), 0., a.sin()) * (r + 0.006))
                .with_scale(Vec3::new(1., 0.85, 1.)),
            Vec3::splat(0.72),
        );
    }
}

pub(super) fn build(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    flames: &mut Assets<FlameMaterial>,
) -> TorchArt {
    let wood = materials.add(wood_material(images));
    let mut iron_mat = den_material(true, images);
    iron_mat.base_color = Color::srgb(0.7, 0.74, 0.78);
    iron_mat.perceptual_roughness = 0.69;
    let iron_mat = materials.add(iron_mat);
    let mut iron = Geometry::default();
    for (y, height) in [(-0.90, 0.055), (-0.52, 0.07), (-0.23, 0.065)] {
        band(
            &mut iron,
            shaft_center(y),
            shaft_radius((y + 1.02) / 0.91) + 0.004,
            height,
        );
    }
    // Blacksmith's wall plate, two anchors, curved support and fuel basket.
    add_shape(
        &mut iron,
        Cuboid::new(0.028, 0.35, 0.115).mesh().build(),
        Transform::from_xyz(0.30, -0.52, 0.),
        Vec3::splat(0.62),
    );
    for y in [-0.65, -0.39] {
        // Long anchor pins bed into the earthen wall in unshored Rootwarrens.
        beam(
            &mut iron,
            Vec3::new(0.30, y, 0.),
            Vec3::new(0.72, y, 0.),
            0.013,
            Vec3::splat(0.65),
        );
        add_shape(
            &mut iron,
            Sphere::new(0.018).mesh().uv(16, 8),
            Transform::from_xyz(0.275, y, 0.).with_scale(Vec3::new(0.4, 1., 1.)),
            Vec3::splat(0.65),
        );
    }
    beam(
        &mut iron,
        Vec3::new(0.28, -0.60, 0.),
        shaft_center(-0.52),
        0.023,
        Vec3::ONE,
    );
    beam(
        &mut iron,
        Vec3::new(0.28, -0.65, 0.),
        shaft_center(-0.77),
        0.018,
        Vec3::ONE,
    );
    band(&mut iron, Vec3::new(0., -0.065, 0.), 0.128, 0.046);
    band(&mut iron, Vec3::new(0.02, -0.235, 0.), 0.092, 0.035);
    for i in 0..8 {
        let a = i as f32 * TAU / 8.;
        let r = Vec3::new(a.cos(), 0., a.sin());
        beam(
            &mut iron,
            Vec3::new(0.02, -0.23, 0.) + r * 0.09,
            Vec3::new(0., -0.04, 0.) + r * 0.126,
            0.008,
            Vec3::splat(0.7),
        );
    }
    iron.finish_normals();
    let mut fuel = Geometry::default();
    add_shape(
        &mut fuel,
        Sphere::new(1.).mesh().uv(48, 24),
        Transform::from_xyz(0.006, -0.115, 0.).with_scale(Vec3::new(0.118, 0.16, 0.115)),
        Vec3::ONE,
    );
    // Individual overlapping charred binding cords, including frayed fibres.
    for i in 0..560 {
        let t = i as f32 / 560.;
        let a = t * TAU * 11.;
        let p = Vec3::new(
            a.cos() * (0.091 + t * 0.025),
            -0.235 + t * 0.245,
            a.sin() * (0.091 + t * 0.025),
        );
        let b = a + TAU * 11. / 560.;
        let q = Vec3::new(
            b.cos() * (0.091 + t * 0.025),
            p.y + 0.245 / 560.,
            b.sin() * (0.091 + t * 0.025),
        );
        beam(
            &mut fuel,
            p,
            q,
            0.011,
            Vec3::splat(0.6 + 0.3 * (a * 4.).sin().abs()),
        );
    }
    let mut fuel_mat = den_material(false, images);
    fuel_mat.base_color = Color::srgb(0.11, 0.085, 0.065);
    let fuel_mat = materials.add(fuel_mat);
    let mut coals = Geometry::default();
    for i in 0..42 {
        let a = i as f32 * 2.39996;
        let r = 0.112 * (0.2 + 0.8 * (i as f32 * 1.77).sin().abs());
        add_shape(
            &mut coals,
            Sphere::new(1.).mesh().ico(1).unwrap(),
            Transform::from_xyz(a.cos() * r, -0.015 + 0.014 * a.sin(), a.sin() * r)
                .with_scale(Vec3::new(0.008, 0.003, 0.013)),
            Vec3::new(1., 0.35 + r * 2., 0.04),
        );
    }
    let ember_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1., 0.17, 0.015),
        emissive: LinearRgba::new(6., 0.45, 0.015, 1.),
        unlit: true,
        ..default()
    });
    // Baked local bounds: shader and CPU lobe positions use these same metres.
    let mut volume = Geometry::default();
    add_shape(
        &mut volume,
        Cuboid::new(0.9, 1.36, 0.9).mesh().build(),
        Transform::from_xyz(0., 0.60, 0.),
        Vec3::ONE,
    );
    TorchArt {
        parts: vec![
            (meshes.add(wood_mesh()), wood),
            (meshes.add(iron.mesh()), iron_mat),
            (meshes.add(fuel.mesh()), fuel_mat),
            (meshes.add(coals.mesh()), ember_mat),
        ],
        volume: meshes.add(volume.mesh()),
        fire: flames.add(FlameMaterial { motion: Vec4::ZERO }),
    }
}

pub(super) fn spawn(
    parent: &mut ChildSpawnerCommands,
    art: &TorchArt,
    local_origin: Vec3,
    den_origin: Vec3,
) {
    let world_origin = den_origin + local_origin;
    let phase = (world_origin.x * 0.73 + world_origin.z * 0.37).rem_euclid(TAU);
    parent
        .spawn((
            Name::new("Lower room / weathered iron-bound wooden torch"),
            Transform::from_translation(local_origin),
            Visibility::default(),
        ))
        .with_children(|torch| {
            for (mesh, material) in &art.parts {
                torch.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                ));
            }
            torch.spawn((
                Name::new("Volumetric blue-rooted flame / golden wisps and embers"),
                Mesh3d(art.volume.clone()),
                MeshMaterial3d(art.fire.clone()),
                Transform::default(),
                NotShadowCaster,
            ));
            torch.spawn((
                FireLight { phase },
                crate::rendering::lighting::LightEmitter {
                    range: 11.5,
                    radius: 0.13,
                    priority: 1.5,
                    ..crate::rendering::lighting::LightEmitter::new(0.0)
                },
                Transform::default(),
            ));
        });
}

// Keep these analytic lobe paths in sync with torch.wgsl. The light samples
// their luminous centroids, not an unrelated random flicker at the bracket.
fn wisp(t: f32, i: f32, h: f32) -> (Vec3, f32) {
    let phase = t + i * 1.71;
    let height = 0.56 + 0.15 * (phase * 2.7).sin() + 0.08 * (phase * 6.1).sin();
    let root = Vec2::new((i * 2.39996).cos(), (i * 2.39996).sin()) * (0.025 + i * 0.009);
    let bend = Vec2::new((phase * 3.1 - h * 5.).sin(), (phase * 2.3 - h * 6.).cos())
        * (0.022 + 0.12 * h * h);
    (
        Vec3::new(root.x + bend.x, h * height, root.y + bend.y),
        height,
    )
}
fn animate_fire(
    time: Res<Time>,
    game: Res<crate::app::GameState>,
    mut clock: Local<f32>,
    mut materials: ResMut<Assets<FlameMaterial>>,
    mut lights: Query<(
        &FireLight,
        &mut crate::rendering::lighting::LightEmitter,
        &mut Transform,
    )>,
) {
    if !game.paused {
        *clock += time.delta_secs();
    }
    for (_, material) in materials.iter_mut() {
        material.motion.x = *clock;
    }
    for (fire, mut light, mut transform) in &mut lights {
        let t = *clock + fire.phase;
        let mut center = Vec3::ZERO;
        let mut energy = 0.;
        for i in 0..5 {
            let (p, weight) = wisp(t, i as f32, 0.28);
            center += p * weight;
            energy += weight;
        }
        let strength = energy / 2.8;
        transform.translation = center / energy;
        light.lumens = 72_000. * strength;
        light.color = [1., 0.64 + 0.045 * (t * 2.7).sin(), 0.22];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fire_and_its_lights_move_together_and_freeze_when_paused() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<crate::app::GameState>()
            .init_resource::<crate::player::camera::CameraRig>()
            .init_resource::<Assets<FlameMaterial>>()
            .add_systems(Update, animate_fire);
        let material = app
            .world_mut()
            .resource_mut::<Assets<FlameMaterial>>()
            .add(FlameMaterial { motion: Vec4::ZERO });
        let entity = app
            .world_mut()
            .spawn((
                FireLight { phase: 1.3 },
                crate::rendering::lighting::LightEmitter::new(0.0),
                Transform::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(16));
        app.update();
        let first = *app.world().get::<Transform>(entity).unwrap();
        let first_energy = app
            .world()
            .get::<crate::rendering::lighting::LightEmitter>(entity)
            .unwrap()
            .lumens;
        assert!(first_energy > 20_000.);
        app.update();
        let moving = *app.world().get::<Transform>(entity).unwrap();
        assert_ne!(first.translation, moving.translation);
        let energy = app
            .world()
            .get::<crate::rendering::lighting::LightEmitter>(entity)
            .unwrap()
            .lumens;
        assert_ne!(first_energy, energy);
        let clock = app
            .world()
            .resource::<Assets<FlameMaterial>>()
            .get(&material)
            .unwrap()
            .motion;
        app.world_mut()
            .resource_mut::<crate::app::GameState>()
            .paused = true;
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(moving, *app.world().get::<Transform>(entity).unwrap());
        assert_eq!(
            energy,
            app.world()
                .get::<crate::rendering::lighting::LightEmitter>(entity)
                .unwrap()
                .lumens
        );
        assert_eq!(
            clock,
            app.world()
                .resource::<Assets<FlameMaterial>>()
                .get(&material)
                .unwrap()
                .motion
        );
    }

    #[test]
    fn wisp_paths_and_energy_stay_inside_the_volume() {
        for frame in 0..2000 {
            let t = frame as f32 * 0.017;
            let mut energy = 0.;
            for i in 0..5 {
                for h in [0., 0.28, 0.70, 1.] {
                    let (p, height) = wisp(t, i as f32, h);
                    assert!(p.is_finite() && p.x.abs() < 0.3 && p.z.abs() < 0.3);
                    assert!((0.32..0.80).contains(&height));
                    if h == 0.28 {
                        energy += height;
                    }
                }
            }
            assert!((1.6..4.).contains(&energy));
        }
    }
}
