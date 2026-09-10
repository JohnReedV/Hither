//! Camera-space close-view anatomy with an independent depth buffer.
use super::hand_attack::{self, Punch};
use crate::{
    app::GameState,
    player::{
        avatar::PlayerCamera,
        camera::{CameraMode, CameraRig},
    },
};
use bevy::mesh::morph::MorphWeights;
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    world_serialization::WorldInstanceReady,
};

pub struct HandPlugin;
impl Plugin for HandPlugin {
    fn build(&self, app: &mut App) {
        crate::rendering::hand_lighting::plugin(app);
        app.init_resource::<Punch>()
            .add_systems(PostStartup, setup)
            .add_systems(
                Update,
                (hand_attack::update, animate, align_world_view)
                    .chain()
                    .after(crate::player::camera::update_camera_view),
            );
    }
}
#[derive(Component)]
struct HandCamera;
#[derive(Component)]
struct HandViewRoot;
#[derive(Component)]
struct HandDeform;
#[derive(Component, Default)]
struct HandMotion {
    previous: Option<(Vec3, f32, f32)>,
    phase: f32,
    breath: f32,
    speed: f32,
    sway: Vec2,
    lift: f32,
}
fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    target: Single<&RenderTarget, With<PlayerCamera>>,
) {
    commands.spawn((
        Camera3d::default(),
        // Match the world's binding layout; hand_lighting skips the unused prepass.
        bevy::core_pipeline::prepass::DepthPrepass,
        crate::rendering::hand_lighting::WorldLighting::Hands,
        bevy::camera::Hdr,
        crate::rendering::graphics::WorldPass,
        Camera {
            order: -2,
            output_mode: bevy::camera::CameraOutputMode::Skip,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        (*target).clone(),
        Projection::Perspective(PerspectiveProjection {
            fov: 2.0 * 0.66_f32.atan(),
            near: 0.01,
            far: 4.0,
            ..default()
        }),
        Tonemapping::None,
        Msaa::Off,
        RenderLayers::layer(2),
        Transform::default(),
        HandCamera,
    ));
    let root = commands
        .spawn((HandViewRoot, Transform::default(), Visibility::default()))
        .id();
    commands
        .spawn((
            WorldAssetRoot(server.load("avatars/first-person-hand.glb#Scene0")),
            Transform::from_xyz(0.20, -0.19, -0.34),
            Visibility::Hidden,
            HandMotion::default(),
            ChildOf(root),
        ))
        .observe(ready);
}
// Keep the independent hand depth buffer, but evaluate illumination in world space.
#[allow(clippy::type_complexity)] // Camera and rig share the same world pose.
fn align_world_view(
    view: Res<crate::player::camera::CameraView>,
    mut transforms: Query<&mut Transform, Or<(With<HandCamera>, With<HandViewRoot>)>>,
) {
    let pose = Transform::from_translation(view.position).looking_to(view.forward, view.up);
    for mut transform in &mut transforms {
        transform.set_if_neq(pose);
    }
}
fn ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh3d>>,
    morphs: Query<(), With<MorphWeights>>,
) {
    for entity in children.iter_descendants(event.entity) {
        if morphs.contains(entity) {
            commands.entity(entity).insert(HandDeform);
        }
        if meshes.contains(entity) {
            commands
                .entity(entity)
                .insert((RenderLayers::layer(2), bevy::light::NotShadowCaster));
        }
    }
}
fn animate(
    time: Res<Time>,
    rig: Res<CameraRig>,
    game: Res<GameState>,
    punch: Res<Punch>,
    mut morphs: Query<&mut MorphWeights, With<HandDeform>>,
    mut hands: Query<(&mut Transform, &mut Visibility, &mut HandMotion)>,
    mut cameras: Query<&mut Camera, With<HandCamera>>,
) {
    let attack = punch.pose();
    for mut weights in &mut morphs {
        if let Some(weight) = weights.weights_mut().first_mut() {
            *weight = attack.clench;
        }
    }
    let visible = rig.camera_mode == CameraMode::FirstPerson && !rig.is_spectating();
    for mut camera in &mut cameras {
        camera.is_active = visible;
    }
    for (mut transform, mut visibility, mut motion) in &mut hands {
        let was_visible = *visibility != Visibility::Hidden;
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let previous = motion.previous.replace((rig.position, rig.yaw, rig.pitch));
        if !visible || game.paused {
            continue;
        }
        let dt = time.delta_secs().min(0.05);
        if dt <= 0.0 {
            continue;
        }
        let (position, yaw, pitch) = previous.unwrap_or((rig.position, rig.yaw, rig.pitch));
        let travel = rig.position.distance(position);
        let reset = !was_visible || travel > 1.0;
        let speed = if reset || !rig.grounded {
            0.0
        } else {
            (rig.position - position).with_y(0.0).length() / dt
        };
        let blend = 1.0 - (-12.0 * dt).exp();
        motion.speed += (speed.min(6.0) - motion.speed) * blend;
        motion.phase += motion.speed * dt * 2.6;
        motion.breath += dt;
        let yaw_delta = (rig.yaw - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let target = if reset {
            Vec2::ZERO
        } else {
            Vec2::new(-yaw_delta / dt, (rig.pitch - pitch) / dt)
                .clamp(Vec2::splat(-2.0), Vec2::splat(2.0))
                * 0.012
        };
        motion.sway = motion.sway.lerp(target, blend);
        let lift =
            (-rig.vertical_velocity * 0.003).clamp(-0.018, 0.025) + rig.landing_offset * 0.20;
        motion.lift += (lift - motion.lift) * blend;
        let walk = (motion.speed / 3.0).min(1.0);
        transform.translation = Vec3::new(
            0.20 + motion.sway.x + motion.phase.sin() * 0.007 * walk,
            -0.19
                + motion.sway.y
                + motion.lift
                + (motion.breath * 1.65).sin() * 0.002
                + (motion.phase * 2.0).cos() * 0.004 * walk,
            -0.34 + (motion.breath * 1.65).cos() * 0.0015,
        );
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            motion.sway.y * 1.5,
            -motion.sway.x * 2.0,
            motion.phase.sin() * 0.018 * walk,
        );
        let rest = Vec3::new(0.20, -0.19, -0.34);
        transform.translation = rest
            + (transform.translation - rest) * (1.0 - 0.8 * attack.clench)
            + attack.translation;
        transform.rotation *= Quat::from_euler(
            EulerRot::XYZ,
            attack.rotation.x,
            attack.rotation.y,
            attack.rotation.z,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> (App, Entity) {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        app.insert_resource(time)
            .insert_resource(CameraRig::default())
            .insert_resource(GameState::default())
            .init_resource::<Punch>()
            .add_systems(Update, animate);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                HandMotion::default(),
            ))
            .id();
        (app, entity)
    }
    #[test]
    fn only_player_first_person_displays_hand_and_pause_freezes_pose() {
        let (mut app, entity) = app();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Inherited
        );
        let pose = *app.world().get::<Transform>(entity).unwrap();
        app.world_mut().resource_mut::<GameState>().paused = true;
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), pose);
        app.world_mut().resource_mut::<GameState>().paused = false;
        for mode in [CameraMode::ThirdPerson, CameraMode::SecondPerson] {
            app.world_mut().resource_mut::<CameraRig>().camera_mode = mode;
            app.update();
            assert_eq!(
                *app.world().get::<Visibility>(entity).unwrap(),
                Visibility::Hidden
            );
        }
        app.world_mut().resource_mut::<CameraRig>().camera_mode = CameraMode::FirstPerson;
        app.world_mut()
            .resource_mut::<CameraRig>()
            .toggle_spectator();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );
    }
    #[test]
    fn teleport_and_wrapped_yaw_do_not_throw_the_hand_across_the_view() {
        let (mut app, entity) = app();
        app.world_mut().resource_mut::<CameraRig>().yaw = std::f32::consts::PI - 0.001;
        app.update();
        app.world_mut().resource_mut::<CameraRig>().yaw = -std::f32::consts::PI + 0.001;
        app.update();
        assert!(app.world().get::<HandMotion>(entity).unwrap().sway.length() < 0.001);
        app.world_mut().resource_mut::<CameraRig>().position.x += 100.0;
        app.update();
        let hand = app.world().get::<HandMotion>(entity).unwrap();
        assert_eq!(hand.speed, 0.0);
        assert!(
            app.world()
                .get::<Transform>(entity)
                .unwrap()
                .translation
                .is_finite()
        );
    }
}
