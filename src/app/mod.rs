//! Application composition and the explicit runtime schedule.
pub(crate) mod frame_pacing;
pub(crate) mod settings;

use crate::app::frame_pacing::{FrameLimiter, limit_frame_rate};
use crate::app::settings::{GraphicsSettings, save_settings};
use crate::diagnostics::forest_profile;
use crate::player::avatar;
use crate::player::camera::{CameraRig, cycle_camera_mode, update_camera_view};
use crate::player::input::{InitialCursorLock, initialize_cursor_lock, recapture_mouse};
use crate::player::movement::move_camera;
use crate::rendering::graphics;
use crate::rendering::sdf::{SdfMaterial, update_shader_uniforms};
use crate::ui::chat;
use crate::ui::hud::update_fps_counter;
use crate::ui::menu::{
    EditingFps, EditingSensitivity, focus_fps_input, focus_sensitivity_input, menu_buttons,
    pause_on_escape, sync_options_ui, type_fps_value, type_sensitivity_value, update_fps_slider,
    update_sensitivity_slider,
};
use crate::world::orchard::fruit::{OrchardState, pick_fruit};
use crate::world::vegetation::{grass, understory};
use crate::world::{biome, orchard, snow, terrain};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::PresentMode;
use std::time::Duration;

pub(crate) fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--validate-avatar") {
        let result = args
            .get(2)
            .ok_or("usage: --validate-avatar <path.glb>".to_owned())
            .and_then(|path| avatar::validate_file(path));
        match result {
            Ok(()) => println!("Valid Hither Avatar v1"),
            Err(error) => {
                eprintln!("Avatar rejected: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    let settings = GraphicsSettings::load();
    let rig = CameraRig::for_startup();
    let view = rig.view();
    let startup_window_mode = settings.display_mode.startup_window_mode();
    println!(
        "World seed: {} | spawn biome: {:?} | spawn snow coverage: {:.0}%",
        biome::world_seed(),
        biome::at(Vec2::ZERO),
        biome::snow_amount(Vec2::ZERO) * 100.0
    );

    let offscreen_profile = std::env::var_os("HITHER_PROFILE_OFFSCREEN").is_some();
    let mut plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Hither — Signed Distance Fields".into(),
            resolution: (1280, 720).into(),
            present_mode: PresentMode::AutoNoVsync,
            mode: startup_window_mode,
            ..default()
        }),
        ..default()
    });
    // Frame-critical ECS/render work benefits from coarse parallel batches.
    // The default uses all remaining logical CPUs (24 on a 32-thread host),
    // causing scheduler/driver contention for this scene's short tasks. Keep
    // background generation and IO on their separate default pools; Bevy still
    // scales the frame pool down automatically on smaller machines.
    let mut task_pools = bevy::app::TaskPoolOptions::default();
    task_pools.compute.max_threads = 4;
    plugins = plugins.set(bevy::app::TaskPoolPlugin {
        task_pool_options: task_pools,
    });
    if offscreen_profile {
        // Retain the logical Window for UI/settings systems, but create no OS
        // window or swapchain. Both cameras render into images in the profiler.
        plugins = plugins.disable::<bevy::winit::WinitPlugin>();
    }
    let mut app = App::new();
    if offscreen_profile {
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(Duration::ZERO));
    }
    app.add_plugins(forest_profile::ForestProfilePlugin)
        .insert_resource(ClearColor(Color::srgb(0.015, 0.02, 0.035)))
        .insert_resource(rig)
        .insert_resource(view)
        .insert_resource(GameState::default())
        .insert_resource(settings)
        .insert_resource(FrameLimiter::default())
        .init_resource::<snow::Tracks>()
        .insert_resource(EditingFps::default())
        .insert_resource(EditingSensitivity::default())
        .insert_resource(OrchardState::default())
        .insert_resource(InitialCursorLock(Timer::from_seconds(0.3, TimerMode::Once)))
        .add_plugins((
            plugins,
            MaterialPlugin::<SdfMaterial>::default(),
            avatar::AvatarPlugin,
            crate::player::hand::HandPlugin,
            orchard::OrchardPlugin,
            crate::world::orcs::OrcPlugin,
            crate::world::goblins::GoblinPlugin,
            crate::world::goblin_dens::DenPlugin,
            grass::GrassPlugin,
            terrain::TerrainPlugin,
            crate::world::geology::GeologyPlugin,
            understory::UnderstoryPlugin,
            graphics::GraphicsPlugin,
            chat::ChatPlugin,
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .add_plugins((
            crate::world::streaming::StreamingPlugin,
            crate::rendering::lighting::LightingPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                initialize_cursor_lock,
                pause_on_escape,
                menu_buttons,
                focus_fps_input,
                type_fps_value,
                update_fps_slider,
                focus_sensitivity_input,
                type_sensitivity_value,
                update_sensitivity_slider,
                sync_options_ui,
                recapture_mouse,
                cycle_camera_mode,
                move_camera,
                update_camera_view,
                pick_fruit,
                snow::update,
                update_shader_uniforms,
                update_fps_counter,
            )
                .chain(),
        )
        .add_systems(Last, (save_settings, limit_frame_rate).chain());
    if offscreen_profile && std::env::var_os("HITHER_PROFILE_NO_GPU_TIMINGS").is_none() {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    if offscreen_profile {
        crate::diagnostics::frame_completion::install(&mut app);
    }
    crate::diagnostics::streaming_profile::plugin(&mut app);
    crate::diagnostics::den_profile::plugin(&mut app);
    crate::rendering::point_shadow_cache::plugin(&mut app);
    app.run();
}
#[derive(Resource, Default)]
pub(crate) struct GameState {
    pub(crate) paused: bool,
}

pub(crate) fn setup(
    device: Res<bevy::render::renderer::RenderDevice>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SdfMaterial>>,
    window: Single<&Window>,
    rig: Res<CameraRig>,
    settings: Res<GraphicsSettings>,
) {
    crate::rendering::scene::setup_scene(
        &mut commands,
        &mut meshes,
        &mut materials,
        &window,
        &rig,
        &settings,
        &device,
    );
    crate::ui::layout::setup_ui(&mut commands, &settings);
}
