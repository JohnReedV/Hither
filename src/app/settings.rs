use crate::rendering::{graphics, view_distance};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::MonitorSelection;
use bevy::window::VideoModeSelection;
use bevy::window::WindowMode;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

#[derive(Resource, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct GraphicsSettings {
    pub(crate) max_fps: u32,
    pub(crate) show_fps: bool,
    pub(crate) display_mode: DisplayMode,
    pub(crate) mouse_sensitivity: f32,
    pub(crate) show_coordinates: bool,
    pub(crate) resolution: usize,
    pub(crate) anti_aliasing: graphics::AntiAliasing,
    pub(crate) texture_quality: graphics::Quality,
    pub(crate) shadow_quality: graphics::Quality,
    pub(crate) grass_blades: bool,
    pub(crate) render_distance: f32,
    pub(crate) detail_distance: f32,
    pub(crate) shadow_distance: f32,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            max_fps: 144,
            show_fps: true,
            display_mode: DisplayMode::Fullscreen,
            mouse_sensitivity: 1.0,
            show_coordinates: false,
            resolution: 0,
            anti_aliasing: graphics::AntiAliasing::Fxaa,
            texture_quality: graphics::Quality::High,
            shadow_quality: graphics::Quality::Medium,
            grass_blades: true,
            render_distance: view_distance::DEFAULT,
            detail_distance: 24.0,
            shadow_distance: 24.0,
        }
    }
}

impl GraphicsSettings {
    pub(crate) fn load() -> Self {
        let path = settings_path();
        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(mut settings) = serde_json::from_str::<Self>(&contents) else {
            warn!("Could not read settings from {}", path.display());
            return Self::default();
        };

        // Migrate existing files without expanding their former shadow range.
        // Very large detail settings get a separate, editable 128m budget.
        if serde_json::from_str::<serde_json::Value>(&contents)
            .ok()
            .is_some_and(|v| v.get("shadow_distance").is_none())
        {
            settings.shadow_distance = settings.detail_distance.min(128.0);
        }
        settings.max_fps = settings.max_fps.clamp(1, 1000);
        settings.render_distance = if settings.render_distance.is_finite() {
            settings
                .render_distance
                .clamp(view_distance::MIN, view_distance::MAX)
                .round()
        } else {
            view_distance::DEFAULT
        };
        settings.resolution = settings.resolution.min(graphics::RESOLUTIONS.len() - 1);
        settings.detail_distance = if settings.detail_distance.is_finite() {
            settings
                .detail_distance
                .clamp(0.0, settings.render_distance)
                .round()
        } else {
            24.0_f32.min(settings.render_distance)
        };
        settings.shadow_distance = if settings.shadow_distance.is_finite() {
            settings
                .shadow_distance
                .clamp(0.0, settings.render_distance)
                .round()
        } else {
            24.0_f32.min(settings.render_distance)
        };
        if !settings.mouse_sensitivity.is_finite() {
            settings.mouse_sensitivity = 1.0;
        }
        settings.mouse_sensitivity = settings
            .mouse_sensitivity
            .clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY);
        settings
    }

    pub(crate) fn save(&self) -> std::io::Result<()> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary_path = path.with_extension("json.tmp");
        let serialized = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(&temporary_path, serialized)?;
        fs::rename(temporary_path, path)
    }
}

fn settings_path() -> PathBuf {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home)
            .join("hither-sdf")
            .join("settings.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("hither-sdf")
            .join("settings.json");
    }
    PathBuf::from("hither-settings.json")
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DisplayMode {
    Windowed,
    Borderless,
    Fullscreen,
}

impl DisplayMode {
    pub(crate) fn startup_window_mode(self) -> WindowMode {
        match self {
            Self::Windowed => WindowMode::Windowed,
            Self::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            Self::Fullscreen => {
                WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current)
            }
        }
    }

    pub(crate) fn window_mode(self) -> WindowMode {
        match self {
            Self::Windowed => WindowMode::Windowed,
            Self::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            Self::Fullscreen => {
                WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
            }
        }
    }
}
#[derive(Default)]
pub(crate) struct SettingsSaveState {
    pub(crate) pending_since: Option<Instant>,
}

pub(crate) fn save_settings(
    settings: Res<GraphicsSettings>,
    mut state: Local<SettingsSaveState>,
    mut exits: MessageReader<AppExit>,
) {
    if settings.is_changed() {
        state.pending_since = Some(Instant::now());
    }

    let exiting = exits.read().next().is_some();
    let ready = state
        .pending_since
        .is_some_and(|changed_at| changed_at.elapsed() >= Duration::from_millis(350));
    if !exiting && !ready {
        return;
    }

    if let Err(error) = settings.save() {
        warn!("Could not save Hither settings: {error}");
        state.pending_since = Some(Instant::now());
    } else {
        state.pending_since = None;
    }
}

pub(crate) const MIN_MOUSE_SENSITIVITY: f32 = 0.1;
pub(crate) const MAX_MOUSE_SENSITIVITY: f32 = 5.0;
