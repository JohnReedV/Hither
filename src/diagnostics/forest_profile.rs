//! Opt-in, fixed-seed native frame-time benchmark. No systems in normal play.
use bevy::{app::AppExit, prelude::*};
use std::time::Instant;

pub struct ForestProfilePlugin;
impl Plugin for ForestProfilePlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("HITHER_PROFILE_FOREST").is_some() {
            app.init_resource::<Profile>().add_systems(
                Update,
                sample
                    .after(crate::player::movement::move_camera)
                    .before(crate::player::camera::update_camera_view),
            );
            if std::env::var_os("HITHER_PROFILE_OCCLUDER").is_some() {
                app.add_systems(PostStartup, benchmark_occluder);
            }
            if std::env::var_os("HITHER_PROFILE_OFFSCREEN").is_some() {
                app.add_systems(PostStartup, offscreen_target);
            }
        }
    }
}
#[derive(Resource, Default)]
struct Profile {
    start: Option<(Instant, Vec3, f32, f32)>,
    stationary: Vec<f64>,
    moving: Vec<f64>,
    diagnostics_applied: bool,
    direction: Vec3,
    texture_cycle: bool,
    timings: std::collections::BTreeMap<String, Vec<f64>>,
    last_diagnostic: Option<Instant>,
}

fn offscreen_target(
    mut commands: Commands,
    cameras: Query<
        Entity,
        (
            With<Camera2d>,
            Without<crate::rendering::graphics::WorldPass>,
        ),
    >,
    mut images: ResMut<Assets<Image>>,
) {
    let target = images.add(Image::new_target_texture(
        1280,
        720,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(bevy::camera::RenderTarget::Image(target.clone().into()));
    }
}
#[allow(clippy::too_many_arguments)] // Bevy system parameters, not a call-site API.
fn sample(
    mut profile: ResMut<Profile>,
    time: Res<Time<Real>>,
    mut rig: ResMut<crate::player::camera::CameraRig>,
    meshes: Res<Assets<Mesh>>,
    instances: Query<(&Mesh3d, &ViewVisibility)>,
    mut exit: MessageWriter<AppExit>,
    mut lights: Query<&mut DirectionalLight>,
    mut visibility: Query<(&Name, &mut Visibility)>,
    windows: Query<&Window>,
    world_cameras: Query<&Camera, With<crate::rendering::graphics::WorldPass>>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut settings: ResMut<crate::app::settings::GraphicsSettings>,
) {
    let spectator = std::env::var_os("HITHER_PROFILE_SPECTATOR").is_some();
    let warmup = std::env::var("HITHER_PROFILE_WARMUP")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v >= 0.)
        .unwrap_or(if spectator { 20.0 } else { 4.0 });
    let flight_seconds = if spectator { 16.0 } else { 8.0 };
    let speed = std::env::var("HITHER_PROFILE_SPEED")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(if spectator { 40.0 } else { 4.0 });
    if spectator && profile.start.is_none() {
        rig.toggle_spectator();
        rig.position.y = std::env::var("HITHER_PROFILE_HEIGHT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8.0);
        rig.yaw = -std::f32::consts::FRAC_PI_2;
        rig.pitch = -0.20;
    }
    if profile.start.is_none() {
        profile.direction = std::env::var("HITHER_PROFILE_DIRECTION").map_or(Vec3::X, |s| {
            let values: Vec<f32> = s
                .split(',')
                .map(|v| v.parse().expect("profile direction x,y,z"))
                .collect();
            assert!(values.len() == 3 && values.iter().all(|v| v.is_finite()));
            Vec3::new(values[0], values[1], values[2]).normalize_or_zero()
        });
        profile.texture_cycle = std::env::var_os("HITHER_PROFILE_TEXTURE_CYCLE").is_some();
        if let Ok(mode) = std::env::var("HITHER_PROFILE_CAMERA") {
            use crate::player::camera::CameraMode;
            rig.camera_mode = match mode.as_str() {
                "third" => CameraMode::ThirdPerson,
                "second" => CameraMode::SecondPerson,
                "first" => CameraMode::FirstPerson,
                _ => panic!("profile camera must be first, second or third"),
            };
        }
        // Reproduce a wall, doorway, or above-wall view without desktop input.
        if let Ok(pose) = std::env::var("HITHER_PROFILE_POSE") {
            let values: Vec<f32> = pose
                .split(',')
                .map(|v| {
                    v.trim()
                        .parse()
                        .expect("HITHER_PROFILE_POSE must be x,y,z,yaw,pitch in metres/radians")
                })
                .collect();
            assert!(values.len() == 5 && values.iter().all(|v| v.is_finite()));
            rig.position = Vec3::new(values[0], values[1], values[2]);
            rig.yaw = values[3];
            rig.pitch = values[4];
        }
    }
    let (start, origin, yaw, pitch) =
        *profile
            .start
            .get_or_insert((Instant::now(), rig.position, rig.yaw, rig.pitch));
    let elapsed = start.elapsed().as_secs_f32();
    // Real desktop pointer movement must not change the benchmark's view.
    rig.yaw = yaw;
    rig.pitch = pitch;
    rig.position = origin;
    if elapsed > 3.0 && !profile.diagnostics_applied {
        profile.diagnostics_applied = true;
        if std::env::var_os("HITHER_PROFILE_NO_SHADOWS").is_some() {
            for mut light in &mut lights {
                light.shadow_maps_enabled = false;
            }
        }
        for (name, mut visible) in &mut visibility {
            if (std::env::var_os("HITHER_PROFILE_NO_GRASS").is_some()
                && name.as_str() == "Grass / curved blades")
                || (std::env::var_os("HITHER_PROFILE_NO_TREES").is_some()
                    && matches!(name.as_str(), "Wild orange tree" | "Snow-laden spruce"))
            {
                *visible = Visibility::Hidden;
            }
        }
    }
    if elapsed < warmup {
        return;
    }
    if profile.texture_cycle {
        use crate::rendering::graphics::Quality;
        let quality = match ((elapsed - warmup) / 2.) as usize % 3 {
            0 => Quality::Low,
            1 => Quality::Medium,
            _ => Quality::High,
        };
        if settings.texture_quality != quality {
            settings.texture_quality = quality;
        }
    }
    let ms = time.delta_secs_f64() * 1000.0;
    if elapsed < warmup + 8.0 {
        profile.stationary.push(ms);
    } else if elapsed < warmup + 8.0 + flight_seconds {
        profile.moving.push(ms);
        // Reproducible 4 m/s traverse; bypass collision so every build takes the
        // same route, including streaming boundaries, even near trunks.
        rig.position = origin + profile.direction * (elapsed - warmup - 8.0) * speed;
        if let Ok(turn) = std::env::var("HITHER_PROFILE_TURN") {
            rig.yaw = yaw
                + (elapsed - warmup - 8.0)
                    * turn
                        .parse::<f32>()
                        .expect("HITHER_PROFILE_TURN must be radians/second");
        }
        let latest = diagnostics
            .iter()
            .filter(|d| d.path().to_string().starts_with("render/"))
            .filter_map(|d| d.measurement().map(|m| m.time))
            .max();
        if latest != profile.last_diagnostic {
            profile.last_diagnostic = latest;
            for diagnostic in diagnostics.iter() {
                // Several camera passes share a diagnostic path. Sum every
                // measurement from this GPU report, not just the last camera.
                let values: Vec<_> = diagnostic
                    .measurements()
                    .filter(|m| Some(m.time) == latest)
                    .collect();
                if !values.is_empty() {
                    profile
                        .timings
                        .entry(diagnostic.path().to_string())
                        .or_default()
                        .push(values.iter().map(|m| m.value).sum());
                }
            }
        }
    } else {
        println!("VISIBILITY mode={:?}", crate::rendering::occlusion::mode());
        println!("PROFILE yaw={yaw:.4} pitch={pitch:.4}");
        println!("PROFILE spectator={spectator} speed={speed} origin={origin:?}");
        println!(
            "PROFILE settings: resolution_preset={} render_distance={} detail_distance={}",
            settings.resolution, settings.render_distance, settings.detail_distance
        );
        for window in &windows {
            println!(
                "PROFILE physical_resolution={}x{}",
                window.physical_width(),
                window.physical_height()
            );
        }
        for camera in &world_cameras {
            if let Some(size) = camera.physical_viewport_size() {
                println!("PROFILE world_target={}x{}", size.x, size.y);
            }
        }
        let profile = &mut *profile;
        for (label, values) in [
            ("stationary", &mut profile.stationary),
            ("moving", &mut profile.moving),
        ] {
            values.sort_by(f64::total_cmp);
            if !values.is_empty() {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                println!(
                    "PROFILE {label}: fps={:.1} median_ms={:.2} p95_ms={:.2} p99_ms={:.2} p999_ms={:.2} max_ms={:.2}",
                    1000.0 / mean,
                    values[values.len() / 2],
                    values[values.len() * 95 / 100],
                    values[values.len() * 99 / 100],
                    values[values.len() * 999 / 1000],
                    values[values.len() - 1]
                );
            }
        }
        for (path, values) in &mut profile.timings {
            if !values.is_empty()
                && (path.ends_with("elapsed_gpu") || path.ends_with("elapsed_cpu"))
            {
                values.sort_by(f64::total_cmp);
                println!(
                    "TIMING {path}: median={:.3} p95={:.3} p99={:.3} max={:.3}",
                    values[values.len() / 2],
                    values[values.len() * 95 / 100],
                    values[values.len() * 99 / 100],
                    values[values.len() - 1]
                );
            }
        }
        let mut triangles = 0;
        let mut visible = 0;
        for (handle, visibility) in &instances {
            if visibility.get() {
                visible += 1;
                if let Some(mesh) = meshes.get(&handle.0)
                    && mesh
                        .asset_usage
                        .contains(bevy::asset::RenderAssetUsages::MAIN_WORLD)
                {
                    triangles += mesh.try_indices().map_or_else(
                        |_| {
                            mesh.try_attribute(Mesh::ATTRIBUTE_POSITION)
                                .map_or(0, |v| v.len())
                        },
                        |i| i.len(),
                    ) / 3;
                }
            }
        }
        println!(
            "PROFILE instances={} visible={} visible_triangles={triangles}",
            instances.iter().count(),
            visible
        );
        exit.write(AppExit::Success);
    }
}

// An ordinary opaque mesh in front of a forest tests general surface occlusion,
// independently of the castle, collision geometry, or room membership.
fn benchmark_occluder(
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let view = rig.view();
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(14., 10., 0.3))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.35, 0.4),
            ..default()
        })),
        Transform::from_translation(view.position + view.forward * 3.)
            .looking_to(view.forward, view.up),
    ));
}
