//! Mirelings: small reference-inspired goblins with dedicated skinned animation.
use super::{
    navigation::{self as nav, Agent},
    orcs::navigation::World,
};
use crate::world::streaming::{Coordinator, Layer};
use crate::{
    app::GameState,
    player::{camera::CameraRig, movement::PLAYER_EYE_HEIGHT},
};
use bevy::{gltf::GltfMesh, prelude::*, world_serialization::WorldInstanceReady};
use std::{collections::HashMap, time::Duration};

// Bundled avatar crown-to-sole bound is 1.5173314m (including hair/shoes),
// scaled by its 1.43m eye height.
// Goblin crown is 1.024m before scaling. Compare crown-to-sole, not eye heights.
pub(crate) const HEIGHT: f32 = (1.517_331_4 * PLAYER_EYE_HEIGHT / 1.43) / 3.;
const SCALE: f32 = HEIGHT / 1.024;
const AGENT: Agent = Agent {
    radius: 0.12,
    height: HEIGHT,
    step: 0.09,
    slope: 0.8,
};
const CLIPS: [&str; 4] = ["Idle", "Scamper", "Alert", "Swipe"];

pub(crate) struct GoblinPlugin;
impl Plugin for GoblinPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("HITHER_PROFILE_FOREST").is_some() {
            app.add_systems(
                PostUpdate,
                profile_isolation.before(TransformSystems::Propagate),
            );
        }
        if std::env::var_os("HITHER_PROFILE_NO_POINT_SHADOWS").is_some()
            && let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp)
        {
            render.add_systems(
                bevy::render::Render,
                profile_point_shadows
                    .after(bevy::render::RenderSystems::ExtractCommands)
                    .before(bevy::render::RenderSystems::CreateViews),
            );
        }
        app.init_resource::<Population>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (load, stream.in_set(Layer::Goblins), behavior, animate)
                    .chain()
                    .after(crate::player::movement::move_camera),
            )
            .add_systems(
                PostUpdate,
                select_mesh_detail
                    .after(TransformSystems::Propagate)
                    .before(bevy::camera::visibility::VisibilitySystems::CalculateBounds)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
    }
}
#[derive(Resource)]
struct Art {
    source: Handle<Gltf>,
    gameplay: Handle<Gltf>,
    details: HashMap<AssetId<Mesh>, Vec<DetailAsset>>,
    scene: Option<Handle<WorldAsset>>,
    graph: Handle<AnimationGraph>,
    clips: Vec<AnimationNodeIndex>,
    stride_speed: f32,
    preview: bool,
}
#[derive(Resource, Default)]
struct Population {
    preview: Option<Entity>,
    dens: HashMap<IVec2, Vec<Entity>>,
}
#[derive(Component)]
struct Goblin {
    home: Vec3,
    age: f32,
    alert: f32,
    speed: f32,
    motion: Motion,
    clip: usize,
    velocity: f32,
    navigation: nav::Navigator,
    recovery: nav::Recovery,
    pose: Option<(Transform, Transform)>,
}
#[derive(Component)]
struct Animator {
    owner: Entity,
    clip: usize,
}
fn setup(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(Art {
        source: server.load("goblins/mireling.glb"),
        gameplay: server.load("goblins/mireling-gameplay.glb"),
        details: default(),
        preview: std::env::var_os("HITHER_GOBLIN_PREVIEW").is_some(),
        scene: None,
        graph: default(),
        clips: vec![],
        stride_speed: {
            let metadata: serde_json::Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/goblins/authoring.json"
            )))
            .expect("goblin authoring metadata");
            (metadata["stance_travel"].as_f64().unwrap()
                / (metadata["walk_cycle_seconds"].as_f64().unwrap()
                    * metadata["stance_fraction"].as_f64().unwrap())) as f32
                * SCALE
        },
    });
}
fn load(
    server: Res<AssetServer>,
    assets: Res<Assets<Gltf>>,
    meshes: Res<Assets<GltfMesh>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut art: ResMut<Art>,
) {
    if art.scene.is_some()
        || !server.is_loaded_with_dependencies(&art.source)
        || !server.is_loaded_with_dependencies(&art.gameplay)
    {
        return;
    }
    let Some(gltf) = assets.get(&art.source) else {
        return;
    };
    let Some(gameplay) = assets.get(&art.gameplay) else {
        return;
    };
    for (name, original) in &gltf.named_meshes {
        let Some(original) = meshes.get(original) else {
            return;
        };
        let mut levels = Vec::new();
        for level in 0..4 {
            let key = format!("lod{level}_{name}");
            let Some(handle) = gameplay.named_meshes.get(key.as_str()) else {
                return;
            };
            let Some(mesh) = meshes.get(handle) else {
                return;
            };
            let primitive = &mesh.primitives[0];
            levels.push(DetailAsset {
                mesh: primitive.mesh.clone(),
                material: {
                    let path = primitive
                        .material
                        .as_ref()
                        .expect("goblin LOD material")
                        .path()
                        .expect("goblin material asset path");
                    server.load(
                        path.clone()
                            .with_label(format!("{}/std", path.label().unwrap())),
                    )
                },
            });
        }
        art.details.insert(original.primitives[0].mesh.id(), levels);
    }
    let (graph, clips) =
        AnimationGraph::from_clips(CLIPS.map(|name| gltf.named_animations[name].clone()));
    art.graph = graphs.add(graph);
    art.clips = clips;
    art.scene = Some(gltf.scenes[0].clone());
}
fn stream(
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    game: Res<GameState>,
    rig: Res<CameraRig>,
    range: Option<Res<crate::rendering::view_distance::Range>>,
    art: Res<Art>,
    mut population: ResMut<Population>,
) {
    if game.paused {
        return;
    }
    let Some(scene) = &art.scene else {
        return;
    };
    let activation = range.as_ref().map_or(145.0, |r| (r.load + 68.0).min(145.0));
    population.dens.retain(|cell, entities| {
        let keep = super::goblin_dens::site(*cell)
            .is_some_and(|d| d.center.distance(rig.position) < activation + 35.);
        if !keep {
            for entity in entities {
                commands.entity(*entity).despawn();
            }
        }
        keep
    });
    let den_cell = (rig.position.xz() / super::goblin_dens::CELL)
        .floor()
        .as_ivec2();
    for x in -1..=1 {
        for z in -1..=1 {
            let cell = den_cell + IVec2::new(x, z);
            let Some(den) = super::goblin_dens::site(cell) else {
                continue;
            };
            if den.center.distance(rig.position) > activation || !den.ready() {
                continue;
            }
            if population
                .dens
                .get(&cell)
                .is_some_and(|residents| residents.len() == den.resident_count())
            {
                continue;
            }
            let mut residents = population.dens.remove(&cell).unwrap_or_default();
            for (i, home) in den
                .residents()
                .into_iter()
                .enumerate()
                .skip(residents.len())
            {
                let Some(_installation) = coordinator.install(Layer::Goblins, 1, 0) else {
                    break;
                };
                residents.push(
                    commands
                        .spawn((
                            Name::new(format!(
                                "Den goblin / house {} / resident {}",
                                i / 4 + 1,
                                i % 4 + 1
                            )),
                            Goblin {
                                home,
                                age: i as f32 * 1.37,
                                alert: 0.,
                                speed: 0.,
                                motion: default(),
                                clip: 0,
                                velocity: 0.,
                                navigation: nav::Navigator::layered_pursuit(),
                                recovery: default(),
                                pose: None,
                            },
                            WorldAssetRoot(scene.clone()),
                            Transform::from_translation(home).with_scale(Vec3::splat(SCALE)),
                        ))
                        .observe(ready)
                        .id(),
                );
            }
            population.dens.insert(cell, residents);
        }
    }

    // The only surface goblin is the explicitly requested model-preview subject.
    // Normal gameplay gets its population exclusively from the dens above.
    if !art.preview || population.preview.is_some() {
        return;
    }
    let p = Vec2::new(0., -4.);
    let home = Vec3::new(p.x, super::terrain::height(p), p.y);
    population.preview = Some(
        commands
            .spawn((
                Name::new("Goblin / Mireling preview"),
                Goblin {
                    home,
                    age: 0.,
                    alert: 0.,
                    speed: 0.,
                    motion: default(),
                    clip: 0,
                    velocity: 0.,
                    navigation: nav::Navigator::layered_pursuit(),
                    recovery: default(),
                    pose: None,
                },
                WorldAssetRoot(scene.clone()),
                Transform::from_translation(home).with_scale(Vec3::splat(SCALE)),
            ))
            .observe(ready)
            .id(),
    );
}
fn ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    art: Res<Art>,
    goblins: Query<&Goblin>,
    mut players: Query<&mut AnimationPlayer>,
    meshes: Query<(&Mesh3d, &bevy::mesh::skinning::SkinnedMesh)>,
) {
    let Ok(goblin) = goblins.get(event.entity) else {
        return;
    };
    for entity in children.iter_descendants(event.entity) {
        if let Ok((mesh, skin)) = meshes.get(entity)
            && let Some(levels) = art.details.get(&mesh.0.id())
            && !(art.preview && std::env::var_os("HITHER_GOBLIN_SCULPT").is_some())
        {
            // Layer 1 is the existing world shadow-only layer. Lights see 0+1;
            // gameplay and first-person cameras do not see this proxy.
            let shadow = &levels[3];
            let proxy = commands
                .spawn((
                    Name::new("Goblin shadow proxy"),
                    Mesh3d(shadow.mesh.clone()),
                    MeshMaterial3d(shadow.material.clone()),
                    skin.clone(),
                    bevy::camera::visibility::DynamicSkinnedMeshBounds,
                    Transform::IDENTITY,
                    bevy::camera::visibility::RenderLayers::layer(1),
                    ChildOf(entity),
                ))
                .id();
            commands.entity(entity).insert((
                Mesh3d(levels[0].mesh.clone()),
                MeshMaterial3d(levels[0].material.clone()),
                bevy::light::NotShadowCaster,
                MeshDetail {
                    levels: levels.clone(),
                    level: 0,
                    proxy,
                },
            ));
        }
        if let Ok(mut player) = players.get_mut(entity) {
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, art.clips[0], Duration::ZERO)
                .repeat()
                .seek_to(goblin.age);
            commands.entity(entity).insert((
                AnimationGraphHandle(art.graph.clone()),
                transitions,
                Animator {
                    owner: event.entity,
                    clip: 0,
                },
            ));
        }
    }
}
// All parts select together from projected crown height, with a 10% transition
// band. This adapts to FOV and world render resolution, including supersampling.
#[derive(Clone)]
struct DetailAsset {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}
#[derive(Component)]
struct MeshDetail {
    proxy: Entity,
    levels: Vec<DetailAsset>,
    level: usize,
}
fn detail_level(previous: usize, pixels: f32) -> usize {
    let thresholds = [600., 220., 80.];
    let mut level = previous;
    while level < 3 && pixels < thresholds[level] * 0.9 {
        level += 1;
    }
    while level > 0 && pixels > thresholds[level - 1] * 1.1 {
        level -= 1;
    }
    level
}
fn set_shadow_detail(commands: &mut Commands, visual: Entity, proxy: Entity, level: usize) {
    if level < 2 {
        commands.entity(visual).insert(bevy::light::NotShadowCaster);
        commands.entity(proxy).insert((
            Visibility::Inherited,
            bevy::camera::visibility::DynamicSkinnedMeshBounds,
        ));
    } else {
        // Distant geometry is already cheap. Share it between color and shadow
        // passes instead of extracting a duplicate skin and updating its bounds.
        commands
            .entity(visual)
            .remove::<bevy::light::NotShadowCaster>();
        commands
            .entity(proxy)
            .insert(Visibility::Hidden)
            .remove::<bevy::camera::visibility::DynamicSkinnedMeshBounds>();
    }
}
fn focal_pixels(projection: &Projection, height: u32) -> f32 {
    // The world uses a custom finite perspective projection. Read its matrix,
    // rather than borrowing the first-person hand camera's current FOV.
    projection.get_clip_from_view().y_axis.y.abs() * height as f32 * 0.5
}
fn select_mesh_detail(
    mut commands: Commands,
    views: Query<
        (&Camera, &Projection, &GlobalTransform),
        With<crate::player::avatar::PlayerCamera>,
    >,
    mut meshes: Query<(
        Entity,
        &GlobalTransform,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
        &mut MeshDetail,
    )>,
) {
    let Some((focal, origin)) = views.iter().find_map(|(camera, projection, transform)| {
        Some((
            focal_pixels(projection, camera.physical_viewport_size()?.y),
            transform.translation(),
        ))
    }) else {
        return;
    };
    for (entity, transform, mut mesh, mut material, mut detail) in &mut meshes {
        let distance = transform.translation().distance(origin).max(0.01);
        let level = detail_level(detail.level, HEIGHT * focal / distance);
        if level != detail.level {
            mesh.0 = detail.levels[level].mesh.clone();
            material.0 = detail.levels[level].material.clone();
            if (level < 2) != (detail.level < 2) {
                set_shadow_detail(&mut commands, entity, detail.proxy, level);
            }
            detail.level = level;
        }
    }
}
#[derive(Default)]
struct Motion {
    speed: f32,
    remainder: Vec2,
}
impl Motion {
    fn accelerate(&mut self, desired: f32, dt: f32) {
        self.speed += (desired - self.speed) * (1. - (-dt * 8.).exp());
    }
    fn destination(&mut self, from: Vec2, direction: Vec2, dt: f32) -> Vec2 {
        if direction == Vec2::ZERO {
            self.remainder = Vec2::ZERO;
            return from;
        }
        let delta = direction * self.speed * dt + self.remainder;
        let next = from + delta;
        // Only retain rounding error, never movement rejected by collision.
        self.remainder = delta - (next - from);
        next
    }
}
/// Thirty physics ticks per second, independent of rendering. Each walk still
/// uses the existing swept collision path, including stairs and ledges.
#[derive(Default)]
struct SimulationClock {
    remainder: f64,
}
impl SimulationClock {
    const STEP: f32 = 1. / 30.;
    fn advance(&mut self, dt: f32) -> usize {
        // Preserve the existing 50ms game-time clamp after a stalled frame.
        self.remainder += f64::from(dt.clamp(0., 0.05));
        let step = 1. / 30.;
        let ticks = (self.remainder / step).floor() as usize;
        self.remainder -= ticks as f64 * step;
        ticks
    }
    fn alpha(&self) -> f32 {
        (self.remainder * 30.) as f32
    }
}
#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects the resources, query, and persistent local state independently"
)]
fn behavior(
    time: Res<Time>,
    real_time: Res<Time<Real>>,
    game: Res<GameState>,
    rig: Res<CameraRig>,
    mut goblins: Query<(Entity, &mut Goblin, &mut Transform)>,
    mut graph: Local<nav::Graph>,
    mut first_actor: Local<usize>,
    mut clock: Local<SimulationClock>,
    mut profile: Local<crate::diagnostics::den_profile::ResidentProfile>,
) {
    if game.paused
        || (real_time.elapsed_secs() > 45.
            && std::env::var_os("HITHER_PROFILE_FREEZE_GOBLINS").is_some())
    {
        return;
    }
    profile.begin(real_time.elapsed_secs());
    let ticks = clock.advance(time.delta_secs());
    // Rendering interpolates between authoritative poses. Collision and crowd
    // queries always see simulation positions, never interpolated transforms.
    for (_, mut g, mut t) in &mut goblins {
        let (_, current) = *g.pose.get_or_insert((*t, *t));
        *t = current;
    }
    for _ in 0..ticks {
        for (_, mut g, t) in &mut goblins {
            g.pose = Some((*t, *t));
        }
        // Exact query reuse lasts only one simulation tick. Moving geometry
        // and subsequent ticks must revalidate their supporting surface.
        let _queries = super::orcs::navigation::FrameQueries::begin();
        let dt = SimulationClock::STEP;
        let mut planning_time = Duration::from_millis(1);
        let mut budget = 180;
        let mut recovery_budget = 64;
        let mut positions: Vec<_> = goblins.iter().map(|(e, _, t)| (e, t.translation)).collect();
        if positions.is_empty() {
            return;
        }
        let first = *first_actor % positions.len();
        *first_actor = (first + 1) % positions.len();
        // Rotate access to the shared planning slice, as orcs already do. Physics
        // does not consume another resident's navigation allowance.
        for offset in 0..positions.len() {
            let entity = positions[(first + offset) % positions.len()].0;
            let Ok((_, mut g, mut t)) = goblins.get_mut(entity) else {
                continue;
            };
            g.age += dt;
            let distance = t.translation.xz().distance(rig.position.xz());
            let sees = !rig.is_spectating()
                && distance < 8.
                && (rig.position.y - PLAYER_EYE_HEIGHT - t.translation.y).abs() < 1.;
            g.alert = if sees { g.alert + dt } else { 0. };
            let watching = sees && (g.alert < 1.1 || distance < 0.75);
            let target = if sees {
                rig.position.with_y(rig.position.y - PLAYER_EYE_HEIGHT)
            } else {
                g.home + Vec3::new((g.age * 0.24).sin(), 0., (g.age * 0.19).cos()) * 2.4
            };
            let pause = !sees && g.age % 9. < 3.;
            let desired = if watching || pause {
                0.
            } else if sees {
                0.32
            } else {
                0.18
            };
            g.motion.accelerate(desired, dt);
            let from = t.translation;
            let mut velocity = g.velocity;
            let measurement = profile.start();
            let grounded = if profile.legacy_recovery {
                nav::settle(&World, AGENT, &mut t.translation, &mut velocity, dt)
            } else {
                let mut attempts = recovery_budget.min(8);
                let granted = attempts;
                let grounded = nav::settle_with_recovery(
                    &World,
                    AGENT,
                    &mut t.translation,
                    &mut velocity,
                    dt,
                    &mut g.recovery,
                    &mut attempts,
                );
                recovery_budget -= granted - attempts;
                grounded
            };
            profile.finish(0, measurement);
            profile.supported(grounded);
            g.velocity = velocity;
            let next =
                if (grounded || profile.legacy_recovery) && g.motion.speed > 0.01 && !watching {
                    graph.begin_slice(planning_time);
                    let started = std::time::Instant::now();
                    let measurement = profile.start();
                    let next = g.navigation.steer(
                        &mut graph,
                        &World,
                        AGENT,
                        t.translation,
                        target,
                        dt,
                        &mut budget,
                    );
                    profile.finish(1, measurement);
                    planning_time = planning_time.saturating_sub(started.elapsed());
                    next
                } else {
                    None
                };
            let mut planned_direction = Vec3::ZERO;
            if let Some(next) = next {
                let direction = (next - t.translation).with_y(0.).normalize_or_zero();
                planned_direction = direction;
                let destination = g.motion.destination(t.translation.xz(), direction.xz(), dt);
                let measurement = profile.start();
                let accepted = nav::slide(&World, AGENT, t.translation, destination, |p| {
                    positions.iter().all(|(e, other)| {
                        *e == entity || p.distance_squared(*other) > 0.26_f32.powi(2)
                    })
                });
                profile.finish(2, measurement);
                if let Some(p) = accepted {
                    t.translation = p;
                }
            } else {
                g.motion.remainder = Vec2::ZERO;
            }
            let movement = (t.translation - from).with_y(0.);
            let actual = movement.length() / dt.max(0.001);
            // Filter accepted speed over a short interval so millimetre-sized
            // representable steps do not alternate Idle/Scamper at high frame rates.
            g.speed += (actual - g.speed) * (1. - (-dt * 20.).exp());
            let facing = if sees {
                (target - t.translation).with_y(0.)
            } else if g.speed > 0.015 {
                planned_direction
            } else {
                movement
            };
            if facing.length_squared() > 0.0 {
                let rotation = Quat::from_rotation_y(facing.x.atan2(facing.z));
                t.rotation = t.rotation.slerp(rotation, 1. - (-dt * 9.).exp());
            }
            // Reserve accepted positions immediately, as orc crowd movement
            // does. Frame-start positions let later movers step into the same
            // space, then repeatedly replan against a preventable crowd jam.
            let reservation = (first + offset) % positions.len();
            positions[reservation].1 = t.translation;
            g.clip = if watching && g.alert < 1.1 {
                2
            } else if sees && distance < 0.85 {
                3
            } else if g.speed > 0.015 {
                1
            } else {
                0
            };
        }
        for (_, mut g, t) in &mut goblins {
            let previous = g.pose.unwrap().0;
            g.pose = Some((previous, *t));
        }
    }
    let alpha = clock.alpha();
    for (_, g, mut t) in &mut goblins {
        let (previous, current) = g.pose.unwrap();
        t.translation = previous.translation.lerp(current.translation, alpha);
        t.rotation = previous.rotation.slerp(current.rotation, alpha);
    }
}

fn animate(
    game: Res<GameState>,
    art: Res<Art>,
    goblins: Query<&Goblin>,
    mut players: Query<(
        &mut Animator,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
) {
    for (mut animator, mut player, mut transitions) in &mut players {
        let Ok(g) = goblins.get(animator.owner) else {
            continue;
        };
        if g.clip != animator.clip {
            transitions
                .play(&mut player, art.clips[g.clip], Duration::from_millis(150))
                .repeat();
            animator.clip = g.clip;
        }
        for (_, animation) in player.playing_animations_mut() {
            if game.paused {
                animation.pause();
            } else {
                animation.resume();
            }
        }
        if let Some(animation) = player.animation_mut(art.clips[animator.clip]) {
            // Authored planted-foot travel and cycle duration, including model scale.
            animation.set_speed(if animator.clip == 1 {
                (g.speed / art.stride_speed).clamp(0.1, 3.)
            } else {
                1.
            });
        }
    }
}

// Controlled, opt-in den attribution. Apply continuously so newly streamed
// actors also obey the visibility intervention.
fn profile_isolation(time: Res<Time<Real>>, mut goblins: Query<&mut Visibility, With<Goblin>>) {
    if time.elapsed_secs() < 45. {
        return;
    }
    if std::env::var_os("HITHER_PROFILE_HIDE_GOBLINS").is_some() {
        for mut visibility in &mut goblins {
            *visibility = Visibility::Hidden;
        }
    }
}

// Mutate only the render-world copy. Main-world shadow ownership affects lamp
// priority and fading, so changing it would confound this comparison.
fn profile_point_shadows(
    mut start: Local<Option<std::time::Instant>>,
    mut reported: Local<bool>,
    mut lights: Query<&mut bevy::pbr::ExtractedPointLight>,
) {
    if start
        .get_or_insert_with(std::time::Instant::now)
        .elapsed()
        .as_secs_f32()
        < 45.
    {
        return;
    }
    for mut light in &mut lights {
        if light.shadow_maps_enabled && light.spot_light_angles.is_none() {
            light.shadow_maps_enabled = false;
        }
    }
    if !*reported {
        println!("PROFILE point shadows disabled in render world; lamp ownership unchanged");
        *reported = true;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distant_parts_share_geometry_and_close_parts_restore_the_shadow_proxy() {
        #[derive(Resource)]
        struct Selection {
            visual: Entity,
            proxy: Entity,
            level: usize,
        }
        let mut app = App::new();
        let visual = app.world_mut().spawn_empty().id();
        let proxy = app.world_mut().spawn_empty().id();
        app.insert_resource(Selection {
            visual,
            proxy,
            level: 0,
        });
        app.add_systems(Update, |mut commands: Commands, s: Res<Selection>| {
            set_shadow_detail(&mut commands, s.visual, s.proxy, s.level);
        });
        for level in [0, 2, 3, 1] {
            app.world_mut().resource_mut::<Selection>().level = level;
            app.update();
            assert_eq!(
                app.world()
                    .get::<bevy::light::NotShadowCaster>(visual)
                    .is_some(),
                level < 2
            );
            assert_eq!(
                *app.world().get::<Visibility>(proxy).unwrap(),
                if level < 2 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                }
            );
            assert_eq!(
                app.world()
                    .get::<bevy::camera::visibility::DynamicSkinnedMeshBounds>(proxy)
                    .is_some(),
                level < 2
            );
        }
    }
    #[test]
    fn lod_uses_the_world_camera_position_in_exterior_views() {
        let mut app = App::new();
        app.add_systems(Update, select_mesh_detail);
        let camera = app
            .world_mut()
            .spawn((
                crate::player::avatar::PlayerCamera,
                Camera {
                    viewport: Some(bevy::camera::Viewport {
                        physical_size: UVec2::new(3840, 2160),
                        ..default()
                    }),
                    ..default()
                },
                Projection::custom(crate::rendering::view_distance::FinitePerspective::new(
                    260.,
                )),
                GlobalTransform::from_translation(Vec3::X * 50.),
            ))
            .id();
        let proxy = app.world_mut().spawn(Visibility::Hidden).id();
        let entity = app
            .world_mut()
            .spawn((
                GlobalTransform::from_translation(Vec3::X * 50.5),
                Mesh3d::default(),
                MeshMaterial3d::<StandardMaterial>::default(),
                MeshDetail {
                    proxy,
                    levels: vec![
                        DetailAsset {
                            mesh: default(),
                            material: default()
                        };
                        4
                    ],
                    level: 3,
                },
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<MeshDetail>(entity).unwrap().level, 0);
        assert_eq!(
            *app.world().get::<Visibility>(proxy).unwrap(),
            Visibility::Inherited
        );
        app.world_mut()
            .entity_mut(camera)
            .insert(GlobalTransform::IDENTITY);
        app.update();
        assert_eq!(app.world().get::<MeshDetail>(entity).unwrap().level, 3);
        assert_eq!(
            *app.world().get::<Visibility>(proxy).unwrap(),
            Visibility::Hidden
        );
    }
    #[test]
    fn projected_size_uses_the_custom_world_projection() {
        let projection = Projection::custom(
            crate::rendering::view_distance::FinitePerspective::new(260.),
        );
        assert!((focal_pixels(&projection, 2160) - 2160. / 1.32).abs() < 0.001);
        assert!(
            (focal_pixels(&projection, 1080) * 2. - focal_pixels(&projection, 2160)).abs() < 0.001
        );
    }
    #[test]
    fn projected_detail_has_hysteresis_and_handles_large_camera_jumps() {
        assert_eq!(detail_level(0, 600.), 0);
        assert_eq!(detail_level(1, 600.), 1);
        assert_eq!(detail_level(0, 500.), 1);
        assert_eq!(detail_level(3, 700.), 0);
        assert_eq!(detail_level(0, 20.), 3);
        assert_eq!(detail_level(2, 80.), 2);
        assert_eq!(detail_level(3, 80.), 3);
    }
    #[test]
    fn physics_tick_count_is_independent_of_render_rate() {
        for fps in [30, 60, 80, 144, 240] {
            let mut clock = SimulationClock::default();
            let ticks: usize = (0..fps * 10).map(|_| clock.advance(1. / fps as f32)).sum();
            assert!((299..=300).contains(&ticks), "{fps}: {ticks}");
            assert!((0. ..1.).contains(&clock.alpha()));
        }
    }
    #[test]
    fn small_steps_accelerate_at_distant_dens_at_every_frame_rate() {
        for fps in [30, 80, 144, 240] {
            for origin in [
                Vec2::ZERO,
                Vec2::new(-9287.5, -7348.5),
                Vec2::new(-18508., -2194.),
                Vec2::splat(32000.),
            ] {
                for direction in [Vec2::X, Vec2::new(0.8, 0.6)] {
                    let mut motion = Motion::default();
                    let mut p = origin;
                    let dt = 1. / fps as f32;
                    for _ in 0..fps * 2 {
                        motion.accelerate(0.18, dt);
                        p = motion.destination(p, direction, dt);
                    }
                    let r = (-8. / fps as f64).exp();
                    let distance = 0.18 * (2. - r * (1. - r.powi(fps * 2)) / (1. - r) / fps as f64);
                    assert!(
                        (p - origin - direction * distance as f32).length() < 0.004,
                        "origin={origin:?} fps={fps} moved={:?}",
                        p - origin
                    );
                    assert!((motion.speed - 0.18).abs() < 0.0001);
                }
            }
        }
    }
    #[test]
    fn rejected_steps_do_not_accumulate_a_push_through_walls() {
        let origin = Vec2::new(-18508., -2194.);
        let mut motion = Motion::default();
        for _ in 0..1000 {
            motion.accelerate(0.18, 1. / 144.);
            let candidate = motion.destination(origin, Vec2::X, 1. / 144.);
            // Simulate collision rejection: the actor stays at the wall.
            assert!((candidate - origin).length() < 0.003);
            assert!(motion.remainder.length() < 0.001);
        }
        assert!(motion.destination(origin, Vec2::ZERO, 1. / 144.) == origin);
    }
    #[test]
    fn crown_is_one_third_of_player() {
        assert!((SCALE * 1.024 * 3. - 1.517_331_4 * PLAYER_EYE_HEIGHT / 1.43).abs() < 1e-6);
    }
    fn streaming_app(preview: bool) -> App {
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.init_resource::<Time>()
            .init_resource::<GameState>()
            .init_resource::<CameraRig>()
            .init_resource::<Population>()
            .insert_resource(Art {
                source: default(),
                gameplay: default(),
                details: default(),
                scene: Some(default()),
                graph: default(),
                clips: vec![],
                stride_speed: 1.,
                preview,
            })
            .add_systems(Update, stream.in_set(Layer::Goblins));
        app
    }

    #[test]
    fn normal_streaming_does_not_spawn_surface_goblins() {
        let mut app = streaming_app(false);
        for position in [
            Vec3::ZERO,
            Vec3::new(96., 0., 48.),
            Vec3::new(-96., 0., -48.),
            Vec3::ZERO,
        ] {
            app.world_mut().resource_mut::<CameraRig>().position = position;
            app.update();
            assert_eq!(
                app.world_mut().query::<&Goblin>().iter(app.world()).count(),
                0
            );
            assert!(app.world().resource::<Population>().preview.is_none());
        }
    }

    #[test]
    fn preview_spawns_one_subject_without_duplicates() {
        let mut app = streaming_app(true);
        for _ in 0..3 {
            app.update();
            let mut query = app.world_mut().query::<&Goblin>();
            let goblin = query.single(app.world()).unwrap();
            assert_eq!(goblin.home.xz(), Vec2::new(0., -4.));
            assert!(app.world().resource::<Population>().preview.is_some());
        }
    }
}
