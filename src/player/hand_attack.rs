//! A short, authored punch curve. Input buffering accepts a click late in recovery.
use crate::{
    app::GameState,
    player::camera::{CameraMode, CameraRig},
    ui::chat::ChatState,
};
use bevy::prelude::*;

pub(super) const DURATION: f32 = 0.44;
pub(super) const BODY_DURATION: f32 = 0.60;
// Corresponding preparation, extension, contact, recoil and rest landmarks.
const PHASES: [(f32, f32); 6] = [
    (0.0, 0.0),
    (0.075, 0.10),
    (0.145, 0.22),
    (0.17, 0.235),
    (0.265, 0.37),
    (DURATION, BODY_DURATION),
];
fn remap_time(t: f32, to_body: bool) -> f32 {
    for pair in PHASES.windows(2) {
        let ((a, x), (b, y)) = if to_body {
            (pair[0], pair[1])
        } else {
            ((pair[0].1, pair[0].0), (pair[1].1, pair[1].0))
        };
        if t <= b {
            return x + (y - x) * ((t - a) / (b - a)).clamp(0.0, 1.0);
        }
    }
    if to_body { BODY_DURATION } else { DURATION }
}
#[derive(Resource, Default)]
pub(super) struct Punch {
    elapsed: Option<f32>,
    queued: bool,
    capture_ready: bool,
    exterior: bool,
}
#[derive(Default)]
pub(super) struct Pose {
    pub translation: Vec3,
    pub rotation: Vec3,
    pub clench: f32,
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
impl Punch {
    fn advance(&mut self, dt: f32, clicked: bool) {
        let duration = if self.exterior {
            BODY_DURATION
        } else {
            DURATION
        };
        let buffer_start = if self.exterior { 0.37 } else { 0.27 };
        if clicked {
            match self.elapsed {
                None => self.elapsed = Some(0.0),
                Some(t) if t >= buffer_start => self.queued = true,
                _ => {}
            }
        }
        if let Some(t) = self.elapsed.as_mut() {
            *t += dt;
            if *t >= duration {
                self.elapsed = if self.queued {
                    Some((*t - duration).min(0.05))
                } else {
                    None
                };
                self.queued = false;
            }
        }
    }
    #[cfg(test)]
    pub fn elapsed(&self) -> Option<f32> {
        self.elapsed
    }

    pub fn body_elapsed(&self) -> Option<f32> {
        self.elapsed.map(|t| {
            if self.exterior {
                t
            } else {
                remap_time(t, true)
            }
        })
    }

    fn view_elapsed(&self) -> Option<f32> {
        self.elapsed.map(|t| {
            if self.exterior {
                remap_time(t, false)
            } else {
                t
            }
        })
    }

    pub fn body_weight(&self) -> f32 {
        self.body_elapsed().map_or(0.0, |t| {
            let ease = |t: f32| {
                let t = t.clamp(0.0, 1.0);
                (t * t * t * (t * (t * 6.0 - 15.0) + 10.0)).clamp(0.0, 1.0)
            };
            ease(t / 0.05) * (1.0 - ease((t - 0.50) / 0.10))
        })
    }

    pub fn pose(&self) -> Pose {
        let Some(t) = self.view_elapsed() else {
            return Pose::default();
        };
        // Clench before extension. Keep the fist closed through the recoil.
        let clench = smooth(t / 0.065) * (1.0 - smooth((t - 0.29) / 0.15));
        let wind = Vec3::new(0.035, -0.018, 0.055);
        let hit = Vec3::new(-0.145, 0.135, -0.20);
        let recoil = Vec3::new(-0.07, 0.025, 0.018);
        let wind_rot = Vec3::new(0.13, -0.12, 0.14);
        let hit_rot = Vec3::new(-0.30, -0.16, -0.30);
        let recoil_rot = Vec3::new(-0.25, 0.08, -0.07);
        let (translation, rotation) = if t < 0.075 {
            let u = smooth(t / 0.075);
            (wind * u, wind_rot * u)
        } else if t < 0.145 {
            // Most of the forward travel happens in the first half of 70 ms.
            let u = 1.0 - (1.0 - (t - 0.075) / 0.070).powi(3);
            (wind.lerp(hit, u), wind_rot.lerp(hit_rot, u))
        } else if t < 0.17 {
            let u = smooth((t - 0.145) / 0.025);
            (
                hit + Vec3::new(0.0, -0.004, 0.006) * u,
                hit_rot + Vec3::X * 0.025 * u,
            )
        } else if t < 0.265 {
            let u = smooth((t - 0.17) / 0.095);
            (
                (hit + Vec3::new(0.0, -0.004, 0.006)).lerp(recoil, u),
                (hit_rot + Vec3::X * 0.025).lerp(recoil_rot, u),
            )
        } else {
            let u = smooth((t - 0.265) / (DURATION - 0.265));
            (recoil * (1.0 - u), recoil_rot * (1.0 - u))
        };
        Pose {
            translation,
            rotation,
            clench,
        }
    }
}
pub(super) fn update(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    rig: Res<CameraRig>,
    game: Res<GameState>,
    chat: Res<ChatState>,
    mut punch: ResMut<Punch>,
) {
    let allowed = !rig.is_spectating() && rig.mouse_captured && !chat.blocks_gameplay();
    let clicked =
        allowed && punch.capture_ready && !game.paused && buttons.just_pressed(MouseButton::Left);
    punch.capture_ready = allowed && !game.paused;
    if game.paused {
        return;
    }
    if !allowed {
        punch.elapsed = None;
        punch.queued = false;
        return;
    }
    if punch.elapsed.is_none() {
        punch.exterior = rig.camera_mode != CameraMode::FirstPerson;
    }
    punch.advance(time.delta_secs().min(0.05), clicked);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::camera::CameraMode;
    #[test]
    fn camera_views_share_strike_landmarks_with_continuous_exterior_strike() {
        for (view, body) in PHASES {
            assert!((remap_time(view, true) - body).abs() < 0.00001);
            assert!((remap_time(body, false) - view).abs() < 0.00001);
        }
        let mut exterior = Punch {
            exterior: true,
            ..default()
        };
        exterior.advance(0.22, true);
        let mut view = Punch::default();
        view.advance(0.145, true);
        assert!(
            exterior
                .pose()
                .translation
                .distance(view.pose().translation)
                < 0.00001
        );
        assert_eq!(exterior.pose().clench, 1.0);
        exterior.advance(0.13, false);
        assert!(exterior.elapsed().is_some());
        exterior.advance(0.33, false);
        assert!(exterior.elapsed().is_none());
        view.advance(0.30, false);
        assert!(view.elapsed().is_none());
    }

    #[test]
    fn closes_before_fast_strike_and_returns_exactly_to_rest() {
        let mut punch = Punch::default();
        punch.advance(0.07, true);
        assert_eq!(punch.pose().clench, 1.0);
        punch.advance(0.075, false);
        assert!(punch.pose().translation.z < -0.19);
        assert_eq!(punch.pose().clench, 1.0);
        punch.advance(DURATION, false);
        assert_eq!(punch.pose().translation, Vec3::ZERO);
        assert_eq!(punch.pose().clench, 0.0);
    }
    #[test]
    fn early_clicks_do_not_restart_and_late_click_buffers_only_one_punch() {
        let mut punch = Punch::default();
        punch.advance(0.10, true);
        punch.advance(0.10, true);
        assert!((punch.elapsed.unwrap() - 0.20).abs() < 0.0001);
        assert!(!punch.queued);
        punch.advance(0.10, false);
        punch.advance(0.01, true);
        assert!(punch.queued);
        punch.advance(0.14, false);
        assert!(punch.elapsed.unwrap() < 0.02);
        assert!(!punch.queued);
        punch.advance(DURATION, false);
        assert!(punch.elapsed.is_none());
    }
    #[test]
    fn pose_is_continuous_at_every_phase_boundary() {
        for t in [0.075, 0.145, 0.17, 0.265, DURATION] {
            let a = Punch {
                elapsed: Some(t - 0.00001),
                ..default()
            }
            .pose();
            let b = Punch {
                elapsed: Some(t + 0.00001),
                ..default()
            }
            .pose();
            assert!(a.translation.distance(b.translation) < 0.001);
            assert!(a.rotation.distance(b.rotation) < 0.001);
        }
    }
    #[test]
    fn camera_changes_preserve_the_strike_and_spectating_cancels_it() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.02));
        app.insert_resource(time)
            .init_resource::<Punch>()
            .init_resource::<CameraRig>()
            .init_resource::<GameState>()
            .init_resource::<ChatState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, update);
        for mode in [
            CameraMode::FirstPerson,
            CameraMode::SecondPerson,
            CameraMode::ThirdPerson,
        ] {
            app.world_mut().resource_mut::<CameraRig>().camera_mode = mode;
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            assert!(app.world().resource::<Punch>().elapsed().is_some());
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .reset_all();
            let before = app.world().resource::<Punch>().elapsed().unwrap();
            app.world_mut().resource_mut::<CameraRig>().camera_mode = CameraMode::FirstPerson;
            app.update();
            assert!(app.world().resource::<Punch>().elapsed().unwrap() > before);
            for _ in 0..40 {
                app.update();
            }
            assert!(app.world().resource::<Punch>().elapsed().is_none());
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<CameraRig>()
            .toggle_spectator();
        app.update();
        assert!(app.world().resource::<Punch>().elapsed().is_none());
    }

    #[test]
    fn held_click_is_single_punch_and_pause_preserves_clench() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        app.insert_resource(time)
            .init_resource::<Punch>()
            .init_resource::<CameraRig>()
            .init_resource::<GameState>()
            .init_resource::<ChatState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, update);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        assert!(app.world().resource::<Punch>().elapsed.is_some());
        let before = app.world().resource::<Punch>().elapsed;
        app.world_mut().resource_mut::<GameState>().paused = true;
        app.world_mut().resource_mut::<CameraRig>().mouse_captured = false;
        app.update();
        assert_eq!(app.world().resource::<Punch>().elapsed, before);
        app.world_mut().resource_mut::<GameState>().paused = false;
        app.world_mut().resource_mut::<CameraRig>().mouse_captured = true;
        for _ in 0..60 {
            app.update();
        }
        assert!(app.world().resource::<Punch>().elapsed.is_none());
        app.world_mut().resource_mut::<CameraRig>().camera_mode = CameraMode::ThirdPerson;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<Punch>().elapsed.is_some());
    }
}
