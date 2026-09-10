use crate::app::GameState;
use crate::player::movement::{COYOTE_SECONDS, PLAYER_EYE_HEIGHT, player_collides};
use crate::player::spectator;
use crate::ui::chat;
use crate::world::{biome, orchard};
use bevy::prelude::*;

#[derive(Resource)]
pub(crate) struct CameraRig {
    pub(crate) position: Vec3,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) mouse_captured: bool,
    pub(crate) jump_height: f32,
    pub(crate) vertical_velocity: f32,
    pub(crate) grounded: bool,
    pub(crate) coyote_remaining: f32,
    pub(crate) jump_buffer_remaining: f32,
    pub(crate) landing_offset: f32,
    pub(crate) landing_offset_velocity: f32,
    pub(crate) camera_mode: CameraMode,
    pub(crate) game_mode: spectator::GameMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // Conventional camera-view names are clearer here.
pub(crate) enum CameraMode {
    FirstPerson,
    ThirdPerson,
    SecondPerson,
}

impl CameraMode {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::FirstPerson => Self::ThirdPerson,
            Self::ThirdPerson => Self::SecondPerson,
            Self::SecondPerson => Self::FirstPerson,
        }
    }
}

#[derive(Resource, Clone, Copy, PartialEq)]
pub(crate) struct CameraView {
    pub(crate) position: Vec3,
    pub(crate) forward: Vec3,
    pub(crate) right: Vec3,
    pub(crate) up: Vec3,
}

pub(crate) fn update_camera_view(rig: Res<CameraRig>, mut view: ResMut<CameraView>) {
    // The renderer, mesh camera and crosshair picker share one obstruction sweep.
    view.set_if_neq(rig.view());
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, PLAYER_EYE_HEIGHT, 3.15),
            yaw: 0.0,
            pitch: 0.12,
            mouse_captured: true,
            jump_height: 0.0,
            vertical_velocity: 0.0,
            grounded: true,
            coyote_remaining: COYOTE_SECONDS,
            jump_buffer_remaining: 0.0,
            landing_offset: 0.0,
            landing_offset_velocity: 0.0,
            camera_mode: CameraMode::FirstPerson,
            game_mode: spectator::GameMode::Player,
        }
    }
}

impl CameraRig {
    /// Coordinates use the rig's eye position in either game mode.
    pub(crate) fn teleport(&mut self, position: Vec3) {
        self.position = position;
        self.jump_height = position.y - PLAYER_EYE_HEIGHT;
        self.vertical_velocity = 0.0;
        self.grounded = false;
        self.coyote_remaining = 0.0;
        self.jump_buffer_remaining = 0.0;
        self.landing_offset = 0.0;
        self.landing_offset_velocity = 0.0;
    }

    /// Opt-in reproducible visual QA; ordinary launches always start at the world origin.
    pub(crate) fn for_startup() -> Self {
        if std::env::var_os("HITHER_GOBLIN_DEN_PREVIEW").is_some()
            && let Some((position, target)) = crate::world::goblin_dens::preview_pose()
        {
            let direction = (target - position).normalize();
            return Self {
                position,
                pitch: direction.y.asin(),
                yaw: direction.x.atan2(-direction.z),
                game_mode: spectator::GameMode::Spectator { fallback: position },
                ..default()
            };
        }
        if std::env::var_os("HITHER_GOBLIN_PREVIEW").is_some() {
            let position = Vec3::new(0.85, 0.70, -2.0);
            let direction = (Vec3::new(0., 0.23, -4.) - position).normalize();
            return Self {
                position,
                pitch: direction.y.asin(),
                yaw: direction.x.atan2(-direction.z),
                game_mode: spectator::GameMode::Spectator { fallback: position },
                ..default()
            };
        }
        let Ok(name) = std::env::var("HITHER_PREVIEW_BIOME") else {
            return Self::default();
        };
        if name == "orcs" {
            if std::env::var_os("HITHER_TORCH_INSPECT").is_some() {
                let target = crate::world::orcs::torch_preview_position();
                let position = target + Vec3::new(-1.35, 0.15, 1.65);
                let direction = (target - Vec3::Y * 0.08 - position).normalize();
                return Self {
                    position,
                    pitch: direction.y.asin(),
                    yaw: direction.x.atan2(-direction.z),
                    game_mode: spectator::GameMode::Spectator { fallback: position },
                    ..default()
                };
            }
            if std::env::var_os("HITHER_DEN_REAR").is_some() {
                let position =
                    crate::world::orcs::preview() + Vec3::new(0., PLAYER_EYE_HEIGHT - 6., -6.);
                return Self {
                    position,
                    pitch: -0.15,
                    yaw: 0.,
                    game_mode: spectator::GameMode::Spectator { fallback: position },
                    ..default()
                };
            }
            if std::env::var_os("HITHER_DEN_BOTTOM").is_some() {
                return Self {
                    position: crate::world::orcs::preview()
                        + Vec3::new(0., PLAYER_EYE_HEIGHT - 6., -6.),
                    jump_height: -6.,
                    pitch: -0.3,
                    ..default()
                };
            }
            if std::env::var_os("HITHER_DEN_ENTER").is_some() {
                return Self {
                    position: crate::world::orcs::preview() + Vec3::Y * (PLAYER_EYE_HEIGHT + 0.14),
                    jump_height: 0.14,
                    pitch: -0.1,
                    ..default()
                };
            }
            if std::env::var_os("HITHER_DEN_ACTION").is_some() {
                return Self {
                    position: crate::world::orcs::preview() + Vec3::new(6., PLAYER_EYE_HEIGHT, 7.),
                    pitch: 0.15,
                    yaw: -0.85,
                    ..default()
                };
            }
            if std::env::var_os("HITHER_DEN_INSPECT").is_some() {
                let position = crate::world::orcs::preview() + Vec3::new(8., 8., 12.);
                return Self {
                    position,
                    pitch: -0.50,
                    yaw: -0.59,
                    game_mode: spectator::GameMode::Spectator { fallback: position },
                    ..default()
                };
            }
            return Self {
                position: crate::world::orcs::preview() + Vec3::new(0., PLAYER_EYE_HEIGHT, 4.8),
                pitch: -0.22,
                ..default()
            };
        }
        if name == "oak" {
            if let Some(tree) = orchard::oak::preview_tree() {
                return Self {
                    position: tree.translation
                        + Vec3::new(0.0, PLAYER_EYE_HEIGHT, 19.0 * tree.scale.x),
                    pitch: 0.22,
                    ..default()
                };
            }
        } else {
            let wanted = match name.as_str() {
                "plains" => biome::Biome::Plains,
                "forest" => biome::Biome::TemperateForest,
                "tundra" => biome::Biome::Tundra,
                "boreal" => biome::Biome::BorealForest,
                "mountains" => biome::Biome::Mountains,
                _ => panic!(
                    "HITHER_PREVIEW_BIOME must be plains, forest, tundra, boreal, mountains, or oak"
                ),
            };
            for radius in 2..100_i32 {
                for x in -radius..=radius {
                    for z in -radius..=radius {
                        if x.abs().max(z.abs()) != radius {
                            continue;
                        }
                        let p = Vec2::new(x as f32 * 16.0, z as f32 * 16.0);
                        let interior = [-24.0, 0.0, 24.0].into_iter().all(|dx| {
                            [-24.0, 0.0, 24.0].into_iter().all(|dz| {
                                let q = p + Vec2::new(dx, dz);
                                if wanted == biome::Biome::Mountains {
                                    return biome::mountain_amount(q) > 0.95;
                                }
                                let snow = biome::snow_amount(q);
                                let forest = biome::forest_amount(q);
                                biome::at(q) == wanted
                                    && (snow == 0.0 || snow == 1.0)
                                    && (forest == 0.0 || forest == 1.0)
                            })
                        });
                        let ground = biome::terrain_height(p);
                        if wanted == biome::Biome::Mountains && ground > 22.0 {
                            continue;
                        }
                        let position = Vec3::new(p.x, ground + PLAYER_EYE_HEIGHT, p.y);
                        if interior && !player_collides(position) {
                            let mut yaw = 0.0;
                            if wanted == biome::Biome::Mountains {
                                let mut highest = ground;
                                for i in 0..32 {
                                    let angle = i as f32 * std::f32::consts::TAU / 32.0;
                                    let q = p + Vec2::new(angle.sin(), -angle.cos()) * 180.0;
                                    let h = biome::terrain_height(q);
                                    if h > highest {
                                        highest = h;
                                        yaw = angle;
                                    }
                                }
                            }
                            println!("Preview {name}: {position}");
                            return Self {
                                position,
                                jump_height: ground,
                                yaw,
                                pitch: if wanted == biome::Biome::Mountains {
                                    0.28
                                } else {
                                    0.06
                                },
                                ..default()
                            };
                        }
                    }
                }
            }
        }
        panic!("No preview location found for {name}");
    }

    pub(crate) fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();
        (forward, right, up)
    }

    pub(crate) fn view(&self) -> CameraView {
        let (player_forward, right, up) = self.basis();
        if self.is_spectating() || self.camera_mode == CameraMode::FirstPerson {
            return CameraView {
                position: self.position,
                forward: player_forward,
                right,
                up,
            };
        }

        let forward = if self.camera_mode == CameraMode::ThirdPerson {
            player_forward
        } else {
            -player_forward
        };
        // Frame the whole character, not just the eye line, in exterior views.
        let anchor = self.position - Vec3::Y * 0.32;
        let camera_distance = if self.camera_mode == CameraMode::ThirdPerson {
            THIRD_PERSON_DISTANCE
        } else {
            SECOND_PERSON_DISTANCE
        };
        let desired = anchor - forward * camera_distance;
        let position = resolve_camera_position(anchor, desired);
        let forward = (anchor - position).normalize_or(forward);
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        CameraView {
            position,
            forward,
            right,
            up,
        }
    }
}
pub(crate) fn cycle_camera_mode(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<GameState>,
    chat: Res<chat::ChatState>,
    mut rig: ResMut<CameraRig>,
) {
    if !game.paused
        && !chat.blocks_gameplay()
        && !rig.is_spectating()
        && keys.just_pressed(KeyCode::F3)
    {
        rig.camera_mode = rig.camera_mode.next();
    }
}

fn camera_point_obstructed(point: Vec3) -> bool {
    if point.y < 0. && crate::world::orcs::camera_clear(point, CAMERA_COLLISION_RADIUS) {
        return false;
    }
    if point.y <= biome::terrain_height(point.xz()) + CAMERA_COLLISION_RADIUS
        && !crate::world::orcs::camera_clear(point, CAMERA_COLLISION_RADIUS)
    {
        return true;
    }

    if orchard::logs::collides(point, CAMERA_COLLISION_RADIUS, 0.0)
        || orchard::wild::camera_obstructed(point, CAMERA_COLLISION_RADIUS)
        || orchard::winter::camera_obstructed(point, CAMERA_COLLISION_RADIUS)
        || orchard::alpine::camera_obstructed(point, CAMERA_COLLISION_RADIUS)
        || orchard::oak::camera_obstructed(point, CAMERA_COLLISION_RADIUS)
        || crate::world::geology::camera_obstructed(point, CAMERA_COLLISION_RADIUS)
    {
        return true;
    }

    false
}

#[derive(Clone, Copy, PartialEq)]
struct BoomKey {
    anchor: Vec3,
    desired: Vec3,
    seed: u64,
    colliders: crate::world::orcs::CameraColliders,
}
#[derive(Default)]
struct BoomCache {
    last: Option<(BoomKey, Vec3)>,
}
impl BoomCache {
    fn solve(&mut self, key: BoomKey, solve: impl FnOnce() -> Vec3) -> Vec3 {
        if let Some((previous, result)) = self.last
            && previous == key
        {
            return result;
        }
        let result = solve();
        self.last = Some((key, result));
        result
    }
}
pub(crate) fn resolve_camera_position(anchor: Vec3, desired: Vec3) -> Vec3 {
    static UNCACHED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let uncached = *UNCACHED.get_or_init(|| {
        std::env::var_os("HITHER_PROFILE_FOREST").is_some()
            && std::env::var_os("HITHER_PROFILE_UNCACHED_CAMERA").is_some()
    });
    if !uncached
        && let Some(colliders) = crate::world::orcs::camera_colliders(
            anchor,
            anchor.distance(desired) + CAMERA_COLLISION_RADIUS,
        )
    {
        thread_local! { static CACHE: std::cell::RefCell<BoomCache> = Default::default(); }
        return CACHE.with_borrow_mut(|cache| {
            cache.solve(
                BoomKey {
                    anchor,
                    desired,
                    seed: biome::world_seed(),
                    colliders,
                },
                || resolve_camera_with(anchor, desired, camera_point_obstructed),
            )
        });
    }
    resolve_camera_with(anchor, desired, camera_point_obstructed)
}

fn resolve_camera_with(anchor: Vec3, desired: Vec3, blocked: impl Fn(Vec3) -> bool) -> Vec3 {
    let sweep = |start: Vec3, end: Vec3| camera_sweep(start, end, &blocked);
    let direct = sweep(anchor, desired);
    if direct.distance(anchor) >= 1.2 && direct.xz().distance(anchor.xz()) >= 0.65 {
        return direct;
    }
    // Solve the boom direction, not just its length. Translating the original
    // pitched boom downward still drove it into lintels when looking down.
    // First flatten the boom, then fan out around either shoulder if necessary.
    // Every candidate is swept directly from the framing target: no bent path
    // that leaves a wall between the camera and the character.
    let offset = desired - anchor;
    let distance = offset.length();
    let horizontal = Vec3::new(offset.x, 0.0, offset.z).normalize_or(Vec3::Z);
    let mut best = direct;
    let mut best_clearance = direct.xz().distance(anchor.xz());
    for angle in [
        0.0_f32, 30.0, -30.0, 60.0, -60.0, 90.0, -90.0, 135.0, -135.0, 180.0,
    ] {
        let direction = Quat::from_rotation_y(angle.to_radians()) * horizontal;
        for elevation in [0.0, -0.15, -0.3, 0.15, 0.3] {
            let target = anchor + (direction + Vec3::Y * elevation).normalize() * distance;
            let candidate = sweep(anchor, target);
            let clearance = candidate.xz().distance(anchor.xz());
            if clearance > best_clearance {
                best = candidate;
                best_clearance = clearance;
            }
            if clearance >= distance * 0.8 {
                return candidate;
            }
        }
    }
    best
}

fn camera_sweep(anchor: Vec3, desired: Vec3, blocked: &impl Fn(Vec3) -> bool) -> Vec3 {
    const SAMPLES: usize = 48;
    let mut safe = anchor;
    for index in 1..=SAMPLES {
        let t = index as f32 / SAMPLES as f32;
        let candidate = anchor.lerp(desired, t);
        if blocked(candidate) {
            break;
        }
        safe = candidate;
    }
    safe
}

pub(crate) const THIRD_PERSON_DISTANCE: f32 = 2.25;
pub(crate) const SECOND_PERSON_DISTANCE: f32 = 1.45;
pub(crate) const CAMERA_COLLISION_RADIUS: f32 = 0.1;

#[cfg(test)]
mod camera_collision_tests {
    use super::*;

    #[test]
    fn low_branch_uses_clear_lower_boom_instead_of_avatar_position() {
        let anchor = Vec3::new(0.0, 1.5, 0.0);
        let desired = anchor + Vec3::Z * 3.0;
        let branch = |p: Vec3| p.z > 0.8 && p.z < 1.1 && p.y > 1.4;
        let resolved = resolve_camera_with(anchor, desired, branch);
        assert!(resolved.z > 2.5);
        assert!(resolved.y < anchor.y);
        assert!(!branch(resolved));
    }

    #[test]
    fn solid_wall_still_retracts_and_clear_boom_is_unchanged() {
        let anchor = Vec3::new(0.0, 1.5, 0.0);
        let desired = anchor + Vec3::Z * 3.0;
        assert_eq!(resolve_camera_with(anchor, desired, |_| false), desired);
        let resolved = resolve_camera_with(anchor, desired, |p| p.z >= 0.5);
        assert!(resolved.z < 0.5);
        assert!(resolved.distance(anchor) > 1.2);
    }

    #[test]
    fn pitched_boom_under_door_roof_stays_outside_character() {
        let anchor = Vec3::new(0.0, 0.93, 4.3);
        let desired = anchor + Vec3::new(0.0, 2.1, 0.8);
        let roof = |p: Vec3| p.y >= 1.4;
        let resolved = resolve_camera_with(anchor, desired, roof);
        assert!(resolved.xz().distance(anchor.xz()) > 1.8);
        assert!(!roof(resolved));
    }

    #[test]
    fn narrow_passage_never_places_camera_through_walls() {
        let anchor = Vec3::new(0.0, 0.93, 0.0);
        let blocked = |p: Vec3| p.x.abs() > 0.3 || p.y > 1.4 || p.y < 0.1;
        let resolved = resolve_camera_with(anchor, anchor + Vec3::new(2.0, 1.0, 0.0), blocked);
        assert!(!blocked(resolved));
        assert!(resolved.z.abs() > 1.2);
    }
}

#[cfg(test)]
mod boom_cache_tests {
    use super::*;
    #[test]
    fn unchanged_sweeps_reuse_the_solution_and_motion_or_seed_rechecks() {
        let mut cache = BoomCache::default();
        let mut key = BoomKey {
            anchor: Vec3::Y,
            desired: Vec3::Y + Vec3::Z * 2.,
            seed: 42,
            colliders: crate::world::orcs::camera_colliders(Vec3::Y, 2.1).unwrap(),
        };
        assert_eq!(cache.solve(key, || Vec3::Z), Vec3::Z);
        assert_eq!(
            cache.solve(key, || panic!("unchanged collision solution")),
            Vec3::Z
        );
        key.desired.x += 0.001;
        assert_eq!(cache.solve(key, || Vec3::X), Vec3::X);
        key.anchor.y += 0.001;
        assert_eq!(cache.solve(key, || Vec3::Y), Vec3::Y);
        key.seed += 1;
        assert_eq!(cache.solve(key, || Vec3::ZERO), Vec3::ZERO);
    }
}
