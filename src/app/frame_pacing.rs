use crate::app::settings::GraphicsSettings;
use bevy::prelude::*;
use std::thread;
use std::time::Duration;
use std::time::Instant;

#[derive(Resource)]
pub(crate) struct FrameLimiter {
    pub(crate) last_frame: Instant,
}
impl Default for FrameLimiter {
    fn default() -> Self {
        Self {
            last_frame: Instant::now(),
        }
    }
}
pub(crate) fn limit_frame_rate(settings: Res<GraphicsSettings>, mut limiter: ResMut<FrameLimiter>) {
    let target_frame_time = Duration::from_secs_f64(1.0 / f64::from(settings.max_fps));
    if let Some(remaining) = target_frame_time.checked_sub(limiter.last_frame.elapsed()) {
        thread::sleep(remaining);
    }
    limiter.last_frame = Instant::now();
}
