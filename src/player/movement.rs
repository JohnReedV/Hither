use crate::app::GameState;
use crate::app::settings::GraphicsSettings;
use crate::player::camera::CameraRig;
use crate::player::spectator;
use crate::ui::chat;
use crate::world::orchard;
use crate::world::terrain;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

pub(crate) const WALK_SPEED: f32 = 3.8;
pub(crate) const PLAYER_EYE_HEIGHT: f32 = 1.25;
pub(crate) const PLAYER_RADIUS: f32 = 0.28;
pub(crate) const MAX_COLLISION_STEP: f32 = 0.08;
const MAX_TERRAIN_GRADIENT: f32 = 1.0; // tan(45 degrees)
pub(crate) const JUMP_SPEED: f32 = 5.5;
pub(crate) const JUMP_GRAVITY: f32 = 19.0;
pub(crate) const FALL_GRAVITY_MULTIPLIER: f32 = 1.28;
pub(crate) const RELEASE_GRAVITY_MULTIPLIER: f32 = 2.15;
pub(crate) const APEX_GRAVITY_MULTIPLIER: f32 = 0.72;
pub(crate) const JUMP_BUFFER_SECONDS: f32 = 0.13;
pub(crate) const COYOTE_SECONDS: f32 = 0.11;
pub(crate) const LANDING_SPRING_STRENGTH: f32 = 145.0;
pub(crate) const LANDING_SPRING_DAMPING: f32 = 19.0;
pub(crate) const BASE_MOUSE_SENSITIVITY: f32 = 0.0022;
pub(crate) fn player_collides(position: Vec3) -> bool {
    character_collides(position, PLAYER_RADIUS)
}

pub(crate) fn character_collides(position: Vec3, radius: f32) -> bool {
    if let Some(clear) = crate::world::goblin_dens::body_clear(
        position - Vec3::Y * PLAYER_EYE_HEIGHT,
        radius,
        PLAYER_EYE_HEIGHT + 0.3,
    ) {
        return !clear;
    }
    if position.y < PLAYER_EYE_HEIGHT - 0.15
        && crate::world::orcs::surface_height(position, radius).is_some()
    {
        return crate::world::orcs::underground_collision(position, radius);
    }
    if crate::world::orcs::underground_collision(position, radius)
        || orchard::logs::collides(
            position - Vec3::Y * PLAYER_EYE_HEIGHT,
            radius,
            PLAYER_EYE_HEIGHT,
        )
        || orchard::wild::player_collides(position, radius)
        || orchard::winter::player_collides(position, radius)
        || orchard::alpine::player_collides(position, radius)
        || orchard::oak::player_collides(position, radius)
        || crate::world::geology::collides(
            position - Vec3::Y * PLAYER_EYE_HEIGHT,
            radius,
            PLAYER_EYE_HEIGHT + 0.3,
        )
    {
        return true;
    }

    false
}

pub(crate) fn move_player_with_collisions(mut position: Vec3, displacement: Vec3) -> Vec3 {
    let step_count = (displacement.length() / MAX_COLLISION_STEP).ceil().max(1.0) as usize;
    let step = displacement / step_count as f32;

    for _ in 0..step_count {
        for offset in [Vec3::X * step.x, Vec3::Z * step.z] {
            if offset.length_squared() == 0. {
                continue;
            }
            if let Some(candidate) = supported_player_step(position, offset) {
                position = candidate;
            } else if crate::world::orcs::surface_height(position, PLAYER_RADIUS).is_some() {
                // Axis-only motion pins a body against curved banks and rock
                // corners. Find the closest collision-free sliding direction;
                // every candidate keeps forward progress and uses the same
                // short body/floor sweep, without snapping to a route or lane.
                'slide: for angle in [0.26_f32, 0.52, 0.78, 1.04, 1.30] {
                    for sign in [-1., 1.] {
                        let slide = Quat::from_rotation_y(angle * sign) * offset;
                        if let Some(candidate) = supported_player_step(position, slide) {
                            position = candidate;
                            break 'slide;
                        }
                    }
                }
            }
        }
    }

    position
}

fn supported_player_step(position: Vec3, offset: Vec3) -> Option<Vec3> {
    let mut candidate = position + offset;
    let floor = world_support_height(position, PLAYER_RADIUS);
    let next_floor = world_support_height(candidate, PLAYER_RADIUS);
    let feet = position.y - PLAYER_EYE_HEIGHT;
    // An airborne body can hit the side of a rising bank. Do not let
    // horizontal motion bury its feet beyond landing's step tolerance;
    // that leaves gravity below the only supporting surface.
    if next_floor > feet + 0.20 + 0.001 {
        return None;
    }
    // Reject uphill terrain penetration before grounding can snap the feet
    // onto a steep face. Skipping the step-up alone still lets update_jump
    // raise the player every frame, making cliffs climbable at low speed.
    // Den banks have their own traversal/recovery rules; preserve those.
    if next_floor > floor && next_floor > feet && steep_terrain_gradient(candidate).is_some() {
        return None;
    }
    if feet >= floor - 0.02
        && feet <= floor + 0.06
        && next_floor > feet
        && next_floor - feet <= 0.20
        && walkable_support_slope(candidate)
    {
        candidate.y = next_floor + PLAYER_EYE_HEIGHT;
    }
    // Raising onto a step can change which side of a lid supplies
    // support. Validate again at the final body height.
    if candidate.y != position.y
        && world_support_height(candidate, PLAYER_RADIUS)
            > candidate.y - PLAYER_EYE_HEIGHT + 0.20 + 0.001
    {
        return None;
    }
    (!player_collides(candidate)).then_some(candidate)
}

pub(crate) fn world_support_height(p: Vec3, radius: f32) -> f32 {
    if let Some(y) = crate::world::goblin_dens::support(p, radius) {
        return y;
    }
    let base = crate::world::orcs::surface_height(p, radius)
        .unwrap_or_else(|| orchard::oak::support_height(p, radius));
    base.max(crate::world::geology::support(p, radius).unwrap_or(base))
        .max(orchard::logs::support(p, radius).unwrap_or(base))
}

fn walkable_support_slope(p: Vec3) -> bool {
    if steep_terrain_gradient(p).is_some() {
        return false;
    }
    // Fallen trunks use the same small step allowance as other low obstacles.
    if orchard::logs::support(p, PLAYER_RADIUS).is_some_and(|h| h > terrain::height(p.xz()) + 0.001)
    {
        return true;
    }
    // Low grate edges are ordinary step-ups, not steep terrain faces.
    if crate::world::orcs::entrance_overlap(p, PLAYER_RADIUS) {
        return true;
    }
    let sample = |offset| world_support_height(p + offset, PLAYER_RADIUS);
    let dx = (sample(Vec3::X * 0.04) - sample(-Vec3::X * 0.04)) / 0.08;
    let dz = (sample(Vec3::Z * 0.04) - sample(-Vec3::Z * 0.04)) / 0.08;
    dx * dx + dz * dz <= 1.0 // 45 degrees, independent of movement speed.
}

/// A cliff is a collision surface, but cannot provide standing/jump support.
/// Raised roots and den floors keep their own existing support rules.
fn steep_terrain_gradient(p: Vec3) -> Option<Vec2> {
    if crate::world::goblin_dens::support(p, PLAYER_RADIUS).is_some() {
        return None;
    }
    if crate::world::orcs::surface_height(p, PLAYER_RADIUS).is_some()
        || orchard::oak::support_height(p, PLAYER_RADIUS) > terrain::height(p.xz()) + 0.001
        || orchard::logs::support(p, PLAYER_RADIUS)
            .is_some_and(|h| h > terrain::height(p.xz()) + 0.001)
    {
        return None;
    }
    if crate::world::geology::support(p, PLAYER_RADIUS)
        .is_some_and(|h| h > terrain::height(p.xz()) + 0.02)
    {
        let sample = |offset| world_support_height(p + offset, PLAYER_RADIUS);
        let gradient = Vec2::new(
            sample(Vec3::X * 0.04) - sample(-Vec3::X * 0.04),
            sample(Vec3::Z * 0.04) - sample(-Vec3::Z * 0.04),
        ) / 0.08;
        return (gradient.length_squared() > MAX_TERRAIN_GRADIENT.powi(2)).then_some(gradient);
    }
    let gradient = terrain::gradient(p.xz());
    (gradient.length_squared() > MAX_TERRAIN_GRADIENT.powi(2)).then_some(gradient)
}

pub(crate) fn terrain_allows_standing(p: Vec3) -> bool {
    steep_terrain_gradient(p).is_none()
}

fn slide_on_terrain(rig: &mut CameraRig, gradient: Vec2, support: f32, dt: f32) {
    rig.grounded = false;
    rig.coyote_remaining = 0.0;
    // Project downward travel onto the face. Keep downward velocity so gravity
    // continues accelerating the slide, rather than restarting it each frame.
    let downhill = gradient * (rig.vertical_velocity * dt / (1.0 + gradient.length_squared()));
    let contact = rig.position.with_y(support + PLAYER_EYE_HEIGHT);
    let end = move_player_with_collisions(contact, Vec3::new(downhill.x, 0.0, downhill.y));
    rig.position.x = end.x;
    rig.position.z = end.z;
    // Stay outside the mesh, but fall normally if the face ends at a drop.
    rig.jump_height = rig
        .jump_height
        .max(world_support_height(end, PLAYER_RADIUS));
}

pub(crate) fn gate_ceiling_at(position: Vec3) -> Option<f32> {
    if let Some(top) = crate::world::goblin_dens::ceiling(position) {
        return Some(top - 0.3);
    }
    if let Some(ceiling) = crate::world::orcs::ceiling_at(position) {
        return Some(ceiling - 0.40);
    }
    None
}

pub(crate) fn update_jump(rig: &mut CameraRig, keys: &ButtonInput<KeyCode>, delta_seconds: f32) {
    let delta_seconds = delta_seconds.min(0.05);
    recover_den_overlap(rig);
    // Camera landing bob must not change which side of a den roof or lip
    // supplies support. Physics queries use the actual feet throughout.
    let physics_position = rig.position.with_y(PLAYER_EYE_HEIGHT + rig.jump_height);
    let support = world_support_height(physics_position, PLAYER_RADIUS);
    let steep_gradient = steep_terrain_gradient(physics_position);
    if steep_gradient.is_some() && rig.jump_height <= support + 0.06 {
        // Also revoke stale grounding/coyote time after spawning or leaving
        // spectator mode on a face, before processing any buffered jump.
        rig.grounded = false;
        rig.coyote_remaining = 0.0;
    }
    if rig.grounded && (steep_gradient.is_some() || rig.jump_height > support + 0.06) {
        rig.grounded = false;
    }
    if rig.grounded {
        rig.jump_height = support;
    }
    let previous_height = rig.jump_height;

    if keys.just_pressed(KeyCode::Space) {
        rig.jump_buffer_remaining = JUMP_BUFFER_SECONDS;
    } else {
        rig.jump_buffer_remaining = (rig.jump_buffer_remaining - delta_seconds).max(0.0);
    }

    if rig.grounded {
        rig.coyote_remaining = COYOTE_SECONDS;
    } else {
        rig.coyote_remaining = (rig.coyote_remaining - delta_seconds).max(0.0);
    }

    if rig.jump_buffer_remaining > 0.0 && rig.coyote_remaining > 0.0 {
        rig.grounded = false;
        rig.vertical_velocity = JUMP_SPEED;
        rig.jump_buffer_remaining = 0.0;
        rig.coyote_remaining = 0.0;
        rig.landing_offset_velocity += 0.35;
    }

    if keys.just_released(KeyCode::Space) && rig.vertical_velocity > 0.0 {
        rig.vertical_velocity *= 0.48;
    }

    if !rig.grounded {
        let gravity_multiplier = if rig.vertical_velocity < 0.0 {
            FALL_GRAVITY_MULTIPLIER
        } else if !keys.pressed(KeyCode::Space) {
            RELEASE_GRAVITY_MULTIPLIER
        } else if rig.vertical_velocity.abs() < 1.0 {
            APEX_GRAVITY_MULTIPLIER
        } else {
            1.0
        };

        rig.vertical_velocity -= JUMP_GRAVITY * gravity_multiplier * delta_seconds;
        rig.jump_height += rig.vertical_velocity * delta_seconds;

        if let Some(ceiling) = gate_ceiling_at(physics_position) {
            let maximum_jump_height = ceiling - PLAYER_EYE_HEIGHT;
            if rig.jump_height > maximum_jump_height {
                rig.jump_height = maximum_jump_height;
                rig.vertical_velocity = rig.vertical_velocity.min(0.0);
            }
        }

        if let Some(gradient) = steep_gradient
            && rig.jump_height <= support
            && rig.vertical_velocity <= 0.0
        {
            slide_on_terrain(rig, gradient, support, delta_seconds);
        } else if steep_gradient.is_none() && rig.jump_height <= support
            // Recover small floor penetrations using the same allowance as
            // walking up a step. Horizontal motion and changing den support
            // can put feet just below the floor before gravity runs.
            && previous_height >= support - 0.20 - 0.001
            && rig.vertical_velocity <= 0.0
        {
            let impact_speed = -rig.vertical_velocity;
            rig.jump_height = support;
            rig.vertical_velocity = 0.0;
            rig.grounded = true;
            rig.coyote_remaining = COYOTE_SECONDS;
            if impact_speed > 1.0 {
                rig.landing_offset = -(impact_speed * 0.012).clamp(0.025, 0.085);
                rig.landing_offset_velocity = 0.0;
            }
        }
    }

    let spring_acceleration = -LANDING_SPRING_STRENGTH * rig.landing_offset
        - LANDING_SPRING_DAMPING * rig.landing_offset_velocity;
    rig.landing_offset_velocity += spring_acceleration * delta_seconds;
    rig.landing_offset += rig.landing_offset_velocity * delta_seconds;
    if rig.landing_offset.abs() < 0.0001 && rig.landing_offset_velocity.abs() < 0.001 {
        rig.landing_offset = 0.0;
        rig.landing_offset_velocity = 0.0;
    }

    recover_den_overlap(rig);
    rig.position.y = PLAYER_EYE_HEIGHT + rig.jump_height + rig.landing_offset;
}

fn recover_den_overlap(rig: &mut CameraRig) {
    let physics = rig.position.with_y(PLAYER_EYE_HEIGHT + rig.jump_height);
    if let Some(clear) = crate::world::orcs::recover_player_position(physics, PLAYER_RADIUS) {
        rig.position = clear;
        rig.jump_height = clear.y - PLAYER_EYE_HEIGHT;
        rig.vertical_velocity = 0.;
        rig.grounded = terrain_allows_standing(clear)
            && (rig.jump_height - world_support_height(clear, PLAYER_RADIUS)).abs() < 0.006;
        rig.landing_offset = 0.;
        rig.landing_offset_velocity = 0.;
    }
}

#[allow(clippy::too_many_arguments)] // Independent Bevy input resources.
pub(crate) fn move_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    game: Res<GameState>,
    chat: Res<chat::ChatState>,
    settings: Res<GraphicsSettings>,
    mut rig: ResMut<CameraRig>,
) {
    if game.paused {
        return;
    }

    if chat.blocks_gameplay() {
        // Typing suppresses controls, not gravity or the simulation.
        rig.jump_buffer_remaining = 0.0;
        if !rig.is_spectating() {
            update_jump(&mut rig, &ButtonInput::default(), time.delta_secs());
        }
        return;
    }

    if rig.mouse_captured {
        let sensitivity = BASE_MOUSE_SENSITIVITY * settings.mouse_sensitivity;
        rig.yaw += mouse_motion.delta.x * sensitivity;
        rig.pitch = (rig.pitch - mouse_motion.delta.y * sensitivity).clamp(-1.5, 1.5);
    }

    if rig.is_spectating() {
        if rig.mouse_captured {
            spectator::fly(&mut rig, &keys, &buttons, time.delta_secs());
        }
        return;
    }

    let (forward, right, _) = rig.basis();
    let flat_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let mut direction = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        direction += flat_forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        direction -= flat_forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        direction -= right;
    }
    let displacement = direction.normalize_or_zero() * WALK_SPEED * time.delta_secs();
    // Landing bob is visual only: never let it push the collision feet into a root.
    let old_height = rig.jump_height;
    let physics_position = Vec3::new(
        rig.position.x,
        PLAYER_EYE_HEIGHT + old_height,
        rig.position.z,
    );
    rig.position = move_player_with_collisions(physics_position, displacement);
    rig.jump_height = rig.position.y - PLAYER_EYE_HEIGHT;
    update_jump(&mut rig, &keys, time.delta_secs());
}

#[cfg(test)]
mod player_slope_tests {
    use super::*;
    use crate::world::{biome, terrain};

    // Use actual mountain triangles and the full support/collision queries.
    // Stay inside a triangle so the expected gradient is exact.
    fn mountain_slope(steep: bool) -> (Vec3, Vec3) {
        for z in (-3000..3000).step_by(29) {
            for x in (-3000..3000).step_by(29) {
                let cell = Vec2::new(x as f32, z as f32);
                let p = cell + Vec2::splat(0.25);
                if biome::mountain_amount(p) < 0.95 {
                    continue;
                }
                let a = terrain::height(cell);
                let gradient = Vec2::new(
                    terrain::height(cell + Vec2::X) - a,
                    terrain::height(cell + Vec2::Y) - a,
                );
                let slope = gradient.length();
                let range = if steep { 1.5..3.0 } else { 0.2..0.8 };
                if !range.contains(&slope) {
                    continue;
                }
                let position = Vec3::new(p.x, terrain::height(p) + PLAYER_EYE_HEIGHT, p.y);
                let uphill = if gradient.x.abs() > gradient.y.abs() {
                    Vec3::X * gradient.x.signum()
                } else {
                    Vec3::Z * gradient.y.signum()
                };
                if [-0.08, 0.0, 0.08].into_iter().all(|distance| {
                    let q = position + uphill * distance;
                    let height = terrain::height(q.xz());
                    (world_support_height(q, PLAYER_RADIUS) - height).abs() < 0.001
                        && !player_collides(q.with_y(height + PLAYER_EYE_HEIGHT))
                }) {
                    return (position, uphill);
                }
            }
        }
        panic!("no suitable mountain slope found");
    }

    #[test]
    fn steep_mountains_block_uphill_motion_at_small_and_large_timesteps() {
        let (start, uphill) = mountain_slope(true);
        for distance in [0.001, 0.01, 0.04, 0.08, 0.3] {
            let end = move_player_with_collisions(start, uphill * distance);
            assert_eq!(end, start, "step size {distance}");
        }
    }

    #[test]
    fn steep_contact_revokes_grounding_and_coyote_before_jump_input() {
        let (start, _) = mountain_slope(true);
        let mut rig = CameraRig {
            position: start,
            jump_height: start.y - PLAYER_EYE_HEIGHT,
            ..default()
        };
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Space);
        for _ in 0..6 {
            update_jump(&mut rig, &keys, 1.0 / 60.0);
            assert!(!rig.grounded);
            assert_eq!(rig.coyote_remaining, 0.0);
            assert!(rig.vertical_velocity < 0.0);
            assert!(rig.jump_height >= terrain::height(rig.position.xz()) - 0.001);
        }
        assert!(rig.jump_height < start.y - PLAYER_EYE_HEIGHT);
        assert!(rig.position.xz().distance(start.xz()) > 0.01);
    }

    #[test]
    fn landing_on_steep_mountains_slides_instead_of_enabling_repeated_jumps() {
        let (start, uphill) = mountain_slope(true);
        for dt in [1.0 / 144.0, 1.0 / 60.0, 1.0 / 20.0] {
            let mut rig = CameraRig {
                position: start + Vec3::Y * 0.15,
                jump_height: start.y - PLAYER_EYE_HEIGHT + 0.15,
                vertical_velocity: -1.0,
                grounded: false,
                coyote_remaining: 0.0,
                ..default()
            };
            let mut contacted = false;
            for _ in 0..240 {
                let physics = rig.position.with_y(rig.jump_height + PLAYER_EYE_HEIGHT);
                if terrain_allows_standing(physics) {
                    break;
                }
                // Continually push uphill and spam jump, including on impact.
                rig.position = move_player_with_collisions(physics, uphill * WALK_SPEED * dt);
                rig.jump_height = rig.position.y - PLAYER_EYE_HEIGHT;
                let mut keys = ButtonInput::default();
                keys.press(KeyCode::Space);
                update_jump(&mut rig, &keys, dt);
                assert!(!rig.grounded, "cliff supplied a foothold at dt={dt}");
                assert_eq!(rig.coyote_remaining, 0.0);
                assert!(
                    rig.vertical_velocity < 0.0,
                    "jump reset on cliff at dt={dt}"
                );
                let height = terrain::height(rig.position.xz());
                assert!(rig.jump_height >= height - 0.001, "fell through terrain");
                contacted |= (rig.jump_height - height).abs() < 0.001;
                if contacted && rig.jump_height < start.y - PLAYER_EYE_HEIGHT - 0.1 {
                    break;
                }
            }
            assert!(contacted, "never contacted the face at dt={dt}");
            assert!(rig.jump_height < start.y - PLAYER_EYE_HEIGHT - 0.1);
        }
    }

    #[test]
    fn spectator_reentry_on_steep_terrain_cannot_restore_jump_support() {
        let (start, _) = mountain_slope(true);
        let mut rig = CameraRig {
            position: start,
            jump_height: start.y - PLAYER_EYE_HEIGHT,
            ..default()
        };
        rig.toggle_spectator();
        rig.toggle_spectator();
        assert!(!rig.grounded);
        assert_eq!(rig.coyote_remaining, 0.0);
        update_jump(&mut rig, &ButtonInput::default(), 0.05);
        assert!(rig.jump_height < start.y - PLAYER_EYE_HEIGHT);
    }

    #[test]
    fn gentle_mountain_landing_restores_standing_and_jumping() {
        let (start, _) = mountain_slope(false);
        let mut rig = CameraRig {
            position: start + Vec3::Y * 0.01,
            jump_height: start.y - PLAYER_EYE_HEIGHT + 0.01,
            grounded: false,
            vertical_velocity: -1.0,
            coyote_remaining: 0.0,
            ..default()
        };
        update_jump(&mut rig, &ButtonInput::default(), 0.05);
        assert!(rig.grounded);
        assert!(rig.coyote_remaining > 0.0);
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Space);
        update_jump(&mut rig, &keys, 0.05);
        assert!(!rig.grounded);
        assert!(rig.vertical_velocity > 0.0);
    }

    #[test]
    fn steep_mountains_allow_descent_and_clear_airborne_motion() {
        let (start, uphill) = mountain_slope(true);
        let downhill = move_player_with_collisions(start, -uphill * 0.04);
        assert!(downhill.xz().distance(start.xz()) > 0.03);
        let airborne = start + Vec3::Y;
        let end = move_player_with_collisions(airborne, uphill * 0.04);
        assert!(end.xz().distance(airborne.xz()) > 0.03);
        assert_eq!(end.y, airborne.y);
    }

    #[test]
    fn gentle_mountains_remain_climbable() {
        let (start, uphill) = mountain_slope(false);
        let end = move_player_with_collisions(start, uphill * 0.04);
        assert!(end.xz().distance(start.xz()) > 0.03);
        assert!(end.y > start.y);
        assert!((end.y - PLAYER_EYE_HEIGHT - terrain::height(end.xz())).abs() < 0.001);
    }
}
