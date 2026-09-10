//! Local spectator mode: free flight with safe, grounded-controller re-entry.
use crate::player::camera::CameraRig;
use crate::player::movement::PLAYER_EYE_HEIGHT;
use bevy::prelude::*;

const FLY_SPEED: f32 = 20.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum GameMode {
    #[default]
    Player,
    Spectator {
        fallback: Vec3,
    },
}

impl CameraRig {
    pub(crate) fn is_spectating(&self) -> bool {
        matches!(self.game_mode, GameMode::Spectator { .. })
    }

    /// Spectators have no body contact shadow or grass-trampling influence.
    pub(crate) fn ground_influence_height(&self) -> f32 {
        if self.is_spectating() {
            1000.0
        } else {
            (self.jump_height - crate::world::biome::terrain_height(self.position.xz())).max(0.0)
        }
    }

    pub(crate) fn toggle_spectator(&mut self) {
        match self.game_mode {
            GameMode::Player => {
                self.game_mode = GameMode::Spectator {
                    fallback: self.position,
                };
                self.grounded = false;
            }
            GameMode::Spectator { fallback } => {
                self.position = reentry_position(self.position, fallback);
                self.jump_height = self.position.y - PLAYER_EYE_HEIGHT;
                self.grounded = crate::player::movement::terrain_allows_standing(self.position)
                    && (self.jump_height
                        - crate::player::movement::world_support_height(
                            self.position,
                            crate::player::movement::PLAYER_RADIUS,
                        ))
                    .abs()
                        < 0.006;
                self.game_mode = GameMode::Player;
            }
        }
        self.vertical_velocity = 0.0;
        self.jump_buffer_remaining = 0.0;
        self.coyote_remaining = if self.grounded {
            crate::player::movement::COYOTE_SECONDS
        } else {
            0.0
        };
        self.landing_offset = 0.0;
        self.landing_offset_velocity = 0.0;
    }
}

/// The walking controller uses ground-projected collisions, so clear that
/// projection before restoring it. Preserve height and fall normally in air.
fn reentry_position(position: Vec3, fallback: Vec3) -> Vec3 {
    if crate::world::goblin_dens::support(position, crate::player::movement::PLAYER_RADIUS)
        .is_some()
        && !crate::player::movement::player_collides(position)
    {
        return position;
    }
    let p = position.with_y(
        position
            .y
            .max(crate::world::biome::terrain_height(position.xz()) + PLAYER_EYE_HEIGHT),
    );
    let clear = |p: Vec3| {
        !crate::player::movement::player_collides(p)
            && crate::player::movement::gate_ceiling_at(p)
                .is_none_or(|ceiling| p.y < ceiling - 0.05)
    };
    if clear(p) {
        return p;
    }
    for ring in 1..=24 {
        let radius = ring as f32 * 0.25;
        for i in 0..32 {
            let angle = i as f32 * std::f32::consts::TAU / 32.0;
            let candidate = p + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
            if clear(candidate) {
                return candidate;
            }
        }
    }
    // The world is static; the position occupied before entering is safe.
    fallback.with_y(fallback.y.max(PLAYER_EYE_HEIGHT))
}

pub(crate) fn fly(
    rig: &mut CameraRig,
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    dt: f32,
) {
    let (forward, right, _) = rig.basis();
    let mut direction = Vec3::ZERO;
    for (key, axis) in [
        (KeyCode::KeyW, forward),
        (KeyCode::KeyS, -forward),
        (KeyCode::KeyD, right),
        (KeyCode::KeyA, -right),
        (KeyCode::Space, Vec3::Y),
        (KeyCode::ShiftLeft, -Vec3::Y),
    ] {
        if keys.pressed(key) {
            direction += axis;
        }
    }
    let speed = FLY_SPEED
        * if buttons.pressed(MouseButton::Left) {
            2.0
        } else {
            1.0
        };
    rig.position += direction.normalize_or_zero() * speed * dt.min(0.1);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flight_is_camera_relative_normalized_and_boost_is_exactly_double() {
        let mut rig = CameraRig::default();
        rig.toggle_spectator();
        let origin = rig.position;
        let mut keys = ButtonInput::default();
        let mut buttons = ButtonInput::default();
        keys.press(KeyCode::KeyW);
        fly(&mut rig, &keys, &buttons, 0.05);
        let delta = rig.position - origin;
        assert!((delta.length() - 1.0).abs() < 1e-5);
        assert!(delta.y > 0.0); // Looking upward flies upward, not along the ground.
        rig.position = origin;
        buttons.press(MouseButton::Left);
        fly(&mut rig, &keys, &buttons, 0.05);
        assert!((rig.position - origin - delta * 2.0).length() < 1e-5);
        keys.press(KeyCode::KeyD);
        keys.press(KeyCode::Space);
        rig.position = origin;
        fly(&mut rig, &keys, &buttons, 0.05);
        assert!(((rig.position - origin).length() - 2.0).abs() < 1e-5);
        keys.reset_all();
        buttons.reset_all();
        keys.press(KeyCode::ShiftLeft);
        rig.position = origin;
        fly(&mut rig, &keys, &buttons, 0.05);
        assert!((rig.position - origin + Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn flight_crosses_walls_floor_and_tree_and_reentry_is_safe() {
        for obstruction in [
            Vec3::new(4.3, 1.25, 0.0),
            Vec3::new(0.0, 1.25, 0.0),
            Vec3::new(0.0, -5.0, 3.0),
        ] {
            let mut rig = CameraRig::default();
            rig.toggle_spectator();
            rig.position = obstruction;
            let before = rig.position;
            let mut keys = ButtonInput::default();
            keys.press(KeyCode::KeyD);
            fly(&mut rig, &keys, &ButtonInput::default(), 0.01);
            assert!((rig.position.x - before.x - 0.2).abs() < 1e-5);
            rig.position = obstruction;
            rig.toggle_spectator();
            assert!(!rig.is_spectating());
            assert!(!crate::player::movement::player_collides(rig.position));
            assert!(rig.position.y >= PLAYER_EYE_HEIGHT);
            assert_eq!(rig.vertical_velocity, 0.0);
        }
    }

    #[test]
    fn exiting_in_air_restores_gravity_and_preserves_camera_preference() {
        let mut rig = CameraRig {
            camera_mode: crate::player::camera::CameraMode::ThirdPerson,
            ..default()
        };
        rig.toggle_spectator();
        assert_eq!(rig.view().position, rig.position);
        assert_eq!(rig.ground_influence_height(), 1000.0);
        rig.position.y = 10.0;
        rig.toggle_spectator();
        assert_eq!(
            rig.camera_mode,
            crate::player::camera::CameraMode::ThirdPerson
        );
        assert_eq!(rig.position.y, 10.0);
        assert!(!rig.grounded);
        crate::player::movement::update_jump(&mut rig, &ButtonInput::default(), 0.016);
        assert!(rig.position.y < 10.0);
        assert!(rig.vertical_velocity < 0.0);
    }
}
