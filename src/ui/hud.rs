use crate::app::settings::GraphicsSettings;
use bevy::diagnostic::DiagnosticsStore;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct FpsCounter;

pub(crate) fn update_fps_counter(
    time: Res<Time>,
    settings: Res<GraphicsSettings>,
    diagnostics: Res<DiagnosticsStore>,
    mut refresh_elapsed: Local<f32>,
    mut counter: Single<(&mut Text, &mut Visibility), With<FpsCounter>>,
) {
    let (text, visibility) = &mut *counter;
    visibility.set_if_neq(if settings.show_fps {
        Visibility::Visible
    } else {
        Visibility::Hidden
    });

    if !settings.show_fps {
        return;
    }

    *refresh_elapsed += time.delta_secs();
    if *refresh_elapsed < 0.2 {
        return;
    }
    *refresh_elapsed = 0.0;

    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
    {
        text.set_if_neq(Text::new(format!("{fps:.0}")));
    }
}
