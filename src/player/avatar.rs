//! Hither Avatar v1: self-contained glTF mesh, materials, skeleton and named clips.
use std::{collections::HashMap, path::Path, time::Duration};

use bevy::{
    camera::visibility::NoFrustumCulling, prelude::*, world_serialization::WorldInstanceReady,
};
use serde::Deserialize;

use crate::app::GameState;
use crate::player::camera::{CameraMode, CameraRig, CameraView};
use crate::player::movement::PLAYER_EYE_HEIGHT;

const DEFAULT_AVATAR: &str = "avatars/default.hither-avatar.glb";
const CLIPS: [&str; 8] = [
    "Idle",
    "Walk",
    "WalkBack",
    "StrafeLeft",
    "StrafeRight",
    "Jump",
    "Fall",
    "Land",
];

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_avatar).add_systems(
            Update,
            (
                spawn_loaded_avatar,
                spawn_first_person_body,
                sync_avatar
                    .after(crate::world::snow::update)
                    .after(super::hand_attack::update)
                    .before(crate::rendering::sdf::update_shader_uniforms),
            ),
        );
        app.add_systems(
            PostUpdate,
            sync_punch_layer
                .after(bevy::animation::advance_animations)
                .before(bevy::animation::animate_targets),
        );
    }
}

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component, Default)]
struct AvatarRoot {
    first_person: bool,
    yaw: Option<f32>,
    previous_position: Option<Vec3>,
    turn_direction: i8,
    turn_elapsed: f32,
    planted_foot: Option<(bool, f32)>, // left/right and foot yaw at contact
}

const TURN_CYCLE_SECONDS: f32 = 0.72;

fn angle_difference(target: f32, current: f32) -> f32 {
    (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}

fn update_body_heading(body: &mut AvatarRoot, target: f32, moving: bool, dt: f32) {
    body.planted_foot = None;
    let yaw = *body.yaw.get_or_insert(target);
    let error = angle_difference(target, yaw);
    if moving {
        body.yaw = Some(yaw + error * (1.0 - (-18.0 * dt).exp()));
        body.turn_direction = 0;
        body.turn_elapsed = 0.0;
        return;
    }
    // Tiny mouse corrections do not shuffle the feet. Larger changes start
    // alternating planted steps; finish the current half-cycle before resting.
    if body.turn_direction == 0 && error.abs() > 0.12 {
        body.turn_direction = if error > 0.0 { 1 } else { -1 };
        body.turn_elapsed = 0.0;
    }
    if body.turn_direction != 0 {
        let old_step = (body.turn_elapsed / (TURN_CYCLE_SECONDS * 0.5)).floor();
        body.turn_elapsed += dt;
        let foot_lift = (body.turn_elapsed / TURN_CYCLE_SECONDS * std::f32::consts::TAU)
            .sin()
            .powi(2);
        let max_rotation = (0.4 + 3.4 * foot_lift) * dt;
        // The torso follows the footwork, never an instant camera-yaw snap.
        let rotation = if error.signum() == body.turn_direction as f32 {
            error.clamp(-max_rotation, max_rotation)
        } else {
            0.0
        };
        body.yaw = Some(yaw + rotation);
        let planted = (body.turn_elapsed / (TURN_CYCLE_SECONDS * 0.5)).floor() > old_step;
        if planted {
            let half_cycle = (body.turn_elapsed / (TURN_CYCLE_SECONDS * 0.5)).floor() as u32;
            // Turn clips lift left in the first half and right in the second.
            body.planted_foot = Some((
                half_cycle % 2 == 1,
                body.yaw.unwrap() + body.turn_direction as f32 * 0.14,
            ));
        }
        if planted && (error.abs() < 0.06 || error.signum() != body.turn_direction as f32) {
            body.turn_direction = 0;
        }
    }
}

#[derive(Component)]
struct AvatarAnimation {
    first_person: bool,
    current: usize,
    was_grounded: bool,
    landing_remaining: f32,
    previous_position: Option<Vec3>,
    playback_speed: f32,
    landing_speed: f32,
}

#[derive(Resource)]
struct AvatarAsset {
    gltf: Handle<Gltf>,
    profile: AvatarProfile,
    spawned: bool,
    is_default: bool,
}

#[derive(Resource)]
struct FirstPersonBodyAsset(Handle<Gltf>);

#[derive(Resource)]
struct FirstPersonBodyAnimations(AvatarAnimations);

#[derive(Resource)]
struct AvatarAnimations {
    graph: Handle<AnimationGraph>,
    indices: Vec<AnimationNodeIndex>,
    durations: Vec<f32>,
    punch: Option<PunchLayer>,
}

struct PunchLayer {
    index: AnimationNodeIndex,
    duration: f32,
    arm_locomotion: Vec<AnimationNodeIndex>,
}

#[derive(Clone, Debug, Deserialize)]
struct AvatarProfile {
    version: u32,
    eye_height: f32,
    locomotion_speed: f32,
    forward: String,
    bones: HashMap<String, String>,
}

/// Validate the authoring contract before handing a custom asset to the renderer.
/// Structural glTF/accessor validation remains the glTF loader's responsibility.
fn validate_avatar(bytes: &[u8]) -> Result<AvatarProfile, String> {
    let u32_at = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
    if bytes.len() < 20 || bytes.len() > 64 * 1024 * 1024 || &bytes[..4] != b"glTF" {
        return Err("expected a GLB file of at most 64 MiB".into());
    }
    if u32_at(4) != 2 || u32_at(8) != bytes.len() || &bytes[16..20] != b"JSON" {
        return Err("invalid GLB 2.0 header".into());
    }
    let json_end = 20_usize
        .checked_add(u32_at(12))
        .ok_or("invalid JSON length")?;
    let json = bytes.get(20..json_end).ok_or("truncated GLB JSON")?;
    let doc: serde_json::Value = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let profile: AvatarProfile =
        serde_json::from_value(doc["asset"]["extras"]["hither_avatar"].clone())
            .map_err(|e| format!("missing or invalid Hither Avatar metadata: {e}"))?;
    if profile.version != 1
        || profile.forward != "+Z"
        || !profile.eye_height.is_finite()
        || !(0.5..=2.5).contains(&profile.eye_height)
        || !profile.locomotion_speed.is_finite()
        || !(0.1..=10.0).contains(&profile.locomotion_speed)
    {
        return Err("unsupported version, orientation, eye height or walk speed".into());
    }
    for kind in ["buffers", "images"] {
        if doc[kind]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item.get("uri").is_some()))
        {
            return Err("all buffers and textures must be embedded in the GLB".into());
        }
    }
    let nodes = doc["nodes"].as_array().ok_or("missing nodes")?;
    for semantic in [
        "hips",
        "head",
        "left_hand",
        "right_hand",
        "left_foot",
        "right_foot",
    ] {
        let name = profile
            .bones
            .get(semantic)
            .ok_or(format!("missing bone mapping: {semantic}"))?;
        if nodes
            .iter()
            .filter(|n| n["name"].as_str() == Some(name.as_str()))
            .count()
            != 1
        {
            return Err(format!("bone {name} must name exactly one node"));
        }
    }
    let skins = doc["skins"].as_array().ok_or("missing skinned skeleton")?;
    if skins.is_empty()
        || skins.iter().any(|s| {
            s["joints"]
                .as_array()
                .is_none_or(|j| j.is_empty() || j.len() > 128)
        })
    {
        return Err("each skin requires 1–128 joints".into());
    }
    for name in profile.bones.values() {
        let node = nodes
            .iter()
            .position(|n| n["name"].as_str() == Some(name.as_str()))
            .ok_or_else(|| format!("unknown mapped bone {name}"))?;
        if !skins.iter().any(|skin| {
            skin["joints"]
                .as_array()
                .unwrap()
                .iter()
                .any(|j| j.as_u64() == Some(node as u64))
        }) {
            return Err(format!("mapped bone {name} is not a skin joint"));
        }
    }
    if !nodes
        .iter()
        .any(|node| node.get("skin").is_some() && node.get("mesh").is_some())
    {
        return Err("missing skinned mesh node".into());
    }
    let clips = doc["animations"]
        .as_array()
        .ok_or("missing animation clips")?;
    for name in CLIPS {
        if clips
            .iter()
            .filter(|c| c["name"].as_str() == Some(name))
            .count()
            != 1
        {
            return Err(format!("expected exactly one {name} clip"));
        }
    }
    if doc["scenes"].as_array().is_none_or(|s| s.is_empty()) {
        return Err("missing scene".into());
    }
    Ok(profile)
}

fn load_profile(path: &str) -> Result<AvatarProfile, String> {
    // Keep custom selection within assets; no writes or extraction are needed.
    if Path::new(path)
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("avatar path must be relative to assets".into());
    }
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(path),
    )
    .map_err(|e| e.to_string())?;
    validate_avatar(&bytes)
}

pub fn validate_file(path: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    validate_avatar(&bytes).map(|_| ())
}

fn load_avatar(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(FirstPersonBodyAsset(
        server.load("avatars/first-person-body.glb"),
    ));
    let requested = std::env::var("HITHER_AVATAR").unwrap_or_else(|_| DEFAULT_AVATAR.into());
    let (path, profile) = match load_profile(&requested) {
        Ok(profile) => (requested, profile),
        Err(e) => {
            warn!("Avatar rejected: {e}; using bundled avatar");
            (
                DEFAULT_AVATAR.into(),
                load_profile(DEFAULT_AVATAR).expect("bundled avatar must meet Hither Avatar v1"),
            )
        }
    };
    commands.insert_resource(AvatarAsset {
        gltf: server.load(path.clone()),
        profile,
        spawned: false,
        is_default: path == DEFAULT_AVATAR,
    });
}

fn spawn_loaded_avatar(
    mut commands: Commands,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut asset: ResMut<AvatarAsset>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if asset.spawned {
        return;
    }
    if matches!(
        server.get_load_state(&asset.gltf),
        Some(bevy::asset::LoadState::Failed(_))
    ) {
        if !asset.is_default {
            warn!("Custom avatar could not load; falling back to bundled avatar");
            asset.gltf = server.load(DEFAULT_AVATAR);
            asset.profile = load_profile(DEFAULT_AVATAR).expect("bundled avatar profile");
            asset.is_default = true;
        }
        return;
    }
    if !server.is_loaded_with_dependencies(&asset.gltf) {
        return;
    }
    let Some(gltf) = gltfs.get(&asset.gltf) else {
        return;
    };
    let mut handles: Vec<_> = CLIPS
        .iter()
        .map(|name| gltf.named_animations[*name].clone())
        .collect();
    // Optional v1 extension: older custom avatars retain their strafe fallback.
    for (name, fallback) in [("TurnLeft", "StrafeLeft"), ("TurnRight", "StrafeRight")] {
        handles.push(
            gltf.named_animations
                .get(name)
                .unwrap_or(&gltf.named_animations[fallback])
                .clone(),
        );
    }
    let (graph, indices, punch) =
        exterior_animation_graph(&handles, gltf.named_animations.get("Punch"), &clips);
    commands.insert_resource(AvatarAnimations {
        graph: graphs.add(graph),
        indices,
        punch,
        durations: handles
            .iter()
            .map(|handle| {
                clips
                    .get(handle)
                    .map_or(1.0, |clip| clip.duration().max(0.001))
            })
            .collect(),
    });
    commands
        .spawn((
            AvatarRoot::default(),
            WorldAssetRoot(
                gltf.default_scene
                    .clone()
                    .unwrap_or_else(|| gltf.scenes[0].clone()),
            ),
            Transform::from_scale(Vec3::splat(PLAYER_EYE_HEIGHT / asset.profile.eye_height)),
            Visibility::Hidden,
        ))
        .observe(avatar_ready);
    asset.spawned = true;
}

// Split only the targets supplied by Punch into their own locomotion layer.
// Its exact complementary weight lets the fist fully replace the resting hand,
// while every other bone keeps the original locomotion weights and timing.
fn exterior_animation_graph(
    handles: &[Handle<AnimationClip>],
    punch: Option<&Handle<AnimationClip>>,
    clips: &Assets<AnimationClip>,
) -> (AnimationGraph, Vec<AnimationNodeIndex>, Option<PunchLayer>) {
    let (mut graph, indices) = AnimationGraph::from_clips(handles.iter().cloned());
    let Some((handle, clip)) =
        punch.and_then(|handle| clips.get(handle).map(|clip| (handle, clip)))
    else {
        return (graph, indices, None);
    };
    for handle in handles {
        if let Some(locomotion) = clips.get(handle) {
            for target in locomotion.curves().keys() {
                graph.add_target_to_mask_group(
                    *target,
                    if clip.curves().contains_key(target) {
                        0
                    } else {
                        1
                    },
                );
            }
        }
    }
    for index in &indices {
        graph.get_mut(*index).unwrap().mask = 1;
    }
    let arm_locomotion = handles
        .iter()
        .map(|handle| graph.add_clip_with_mask(handle.clone(), 2, 1.0, graph.root))
        .collect();
    let index = graph.add_clip_with_mask(handle.clone(), 2, 1.0, graph.root);
    let layer = PunchLayer {
        index,
        duration: clip.duration(),
        arm_locomotion,
    };
    (graph, indices, Some(layer))
}

fn sync_punch_layer(
    punch: Res<super::hand_attack::Punch>,
    game: Res<GameState>,
    rig: Res<CameraRig>,
    animations: Option<Res<AvatarAnimations>>,
    mut players: Query<(&mut AnimationPlayer, &AvatarAnimation)>,
) {
    if game.paused || rig.is_spectating() {
        return;
    }
    let Some(animations) = animations else { return };
    let Some(layer) = &animations.punch else {
        return;
    };
    let weight = punch.body_weight();
    for (mut player, state) in &mut players {
        if state.first_person {
            continue;
        }
        // Sample after Bevy has advanced and faded the locomotion clips, so
        // both halves of the skeleton agree even during gait transitions.
        for (base, arm) in animations.indices.iter().zip(&layer.arm_locomotion) {
            if let Some(active) = player.animation(*base) {
                let (time, base_weight) = (active.seek_time(), active.weight());
                player
                    .play(*arm)
                    .set_speed(0.0)
                    .seek_to(time)
                    .set_weight(base_weight * (1.0 - weight));
            } else {
                player.stop(*arm);
            }
        }
        if let Some(elapsed) = punch.body_elapsed() {
            player
                .play(layer.index)
                .set_speed(0.0)
                .seek_to(elapsed / super::hand_attack::BODY_DURATION * layer.duration)
                .set_weight(weight);
        } else {
            player.stop(layer.index);
        }
    }
}

// The close-view body has its own graph so custom exterior avatars cannot alter
// its proportions or animation targets. Both use the same locomotion controller.
fn spawn_first_person_body(
    mut commands: Commands,
    server: Res<AssetServer>,
    asset: Option<Res<FirstPersonBodyAsset>>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Some(asset) = asset else { return };
    if !server.is_loaded_with_dependencies(&asset.0) {
        return;
    }
    let Some(gltf) = gltfs.get(&asset.0) else {
        return;
    };
    let handles: Vec<_> = CLIPS
        .iter()
        .copied()
        .chain(["TurnLeft", "TurnRight"])
        .map(|name| gltf.named_animations[name].clone())
        .collect();
    let (graph, indices) = AnimationGraph::from_clips(handles.iter().cloned());
    let punch = None;
    commands.insert_resource(FirstPersonBodyAnimations(AvatarAnimations {
        graph: graphs.add(graph),
        indices,
        punch,
        durations: handles
            .iter()
            .map(|h| clips.get(h).unwrap().duration().max(0.001))
            .collect(),
    }));
    commands
        .spawn((
            AvatarRoot {
                first_person: true,
                ..default()
            },
            WorldAssetRoot(
                gltf.default_scene
                    .clone()
                    .unwrap_or_else(|| gltf.scenes[0].clone()),
            ),
            Transform::from_scale(Vec3::splat(PLAYER_EYE_HEIGHT / 1.43)),
            Visibility::Hidden,
        ))
        .observe(avatar_ready);
    commands.remove_resource::<FirstPersonBodyAsset>();
}

#[allow(clippy::too_many_arguments)] // Scene readiness needs both independent animation graphs.
fn avatar_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    animations: Option<Res<AvatarAnimations>>,
    body_animations: Option<Res<FirstPersonBodyAnimations>>,
    roots: Query<&AvatarRoot>,
    meshes: Query<(), With<Mesh3d>>,
) {
    let first_person = roots.get(ready.entity).is_ok_and(|root| root.first_person);
    let animations = if first_person {
        &body_animations
            .as_ref()
            .expect("body graph loaded before scene")
            .0
    } else {
        animations
            .as_deref()
            .expect("avatar graph loaded before scene")
    };
    for entity in children.iter_descendants(ready.entity) {
        if meshes.contains(entity) {
            commands.entity(entity).insert(NoFrustumCulling);
        }
        if let Ok(mut player) = players.get_mut(entity) {
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, animations.indices[0], Duration::ZERO)
                .repeat();
            commands.entity(entity).insert((
                AnimationGraphHandle(animations.graph.clone()),
                transitions,
                AvatarAnimation {
                    first_person,
                    current: 0,
                    was_grounded: true,
                    landing_remaining: 0.0,
                    previous_position: None,
                    playback_speed: 1.0,
                    landing_speed: 1.0,
                },
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)] // Independent Bevy resources/queries are intentional.
fn sync_avatar(
    mut tracks: Option<ResMut<crate::world::snow::Tracks>>,
    time: Res<Time>,
    rig: Res<CameraRig>,
    view: Res<CameraView>,
    game: Res<GameState>,
    asset: Option<Res<AvatarAsset>>,
    animations: Option<Res<AvatarAnimations>>,
    body_animations: Option<Res<FirstPersonBodyAnimations>>,
    mut cameras: Query<&mut Transform, (With<PlayerCamera>, Without<AvatarRoot>)>,
    mut roots: Query<(&mut Transform, &mut Visibility, &mut AvatarRoot), Without<PlayerCamera>>,
    mut players: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut AvatarAnimation,
    )>,
) {
    for mut camera in &mut cameras {
        *camera = Transform::from_translation(view.position).looking_to(view.forward, view.up);
    }
    let mut turns = [(0, 0.0); 2];
    for (mut transform, mut visibility, mut body) in &mut roots {
        transform.translation = Vec3::new(rig.position.x, rig.jump_height, rig.position.z);
        let previous = body
            .previous_position
            .replace(rig.position)
            .unwrap_or(rig.position);
        let speed = (rig.position - previous).with_y(0.0).length() / time.delta_secs().max(0.0001);
        if !game.paused {
            update_body_heading(
                &mut body,
                std::f32::consts::PI - rig.yaw,
                speed > 0.05 || !rig.grounded,
                time.delta_secs().min(0.1),
            );
            if !body.first_person
                && !rig.is_spectating()
                && rig.grounded
                && let Some((left, foot_yaw)) = body.planted_foot
                && let Some(tracks) = tracks.as_deref_mut()
            {
                tracks.stamp_turn(rig.position.xz(), std::f32::consts::PI - foot_yaw, left);
            }
        }
        if body.first_person {
            // Keep the abdomen behind the eye during fast mouse turns. The
            // planted-step controller still supplies small natural yaw lag.
            let target = std::f32::consts::PI - rig.yaw;
            let yaw = body.yaw.unwrap_or(target);
            body.yaw = Some(target + angle_difference(yaw, target).clamp(-0.35, 0.35));
        }
        turns[usize::from(body.first_person)] = (
            body.turn_direction,
            (body.turn_elapsed / TURN_CYCLE_SECONDS).rem_euclid(1.0),
        );
        transform.rotation =
            Quat::from_rotation_y(body.yaw.unwrap_or(std::f32::consts::PI - rig.yaw));
        if body.first_person {
            // Compensate the authored eye/abdomen offset in camera heading,
            // rather than swinging the whole body around the eye on turns.
            let offset = 0.38 * PLAYER_EYE_HEIGHT / 1.43;
            let behind_eye = Vec3::new(-rig.yaw.sin(), 0.0, rig.yaw.cos()) * offset;
            let authored_offset = transform.rotation * (Vec3::NEG_Z * offset);
            transform.translation += behind_eye - authored_offset;
        }
        *visibility = if !rig.is_spectating()
            && (rig.camera_mode == CameraMode::FirstPerson) == body.first_person
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (mut player, mut transitions, mut state) in &mut players {
        let (animations, eye_height, locomotion_speed) = if state.first_person {
            let Some(graph) = body_animations.as_ref() else {
                continue;
            };
            (&graph.0, 1.43, 1.6)
        } else {
            let (Some(asset), Some(graph)) = (asset.as_ref(), animations.as_ref()) else {
                continue;
            };
            (
                &**graph,
                asset.profile.eye_height,
                asset.profile.locomotion_speed,
            )
        };
        let (turning, turn_phase) = turns[usize::from(state.first_person)];
        let previous = state
            .previous_position
            .replace(rig.position)
            .unwrap_or(rig.position);
        if game.paused || rig.is_spectating() {
            player.pause_all();
            continue;
        }
        player.resume_all();
        let velocity = (rig.position - previous).with_y(0.0) / time.delta_secs().max(0.0001);
        let speed = velocity.length();
        if rig.grounded && !state.was_grounded {
            // Recover sooner when continuing a stride, but play the entire
            // compression/recovery clip rather than cutting its knees off.
            state.landing_remaining = if speed > 0.2 { 0.20 } else { 0.30 };
            state.landing_speed = animations.durations[7] / state.landing_remaining;
        }
        state.was_grounded = rig.grounded;
        state.landing_remaining = (state.landing_remaining - time.delta_secs()).max(0.0);
        let forward = Vec3::new(rig.yaw.sin(), 0.0, -rig.yaw.cos());
        let right = forward.cross(Vec3::Y);
        let desired = if !rig.grounded {
            if rig.vertical_velocity > 0.2 { 5 } else { 6 }
        } else if state.landing_remaining > 0.0 {
            7
        } else if speed < 0.05 {
            if turning > 0 {
                8
            } else if turning < 0 {
                9
            } else {
                0
            }
        } else if velocity.dot(right).abs() > velocity.dot(forward).abs() * 1.4 {
            if velocity.dot(right) < 0.0 { 3 } else { 4 }
        } else if velocity.dot(forward) < -0.1 {
            2
        } else {
            1
        };
        if state.current != desired {
            let gait_phase = if (1..=4).contains(&state.current) && (1..=4).contains(&desired) {
                player
                    .animation(animations.indices[state.current])
                    .map(|active| {
                        (active.seek_time() / animations.durations[state.current]).rem_euclid(1.0)
                    })
            } else {
                None
            };
            let active = transitions.play(
                &mut player,
                animations.indices[desired],
                Duration::from_secs_f32(if desired == 7 {
                    0.045
                } else if desired >= 5 {
                    0.09
                } else {
                    0.18
                }),
            );
            active.replay();
            if !(5..8).contains(&desired) {
                active.repeat();
            }
            if let Some(phase) = gait_phase {
                active.seek_to(phase * animations.durations[desired]);
            }
            state.current = desired;
        }
        if let Some(active) = player.animation_mut(animations.indices[desired]) {
            if desired >= 8 {
                // Drive the turn clip from the same phase as body rotation.
                active.seek_to(turn_phase * animations.durations[desired]);
                active.set_speed(0.0);
                continue;
            }
            let scale = PLAYER_EYE_HEIGHT / eye_height;
            let target_speed = (speed / (locomotion_speed * scale)).clamp(0.25, 2.0);
            state.playback_speed +=
                (target_speed - state.playback_speed) * (1.0 - (-14.0 * time.delta_secs()).exp());
            active.set_speed(if (1..=4).contains(&desired) {
                state.playback_speed
            } else if desired == 7 {
                state.landing_speed
            } else {
                1.0
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn punch_masks_and_weights_preserve_the_other_arm_and_replace_the_fist() {
        use bevy::animation::AnimationTargetId;
        let right = AnimationTargetId::from_name(&Name::new("right_hand"));
        let left = AnimationTargetId::from_name(&Name::new("left_hand"));
        let mut clips = Assets::<AnimationClip>::default();
        let mut gait = AnimationClip::default();
        gait.curves_mut().insert(right, vec![]);
        gait.curves_mut().insert(left, vec![]);
        let gait = clips.add(gait);
        let mut strike = AnimationClip::default();
        strike.set_duration(0.88);
        strike.curves_mut().insert(right, vec![]);
        let strike = clips.add(strike);
        let (graph, indices, layer) =
            exterior_animation_graph(&[gait.clone(), gait], Some(&strike), &clips);
        let layer = layer.unwrap();
        assert_eq!(graph.mask_groups[&right], 1);
        assert_eq!(graph.mask_groups[&left], 2);
        assert_eq!(graph[indices[0]].mask, 1);
        assert_eq!(graph[layer.arm_locomotion[0]].mask, 2);
        let arm = layer.arm_locomotion.clone();
        let punch_index = layer.index;
        let mut player = AnimationPlayer::default();
        player.play(indices[0]).seek_to(0.17).set_weight(0.35);
        player.play(indices[1]).seek_to(0.41).set_weight(0.65);
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(0.02));
        app.insert_resource(time)
            .init_resource::<super::super::hand_attack::Punch>()
            .init_resource::<CameraRig>()
            .init_resource::<GameState>()
            .init_resource::<crate::ui::chat::ChatState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(AvatarAnimations {
                graph: Handle::default(),
                indices: indices.clone(),
                durations: vec![1.; 2],
                punch: Some(layer),
            })
            .add_systems(
                Update,
                (super::super::hand_attack::update, sync_punch_layer).chain(),
            );
        let entity = app
            .world_mut()
            .spawn((
                player,
                AvatarAnimation {
                    first_person: false,
                    current: 0,
                    was_grounded: true,
                    landing_remaining: 0.,
                    previous_position: None,
                    playback_speed: 1.,
                    landing_speed: 1.,
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .get::<AnimationPlayer>(entity)
                .unwrap()
                .animation(arm[0])
                .unwrap()
                .weight(),
            0.35
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        for _ in 0..4 {
            app.update();
        }
        let player = app.world().get::<AnimationPlayer>(entity).unwrap();
        assert_eq!(player.animation(punch_index).unwrap().weight(), 1.);
        assert!(
            (player.animation(punch_index).unwrap().seek_time()
                - (0.10 + 0.025 / 0.07 * 0.12) / 0.60 * 0.88)
                .abs()
                < 0.00001
        );
        for (i, expected) in [0.35, 0.65].iter().enumerate() {
            assert_eq!(player.animation(indices[i]).unwrap().weight(), *expected);
            assert_eq!(player.animation(arm[i]).unwrap().weight(), 0.);
            assert_eq!(
                player.animation(arm[i]).unwrap().seek_time(),
                player.animation(indices[i]).unwrap().seek_time()
            );
        }
        for _ in 0..30 {
            app.update();
        }
        let player = app.world().get::<AnimationPlayer>(entity).unwrap();
        assert!(player.animation(punch_index).is_none());
        assert_eq!(player.animation(arm[0]).unwrap().weight(), 0.35);
    }

    #[test]
    fn turning_stamps_only_on_animation_contacts_even_in_first_person() {
        let mut app = App::new();
        let rig = CameraRig::default();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
        app.insert_resource(time)
            .insert_resource(rig.view())
            .insert_resource(rig)
            .insert_resource(GameState::default())
            .init_resource::<crate::world::snow::Tracks>()
            .add_systems(Update, sync_avatar);
        let origin = (-100..100)
            .map(|x| Vec2::new(x as f32 * 10.0, 0.0))
            .find(|p| crate::world::biome::snow_amount(*p) == 1.0)
            .unwrap();
        app.world_mut().resource_mut::<CameraRig>().position = Vec3::new(origin.x, 1.25, origin.y);
        app.world_mut().spawn((
            Transform::default(),
            Visibility::Hidden,
            AvatarRoot::default(),
        ));
        app.world_mut().spawn((
            Transform::default(),
            Visibility::Hidden,
            AvatarRoot {
                first_person: true,
                ..default()
            },
        ));
        app.update();
        app.world_mut().resource_mut::<CameraRig>().yaw = 2.0;
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<crate::world::snow::Tracks>()
                .uniforms()
                .1
                .x,
            0.0
        );
        for _ in 0..24 {
            app.update();
        }
        let (prints, meta, _) = app
            .world()
            .resource::<crate::world::snow::Tracks>()
            .uniforms();
        assert_eq!(meta.x, 2.0);
        assert!(prints[0].z >= std::f32::consts::TAU);
        assert!(prints[1].z < std::f32::consts::TAU);
        app.world_mut().resource_mut::<GameState>().paused = true;
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<crate::world::snow::Tracks>()
                .uniforms()
                .1
                .x,
            2.0
        );
        app.world_mut().resource_mut::<GameState>().paused = false;
        app.world_mut().resource_mut::<CameraRig>().grounded = false;
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<crate::world::snow::Tracks>()
                .uniforms()
                .1
                .x,
            2.0
        );
    }

    #[test]
    fn first_person_body_tracks_physical_feet_and_switches_with_exterior_avatar() {
        let mut app = App::new();
        let rig = CameraRig::default();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
        app.insert_resource(time)
            .insert_resource(rig.view())
            .insert_resource(rig)
            .insert_resource(GameState::default())
            .add_systems(Update, sync_avatar);
        let body = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                AvatarRoot {
                    first_person: true,
                    ..default()
                },
            ))
            .id();
        let exterior = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                AvatarRoot::default(),
            ))
            .id();
        for mode in [
            CameraMode::FirstPerson,
            CameraMode::ThirdPerson,
            CameraMode::SecondPerson,
            CameraMode::FirstPerson,
        ] {
            app.world_mut().resource_mut::<CameraRig>().camera_mode = mode;
            app.update();
            assert_eq!(
                *app.world().get::<Visibility>(body).unwrap(),
                if mode == CameraMode::FirstPerson {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                }
            );
            assert_ne!(
                app.world().get::<Visibility>(body),
                app.world().get::<Visibility>(exterior)
            );
        }
        {
            let mut rig = app.world_mut().resource_mut::<CameraRig>();
            rig.jump_height = 0.5;
            rig.position.y = PLAYER_EYE_HEIGHT + 0.5 - 0.08;
            rig.pitch = -1.5;
        }
        app.update();
        let pose = *app.world().get::<Transform>(body).unwrap();
        assert_eq!(
            pose.translation.y, 0.5,
            "landing camera bob must not bury the boots"
        );
        assert!(
            (pose.rotation * Vec3::Y - Vec3::Y).length() < 0.0001,
            "looking down must not rotate the legs with the camera"
        );
        app.world_mut().resource_mut::<CameraRig>().yaw = 2.8;
        app.update();
        let pose = *app.world().get::<Transform>(body).unwrap();
        let rig = app.world().resource::<CameraRig>();
        let heading = app.world().get::<AvatarRoot>(body).unwrap().yaw.unwrap();
        assert!(angle_difference(heading, std::f32::consts::PI - rig.yaw).abs() <= 0.351);
        let offset = 0.38 * PLAYER_EYE_HEIGHT / 1.43;
        let abdomen = pose.translation + pose.rotation * (Vec3::NEG_Z * offset);
        let expected = rig.position.with_y(rig.jump_height)
            + Vec3::new(-rig.yaw.sin(), 0.0, rig.yaw.cos()) * offset;
        assert!(
            abdomen.distance(expected) < 0.0001,
            "fast turns must keep the waist behind the eye"
        );
        app.world_mut().resource_mut::<GameState>().paused = true;
        app.update();
        assert_eq!(*app.world().get::<Transform>(body).unwrap(), pose);
        app.world_mut()
            .resource_mut::<CameraRig>()
            .toggle_spectator();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(body).unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn standing_turn_steps_without_snapping_and_finishes_planted() {
        let mut body = AvatarRoot {
            yaw: Some(0.0),
            ..default()
        };
        update_body_heading(&mut body, 0.05, false, 1.0 / 60.0);
        assert_eq!(body.turn_direction, 0);
        assert_eq!(body.yaw, Some(0.0));
        update_body_heading(&mut body, 1.0, false, 1.0 / 60.0);
        assert_eq!(body.turn_direction, 1);
        assert!(body.yaw.unwrap() < 0.05);
        for _ in 0..180 {
            update_body_heading(&mut body, 1.0, false, 1.0 / 60.0);
        }
        assert!(angle_difference(1.0, body.yaw.unwrap()).abs() < 0.06);
        assert_eq!(body.turn_direction, 0);
    }

    #[test]
    fn turning_uses_shortest_arc_and_walking_cancels_shuffle() {
        let mut body = AvatarRoot {
            yaw: Some(3.1),
            ..default()
        };
        update_body_heading(&mut body, -2.8, false, 1.0 / 60.0);
        assert_eq!(body.turn_direction, 1);
        assert!(body.yaw.unwrap() > 3.1);
        update_body_heading(&mut body, -2.8, true, 1.0 / 60.0);
        assert_eq!(body.turn_direction, 0);
        assert_eq!(body.turn_elapsed, 0.0);
    }

    #[test]
    fn bundled_avatar_obeys_profile() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/avatars/default.hither-avatar.glb"
        ));
        let profile = validate_avatar(bytes).unwrap();
        assert_eq!(profile.version, 1);
    }
    #[test]
    fn corrupt_and_truncated_files_are_rejected() {
        assert!(validate_avatar(b"glTF").is_err());
        let mut bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/avatars/default.hither-avatar.glb"
        ))
        .to_vec();
        bytes[4] = 1;
        assert!(validate_avatar(&bytes).is_err());
    }

    fn modified_avatar(change: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let original = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/avatars/default.hither-avatar.glb"
        ));
        let old_length = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
        let mut doc = serde_json::from_slice(&original[20..20 + old_length]).unwrap();
        change(&mut doc);
        let mut json = serde_json::to_vec(&doc).unwrap();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut bytes = original[..12].to_vec();
        bytes.extend((json.len() as u32).to_le_bytes());
        bytes.extend(b"JSON");
        bytes.extend(json);
        bytes.extend(&original[20 + old_length..]);
        let size = (bytes.len() as u32).to_le_bytes();
        bytes[8..12].copy_from_slice(&size);
        bytes
    }

    #[test]
    fn external_images_and_missing_clips_are_rejected() {
        let external = modified_avatar(|doc| {
            doc["images"][0]["uri"] = "https://example.com/texture.png".into()
        });
        assert!(validate_avatar(&external).unwrap_err().contains("embedded"));
        let missing = modified_avatar(|doc| {
            doc["animations"]
                .as_array_mut()
                .unwrap()
                .retain(|clip| clip["name"] != "Land");
        });
        assert!(validate_avatar(&missing).unwrap_err().contains("clip"));
    }

    #[test]
    fn invalid_metadata_and_bones_are_rejected() {
        let bad_scale =
            modified_avatar(|doc| doc["asset"]["extras"]["hither_avatar"]["eye_height"] = 0.into());
        assert!(validate_avatar(&bad_scale).is_err());
        let wrong_version =
            modified_avatar(|doc| doc["asset"]["extras"]["hither_avatar"]["version"] = 2.into());
        assert!(validate_avatar(&wrong_version).is_err());
        let missing_bone = modified_avatar(|doc| {
            doc["asset"]["extras"]["hither_avatar"]["bones"]["head"] = "NotABone".into()
        });
        assert!(validate_avatar(&missing_bone).is_err());
    }
}
