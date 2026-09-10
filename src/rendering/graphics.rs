//! Saved, live graphics controls. UI is rendered at display resolution independently of the world.
use crate::app::settings::GraphicsSettings;
use crate::ui::menu::{BUTTON_NORMAL, MenuAction, OptionsPanel, PauseOverlay};
use bevy::{
    anti_alias::fxaa::Fxaa,
    camera::{CameraOutputMode, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    light::DirectionalLightShadowMap,
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat, TextureUsages},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const RESOLUTIONS: &[(u32, u32)] = &[
    (0, 0),
    (320, 180),
    (426, 240),
    (640, 360),
    (854, 480),
    (960, 540),
    (1280, 720),
    (1366, 768),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3200, 1800),
    (3840, 2160),
];
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AntiAliasing {
    Off,
    Fxaa,
    Msaa2,
    Msaa4,
    Msaa8,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Low,
    Medium,
    High,
}
pub struct GraphicsPlugin;
impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        super::plant_lod::plugin(app);
        super::radial_shadows::plugin(app);
        super::surface_cache::plugin(app);
        app.init_resource::<crate::rendering::view_distance::Range>()
            .init_resource::<crate::rendering::view_distance::Detail>()
            .add_systems(
                Update,
                crate::rendering::view_distance::sync
                    .after(distance_controls)
                    .before(crate::player::movement::move_camera),
            );
        app.add_systems(Startup, setup.after(crate::app::setup))
            .add_systems(
                Update,
                (buttons, distance_controls, apply, coordinates)
                    .chain()
                    .after(crate::ui::menu::menu_buttons),
            )
            .add_systems(Update, textures);
    }
}
#[derive(Component)]
struct Advanced;
#[derive(Component)]
struct Coordinates;
#[derive(Component, Clone, Copy)]
pub(crate) enum Setting {
    Coordinates,
    Resolution(i32),
    Aa,
    Textures,
    Shadows,
    Grass,
}
#[derive(Component)]
struct DistanceField;
#[derive(Component)]
struct DistanceSlider;
#[derive(Component)]
struct DistanceValue;
#[derive(Component)]
struct DistanceFill;
#[derive(Component)]
struct DistanceThumb;
#[derive(Component)]
struct DistanceLimit(bool);
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
enum DistanceKind {
    #[default]
    Render,
    Detail,
    Shadow,
}
impl DistanceKind {
    fn min(self) -> f32 {
        match self {
            Self::Detail | Self::Shadow => 0.0,
            Self::Render => crate::rendering::view_distance::MIN,
        }
    }

    fn max(self, s: &GraphicsSettings) -> f32 {
        match self {
            Self::Render => crate::rendering::view_distance::MAX,
            Self::Detail | Self::Shadow => s.render_distance,
        }
    }
    fn limit_label(self, upper: bool, settings: &GraphicsSettings) -> String {
        if upper {
            let name = match self {
                Self::Render => "Render",
                Self::Detail => "Detail",
                Self::Shadow => "Shadow",
            };
            format!("{name} max: {:.0} m", self.max(settings))
        } else {
            format!("{:.0}", self.min())
        }
    }
    fn slider_value(self, s: &GraphicsSettings, fraction: f32) -> f32 {
        (self.min() + fraction.clamp(0.0, 1.0) * (self.max(s) - self.min())).round()
    }
    fn fraction(self, s: &GraphicsSettings) -> f32 {
        (self.get(s) - self.min()) / (self.max(s) - self.min())
    }
    fn get(self, s: &GraphicsSettings) -> f32 {
        match self {
            Self::Render => s.render_distance,
            Self::Detail => s.detail_distance,
            Self::Shadow => s.shadow_distance,
        }
    }
    fn set(self, s: &mut GraphicsSettings, n: f32) {
        let n = n.clamp(self.min(), self.max(s));
        match self {
            Self::Render => s.render_distance = n,
            Self::Detail => s.detail_distance = n,
            Self::Shadow => s.shadow_distance = n,
        }
        s.detail_distance = s.detail_distance.min(s.render_distance);
        s.shadow_distance = s.shadow_distance.min(s.render_distance);
    }
}
#[derive(Default)]
struct DistanceEdit {
    kind: DistanceKind,
    active: bool,
    replace: bool,
    buffer: String,
}
#[derive(Resource)]
struct WorldTarget(Handle<Image>);
#[derive(Component)]
struct FinalCamera;
#[derive(Component)]
pub(crate) struct WorldPass;

fn setup(
    mut commands: Commands,
    settings: Res<GraphicsSettings>,
    overlay: Single<Entity, With<PauseOverlay>>,
    mut images: ResMut<Assets<Image>>,
    camera: Single<Entity, With<crate::player::avatar::PlayerCamera>>,
) {
    let mut image = Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::TEXTURE_BINDING;
    let target = images.add(image);
    commands.entity(*camera).insert((
        RenderTarget::Image(target.clone().into()),
        Camera {
            order: -3,
            output_mode: CameraOutputMode::Skip,
            ..default()
        },
    ));
    commands.spawn((
        Camera2d,
        Hdr,
        WorldPass,
        FinalCamera,
        RenderLayers::layer(31),
        RenderTarget::Image(target.clone().into()),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::TonyMcMapface,
        bevy::post_process::bloom::Bloom {
            intensity: 0.06,
            ..default()
        },
        Msaa::Off,
        Fxaa::default(),
    ));
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        IsDefaultUiCamera,
    ));
    commands.spawn((
        ImageNode::new(target.clone()),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        GlobalZIndex(-100),
        bevy::ui::FocusPolicy::Pass,
    ));
    commands.insert_resource(WorldTarget(target));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(Color::srgba(0.78, 0.9, 0.94, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            top: px(7),
            right: px(50),
            ..default()
        },
        GlobalZIndex(90),
        Coordinates,
    ));
    commands.entity(*overlay).with_children(|parent| {
        parent
            .spawn((
                Node {
                    width: px(440),
                    padding: UiRect::all(px(22)),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.035, 0.05, 0.075)),
                OptionsPanel,
                Advanced,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("GRAPHICS"),
                    TextFont {
                        font_size: FontSize::Px(27.0),
                        ..default()
                    },
                ));
                panel
                    .spawn(Node {
                        width: percent(100),
                        height: px(42),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|row| {
                        for step in [-1, 0, 1] {
                            if step == 0 {
                                row.spawn((
                                    Text::new(""),
                                    TextFont {
                                        font_size: FontSize::Px(16.0),
                                        ..default()
                                    },
                                    Setting::Resolution(0),
                                ));
                            } else {
                                row.spawn((
                                    Button,
                                    Setting::Resolution(step),
                                    Node {
                                        width: px(36),
                                        height: px(42),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    BackgroundColor(BUTTON_NORMAL),
                                ))
                                .with_child((
                                    Text::new(if step < 0 { "<" } else { ">" }),
                                    TextFont {
                                        font_size: FontSize::Px(18.0),
                                        ..default()
                                    },
                                ));
                            }
                        }
                    });
                for kind in [
                    DistanceKind::Render,
                    DistanceKind::Detail,
                    DistanceKind::Shadow,
                ] {
                    panel
                        .spawn(Node {
                            width: percent(100),
                            height: px(38),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Text::new(match kind {
                                    DistanceKind::Render => "RENDER DISTANCE (m)",
                                    DistanceKind::Detail => "DETAIL DISTANCE (m)",
                                    DistanceKind::Shadow => "SHADOW DISTANCE (m)",
                                }),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.66, 0.72, 0.78)),
                            ));
                            row.spawn((
                                Button,
                                DistanceField,
                                kind,
                                Node {
                                    width: px(100),
                                    height: px(42),
                                    border: UiRect::all(px(1)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(px(6)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.055, 0.085, 0.12)),
                                BorderColor::all(Color::srgb(0.18, 0.38, 0.46)),
                            ))
                            .with_child((
                                Text::new("48"),
                                TextFont {
                                    font_size: FontSize::Px(20.0),
                                    ..default()
                                },
                                DistanceValue,
                                kind,
                            ));
                        });
                    panel
                        .spawn((
                            Button,
                            DistanceSlider,
                            kind,
                            bevy::ui::RelativeCursorPosition::default(),
                            Node {
                                width: percent(100),
                                height: px(28),
                                position_type: PositionType::Relative,
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                        ))
                        .with_children(|slider| {
                            slider.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(0),
                                    right: px(0),
                                    top: px(10),
                                    height: px(8),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.075, 0.12, 0.16)),
                            ));
                            slider.spawn((
                                DistanceFill,
                                kind,
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(0),
                                    top: px(10),
                                    height: px(8),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.10, 0.62, 0.70)),
                            ));
                            slider.spawn((
                                DistanceThumb,
                                kind,
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: px(5),
                                    width: px(18),
                                    height: px(18),
                                    margin: UiRect::left(px(-9)),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.72, 0.94, 1.0)),
                            ));
                        });
                    panel
                        .spawn(Node {
                            width: percent(100),
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|row| {
                            for upper in [false, true] {
                                row.spawn((
                                    kind,
                                    DistanceLimit(upper),
                                    Text::new(kind.limit_label(upper, &settings)),
                                    TextFont {
                                        font_size: FontSize::Px(10.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.42, 0.52, 0.58)),
                                ));
                            }
                        });
                }
                for setting in [
                    Setting::Aa,
                    Setting::Textures,
                    Setting::Shadows,
                    Setting::Grass,
                ] {
                    panel
                        .spawn((
                            Button,
                            setting,
                            Node {
                                width: percent(100),
                                height: px(42),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(BUTTON_NORMAL),
                        ))
                        .with_child((
                            Text::new(""),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            setting,
                        ));
                }
                panel
                    .spawn((
                        Button,
                        MenuAction::Back,
                        Node {
                            width: percent(100),
                            height: px(42),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(BUTTON_NORMAL),
                    ))
                    .with_child((
                        Text::new("BACK"),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                    ));
            });
    });
}

#[allow(clippy::type_complexity)]
fn buttons(
    actions: Query<(&Interaction, &MenuAction), Changed<Interaction>>,
    mut panel: Single<&mut Node, With<Advanced>>,
    mut controls: Query<
        (&Interaction, &Setting, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut settings: ResMut<GraphicsSettings>,
) {
    for (interaction, action) in &actions {
        if *interaction == Interaction::Pressed {
            match action {
                MenuAction::Graphics => panel.display = Display::Flex,
                MenuAction::Options => panel.display = Display::None,
                _ => {}
            }
        }
    }
    for (interaction, setting, mut color) in &mut controls {
        *color = BackgroundColor(match interaction {
            Interaction::Pressed => crate::ui::menu::BUTTON_PRESSED,
            Interaction::Hovered => crate::ui::menu::BUTTON_HOVERED,
            Interaction::None => BUTTON_NORMAL,
        });
        if *interaction != Interaction::Pressed {
            continue;
        }
        match setting {
            Setting::Coordinates => settings.show_coordinates = !settings.show_coordinates,
            Setting::Resolution(step) => {
                settings.resolution = (settings.resolution as i32 + step)
                    .rem_euclid(RESOLUTIONS.len() as i32)
                    as usize
            }
            Setting::Aa => {
                settings.anti_aliasing = match settings.anti_aliasing {
                    AntiAliasing::Off => AntiAliasing::Fxaa,
                    AntiAliasing::Fxaa => AntiAliasing::Msaa2,
                    AntiAliasing::Msaa2 => AntiAliasing::Msaa4,
                    AntiAliasing::Msaa4 => AntiAliasing::Msaa8,
                    AntiAliasing::Msaa8 => AntiAliasing::Off,
                }
            }
            Setting::Textures => settings.texture_quality = next(settings.texture_quality),
            Setting::Shadows => settings.shadow_quality = next(settings.shadow_quality),
            Setting::Grass => settings.grass_blades = !settings.grass_blades,
        }
    }
}
fn next(q: Quality) -> Quality {
    match q {
        Quality::Low => Quality::Medium,
        Quality::Medium => Quality::High,
        Quality::High => Quality::Low,
    }
}

fn distance_value(kind: DistanceKind, settings: &GraphicsSettings, text: &str) -> Option<f32> {
    text.parse::<u32>()
        .ok()
        .map(|n| (n as f32).clamp(kind.min(), kind.max(settings)))
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn distance_controls(
    mut events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    game: Res<crate::app::GameState>,
    panel: Single<&Node, With<Advanced>>,
    controls: Query<
        (
            &Interaction,
            Option<&bevy::ui::RelativeCursorPosition>,
            Option<&DistanceField>,
            &DistanceKind,
        ),
        Or<(With<DistanceField>, With<DistanceSlider>)>,
    >,
    mut settings: ResMut<GraphicsSettings>,
    mut edit: Local<DistanceEdit>,
    mut values: Query<
        (&mut Text, &DistanceKind, Option<&DistanceLimit>),
        Or<(With<DistanceValue>, With<DistanceLimit>)>,
    >,
    mut parts: Query<
        (&mut Node, Option<&DistanceFill>, &DistanceKind),
        (
            Or<(With<DistanceFill>, With<DistanceThumb>)>,
            Without<Advanced>,
        ),
    >,
) {
    use bevy::input::keyboard::Key;
    let open = game.paused && panel.display != Display::None;
    if !open {
        edit.active = false;
        events.clear();
    }
    if open {
        let field_pressed = controls.iter().find_map(|(i, _, field, kind)| {
            (field.is_some() && *i == Interaction::Pressed).then_some(*kind)
        });
        if mouse.just_pressed(MouseButton::Left) {
            if edit.active && field_pressed != Some(edit.kind) {
                if let Some(n) = distance_value(edit.kind, &settings, &edit.buffer) {
                    edit.kind.set(&mut settings, n);
                }
                edit.active = false;
            }
            if let Some(kind) = field_pressed {
                edit.kind = kind;
                edit.active = true;
                edit.replace = true;
                edit.buffer = format!("{:.0}", kind.get(&settings));
            }
        }
        for (interaction, cursor, _, kind) in &controls {
            if *interaction == Interaction::Pressed
                && let Some(p) = cursor.and_then(|c| c.normalized)
            {
                let n = kind.slider_value(&settings, p.x + 0.5);
                if kind.get(&settings) != n {
                    kind.set(&mut settings, n);
                }
                edit.active = false;
            }
        }
        if edit.active {
            for event in events.read().filter(|e| e.state.is_pressed()) {
                match &event.logical_key {
                    Key::Character(s) => {
                        for c in s.chars().filter(char::is_ascii_digit) {
                            if edit.replace {
                                edit.buffer.clear();
                                edit.replace = false;
                            }
                            if edit.buffer.len() < 6 {
                                edit.buffer.push(c);
                            }
                        }
                    }
                    Key::Backspace => {
                        if edit.replace {
                            edit.buffer.clear();
                            edit.replace = false;
                        } else {
                            edit.buffer.pop();
                        }
                    }
                    Key::Enter => {
                        if let Some(n) = distance_value(edit.kind, &settings, &edit.buffer) {
                            edit.kind.set(&mut settings, n);
                        }
                        edit.active = false;
                    }
                    Key::Escape => edit.active = false,
                    _ => {}
                }
            }
        } else {
            events.clear();
        }
    }
    for (mut value, kind, limit) in &mut values {
        if let Some(limit) = limit {
            value.set_if_neq(Text::new(kind.limit_label(limit.0, &settings)));
            continue;
        }
        value.set_if_neq(Text::new(if edit.active && edit.kind == *kind {
            format!("{}|", edit.buffer)
        } else {
            format!("{:.0}", kind.get(&settings))
        }));
    }
    for (mut node, fill, kind) in &mut parts {
        let fraction = 100.0 * kind.fraction(&settings);
        if fill.is_some() {
            if node.width != percent(fraction) {
                node.width = percent(fraction);
            }
        } else if node.left != percent(fraction) {
            node.left = percent(fraction);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply(
    mut commands: Commands,
    settings: Res<GraphicsSettings>,
    window: Single<&Window>,
    mut target: ResMut<WorldTarget>,
    mut images: ResMut<Assets<Image>>,
    passes: Query<(Entity, Has<FinalCamera>), With<WorldPass>>,
    mut targets: Query<(&mut RenderTarget, &mut Projection), With<WorldPass>>,
    mut nodes: Query<&mut ImageNode>,
    mut labels: Query<(&Setting, &mut Text)>,
    mut shadows: ResMut<DirectionalLightShadowMap>,
) {
    let (w, h) = RESOLUTIONS[settings.resolution];
    // Keep the display aspect ratio even for ultrawide or resized windows.
    let size = render_size(
        settings.resolution,
        UVec2::new(
            window.physical_width().max(1),
            window.physical_height().max(1),
        ),
    );
    if let Some(image) = images.get(&target.0)
        && image.size() != size
    {
        // Bevy 0.19 camera_system does not observe Changed<RenderTarget>.
        // Mark every projection so all passes recompute dimensions this frame;
        // waiting for the Image asset event leaves the depth sizes inconsistent.
        let mut image =
            Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::TEXTURE_BINDING;
        let old = std::mem::replace(&mut target.0, images.add(image));
        for (mut render_target, mut projection) in &mut targets {
            *render_target = RenderTarget::Image(target.0.clone().into());
            projection.set_changed();
        }
        for mut node in &mut nodes {
            if node.image == old {
                node.image = target.0.clone();
            }
        }
        images.remove(old.id());
    }

    if !settings.is_changed() {
        return;
    }
    let msaa = match settings.anti_aliasing {
        AntiAliasing::Msaa2 => Msaa::Sample2,
        AntiAliasing::Msaa4 => Msaa::Sample4,
        AntiAliasing::Msaa8 => Msaa::Sample8,
        _ => Msaa::Off,
    };
    for (entity, final_pass) in &passes {
        commands.entity(entity).insert(msaa);
        if final_pass {
            commands.entity(entity).insert(Fxaa {
                enabled: settings.anti_aliasing == AntiAliasing::Fxaa,
                ..default()
            });
        } else {
            commands.entity(entity).remove::<Fxaa>();
        }
    }
    shadows.size = match settings.shadow_quality {
        Quality::Low => 512,
        Quality::Medium => 1024,
        Quality::High => 2048,
    };
    for (setting, mut text) in &mut labels {
        **text = match setting {
            Setting::Coordinates => format!(
                "COORDINATES: {}",
                if settings.show_coordinates {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            Setting::Resolution(step) => format!(
                "{}  RESOLUTION: {}",
                if *step < 0 {
                    "<"
                } else if *step > 0 {
                    ">"
                } else {
                    ""
                },
                if w == 0 {
                    "NATIVE".into()
                } else {
                    format!("{w} x {h}")
                }
            ),
            Setting::Aa => format!("ANTI-ALIASING: {:?}", settings.anti_aliasing).to_uppercase(),
            Setting::Textures => format!("TEXTURES: {:?}", settings.texture_quality).to_uppercase(),
            Setting::Shadows => format!("SHADOWS: {:?}", settings.shadow_quality).to_uppercase(),
            Setting::Grass => format!(
                "GRASS BLADES: {}",
                if settings.grass_blades { "ON" } else { "OFF" }
            ),
        };
    }
}
pub(crate) fn render_size(preset: usize, window: UVec2) -> UVec2 {
    let (w, h) = RESOLUTIONS[preset.min(RESOLUTIONS.len() - 1)];
    if w == 0 {
        return window;
    }
    let scale = (w as f32 / window.x as f32).min(h as f32 / window.y as f32);
    (window.as_vec2() * scale)
        .round()
        .as_uvec2()
        .max(UVec2::ONE)
}

fn coordinates(
    settings: Res<GraphicsSettings>,
    rig: Res<crate::player::camera::CameraRig>,
    mut label: Single<&mut Text, With<Coordinates>>,
) {
    let p = rig.position;
    label.set_if_neq(Text::new(if settings.show_coordinates {
        format!(
            "X {:.1}  Y {:.1}  Z {:.1}",
            p.x,
            p.y - crate::player::movement::PLAYER_EYE_HEIGHT,
            p.z
        )
    } else {
        String::new()
    }));
}

// Only world-material textures are eligible: never resize UI fonts or render targets.
// Originals are retained on the CPU so switching back to high restores exact pixels.
#[derive(Default)]
struct TextureQueue {
    elapsed: f32,
    pending: std::collections::VecDeque<AssetId<Image>>,
    job: Option<TextureJob>,
}
struct TextureJob {
    id: AssetId<Image>,
    quality: Quality,
    #[cfg(not(test))]
    task: bevy::tasks::Task<Image>,
    #[cfg(test)]
    ready: Option<Image>,
}
impl TextureJob {
    fn new(id: AssetId<Image>, quality: Quality, original: std::sync::Arc<Image>) -> Self {
        Self {
            id,
            quality,
            #[cfg(not(test))]
            task: bevy::tasks::AsyncComputeTaskPool::get()
                .spawn(async move { texture_at_quality(&original, quality) }),
            #[cfg(test)]
            ready: Some(texture_at_quality(&original, quality)),
        }
    }
    fn take_ready(&mut self) -> Option<Image> {
        #[cfg(not(test))]
        {
            bevy::tasks::block_on(bevy::tasks::poll_once(&mut self.task))
        }
        #[cfg(test)]
        {
            self.ready.take()
        }
    }
}
fn texture_at_quality(original: &Image, quality: Quality) -> Image {
    let divisor = match quality {
        Quality::High => 1,
        Quality::Medium => 2,
        Quality::Low => 4,
    };
    let mut reduced = original.clone();
    if divisor > 1 && original.texture_descriptor.mip_level_count > 1 {
        super::mipmaps::reduce(&mut reduced, if divisor == 2 { 1 } else { 2 });
    } else if divisor > 1 {
        let w = original.width();
        let h = original.height();
        let nw = (w / divisor).max(1);
        let nh = (h / divisor).max(1);
        let source = original.data.as_ref().unwrap();
        let mut pixels = Vec::with_capacity((nw * nh * 4) as usize);
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0u32;
                    let mut count = 0;
                    for sy in y * h / nh..(y + 1) * h / nh {
                        for sx in x * w / nw..(x + 1) * w / nw {
                            sum += source[((sy * w + sx) * 4 + c) as usize] as u32;
                            count += 1;
                        }
                    }
                    pixels.push((sum / count) as u8);
                }
            }
        }
        reduced.resize(Extent3d {
            width: nw,
            height: nh,
            depth_or_array_layers: 1,
        });
        reduced.data = Some(pixels);
    }
    reduced
}
fn textures(
    time: Res<Time>,
    settings: Res<GraphicsSettings>,
    materials: Res<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut originals: Local<HashMap<AssetId<Image>, std::sync::Arc<Image>>>,
    mut applied: Local<HashMap<AssetId<Image>, Quality>>,
    mut queue: Local<TextureQueue>,
) {
    let started = std::time::Instant::now();
    let TextureQueue {
        elapsed,
        pending,
        job,
    } = &mut *queue;
    *elapsed += time.delta_secs();
    if *elapsed >= 0.5 || settings.is_changed() {
        *elapsed = 0.0;
        let ids: HashSet<_> = materials
            .iter()
            .flat_map(|(_, m)| {
                [
                    m.base_color_texture.as_ref(),
                    m.normal_map_texture.as_ref(),
                    m.metallic_roughness_texture.as_ref(),
                    m.occlusion_texture.as_ref(),
                    m.emissive_texture.as_ref(),
                ]
                .into_iter()
                .flatten()
                .map(Handle::id)
            })
            .collect();
        originals.retain(|id, _| ids.contains(id));
        applied.retain(|id, _| ids.contains(id));
        pending.clear();
        pending.extend(
            ids.into_iter()
                .filter(|id| applied.get(id) != Some(&settings.texture_quality)),
        );
    }
    if let Some(active) = job
        && let Some(image) = active.take_ready()
    {
        // Quality can change again while a worker is running. Never apply a
        // stale result, nor resurrect an image removed by streaming.
        if active.quality == settings.texture_quality
            && originals.contains_key(&active.id)
            && let Some(mut current) = images.get_mut(active.id)
        {
            *current = image;
            applied.insert(active.id, active.quality);
        }
        *job = None;
        // At most one completed image uploads per frame (including an atomic
        // oversize image). The next worker is admitted on the next frame.
        return;
    }
    if job.is_some() {
        return;
    }
    let legacy = std::env::var_os("HITHER_PROFILE_FOREST").is_some()
        && std::env::var_os("HITHER_PROFILE_UNBUDGETED_TEXTURES").is_some();
    while let Some(id) = pending.pop_front() {
        if !legacy && started.elapsed().as_micros() >= 2000 {
            pending.push_front(id);
            break;
        }
        if applied.get(&id) == Some(&settings.texture_quality) {
            continue;
        }
        let Some(image) = images.get(id) else {
            continue;
        };
        if (settings.texture_quality == Quality::High && !originals.contains_key(&id))
            || image.texture_descriptor.size.depth_or_array_layers != 1
            || !matches!(
                image.texture_descriptor.format,
                TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
            )
            || image.data.is_none()
        {
            continue;
        }
        // Snapshot each original only once. Workers share it without copying
        // pixels on the frame thread on subsequent quality changes.
        let original = originals
            .entry(id)
            .or_insert_with(|| std::sync::Arc::new(image.clone()))
            .clone();
        if legacy {
            *images.get_mut(id).unwrap() = texture_at_quality(&original, settings.texture_quality);
            applied.insert(id, settings.texture_quality);
        } else {
            *job = Some(TextureJob::new(id, settings.texture_quality, original));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displayed_detail_and_shadow_limits_follow_render_distance() {
        let mut app = App::new();
        app.init_resource::<GraphicsSettings>()
            .init_resource::<crate::app::GameState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .add_systems(Update, distance_controls);
        app.init_resource::<Assets<Image>>()
            .add_systems(Startup, setup);
        app.world_mut().spawn(PauseOverlay);
        app.world_mut().spawn(crate::player::avatar::PlayerCamera);
        // Check labels created by the real menu before the update system runs.
        app.world_mut().run_schedule(Startup);
        let mut query = app
            .world_mut()
            .query::<(Entity, &DistanceKind, &DistanceLimit, &Text)>();
        let labels: Vec<_> = query
            .iter(app.world())
            .filter(|(_, kind, limit, _)| limit.0 && **kind != DistanceKind::Render)
            .map(|(entity, kind, _, text)| {
                assert_eq!(text.0, kind.limit_label(true, &GraphicsSettings::default()));
                (entity, *kind)
            })
            .collect();
        assert_eq!(labels.len(), 2);
        for render in [48.0, 32.0, 128.0] {
            app.world_mut()
                .resource_mut::<GraphicsSettings>()
                .render_distance = render;
            app.world_mut().run_schedule(Update);
            for &(label, kind) in &labels {
                assert_eq!(
                    app.world().get::<Text>(label).unwrap().0,
                    kind.limit_label(true, app.world().resource::<GraphicsSettings>())
                );
            }
        }
    }
    #[test]
    fn texture_queue_publishes_oversize_images_singly_and_restores_exact_originals() {
        let mut app = App::new();
        let mut images = Assets::<Image>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let handles: Vec<_> = (0..2)
            .map(|index| {
                let image = Image::new(
                    Extent3d {
                        width: 1024,
                        height: 2048,
                        depth_or_array_layers: 1,
                    },
                    bevy::render::render_resource::TextureDimension::D2,
                    [31 + index, 61, 91, 255].repeat(1024 * 2048),
                    TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::default(),
                );
                let handle = images.add(image);
                let material = materials.add(StandardMaterial {
                    base_color_texture: Some(handle.clone()),
                    ..default()
                });
                (handle, material)
            })
            .collect();
        app.init_resource::<Time>()
            .insert_resource(GraphicsSettings {
                texture_quality: Quality::Low,
                ..default()
            })
            .insert_resource(images)
            .insert_resource(materials)
            .add_systems(Update, textures);
        app.update(); // A low-quality result is in flight.
        app.world_mut()
            .resource_mut::<GraphicsSettings>()
            .texture_quality = Quality::High;
        app.update(); // That stale result must never flash onto the image.
        assert!(handles.iter().all(|(h, _)| {
            app.world()
                .resource::<Assets<Image>>()
                .get(h)
                .unwrap()
                .width()
                == 1024
        }));
        app.world_mut()
            .resource_mut::<GraphicsSettings>()
            .texture_quality = Quality::Low;
        app.update();
        app.update();
        assert_eq!(
            handles
                .iter()
                .filter(|(h, _)| app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(h)
                    .unwrap()
                    .width()
                    == 256)
                .count(),
            1
        );
        for _ in 0..8 {
            app.update();
        }
        for (handle, _) in &handles {
            assert_eq!(
                app.world()
                    .resource::<Assets<Image>>()
                    .get(handle)
                    .unwrap()
                    .width(),
                256
            );
        }
        app.world_mut()
            .resource_mut::<GraphicsSettings>()
            .texture_quality = Quality::High;
        for _ in 0..8 {
            app.update();
        }
        for (index, (handle, _)) in handles.iter().enumerate() {
            let image = app.world().resource::<Assets<Image>>().get(handle).unwrap();
            assert_eq!((image.width(), image.height()), (1024, 2048));
            assert!(
                image
                    .data
                    .as_ref()
                    .unwrap()
                    .chunks_exact(4)
                    .all(|p| p == [31 + index as u8, 61, 91, 255])
            );
        }
    }

    #[test]
    fn detail_and_shadow_sliders_end_at_render_distance() {
        let mut settings = GraphicsSettings::default();
        for render in [16.0, 47.0, 128.0, 1024.0] {
            DistanceKind::Render.set(&mut settings, render);
            for kind in [DistanceKind::Detail, DistanceKind::Shadow] {
                assert_eq!(kind.max(&settings), render);
                assert_eq!(kind.slider_value(&settings, 0.0), 0.0);
                assert_eq!(kind.slider_value(&settings, 1.0), render);
                assert_eq!(distance_value(kind, &settings, "999999"), Some(render));
                kind.set(&mut settings, 2048.0);
                assert_eq!(kind.get(&settings), render);
                assert_eq!(kind.fraction(&settings), 1.0);
            }
        }
        DistanceKind::Render.set(&mut settings, 32.0);
        assert_eq!(settings.detail_distance, 32.0);
        assert_eq!(settings.shadow_distance, 32.0);
        DistanceKind::Detail.set(&mut settings, 0.0);
        DistanceKind::Shadow.set(&mut settings, 0.0);
        DistanceKind::Render.set(&mut settings, 16.0);
        assert_eq!(settings.detail_distance, 0.0);
        assert_eq!(settings.shadow_distance, 0.0);
    }
    #[test]
    fn distance_input_is_bounded() {
        assert_eq!(
            distance_value(DistanceKind::Detail, &GraphicsSettings::default(), "0"),
            Some(0.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Detail, &GraphicsSettings::default(), "1"),
            Some(1.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Shadow, &GraphicsSettings::default(), "0"),
            Some(0.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Render, &GraphicsSettings::default(), "2048"),
            Some(1024.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Render, &GraphicsSettings::default(), "1024"),
            Some(1024.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Render, &GraphicsSettings::default(), "999"),
            Some(999.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Render, &GraphicsSettings::default(), "8"),
            Some(16.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Render, &GraphicsSettings::default(), "64"),
            Some(64.0)
        );
        assert_eq!(
            distance_value(DistanceKind::Render, &GraphicsSettings::default(), ""),
            None
        );
        assert_eq!(
            distance_value(DistanceKind::Render, &GraphicsSettings::default(), "NaN"),
            None
        );
    }
    #[test]
    fn distance_controls_edit_independent_saved_values() {
        let mut settings = GraphicsSettings::default();
        DistanceKind::Detail.set(&mut settings, 32.0);
        DistanceKind::Render.set(&mut settings, 128.0);
        assert_eq!(DistanceKind::Detail.get(&settings), 32.0);
        assert_eq!(DistanceKind::Render.get(&settings), 128.0);
    }
    #[test]
    fn old_settings_keep_tested_defaults() {
        let settings: GraphicsSettings =
            serde_json::from_str(r#"{"max_fps":1000,"show_fps":true}"#).unwrap();
        assert_eq!(settings.anti_aliasing, AntiAliasing::Fxaa);
        assert_eq!(settings.shadow_quality, Quality::Medium);
        assert_eq!(settings.texture_quality, Quality::High);
        assert!(settings.grass_blades);
        assert!(!settings.show_coordinates);
        assert_eq!(settings.resolution, 0);
    }
    #[test]
    fn resolutions_cover_low_to_4k_and_preserve_aspect() {
        assert_eq!(render_size(1, UVec2::new(1920, 1080)), UVec2::new(320, 180));
        assert_eq!(
            render_size(12, UVec2::new(1920, 1080)),
            UVec2::new(3840, 2160)
        );
        assert_eq!(
            render_size(0, UVec2::new(3440, 1440)),
            UVec2::new(3440, 1440)
        );
        let wide = render_size(9, UVec2::new(3440, 1440));
        assert!((wide.x as f32 / wide.y as f32 - 3440.0 / 1440.0).abs() < 0.003);
    }
    #[test]
    fn graphics_round_trip() {
        let settings = GraphicsSettings {
            show_coordinates: true,
            resolution: 12,
            anti_aliasing: AntiAliasing::Msaa4,
            texture_quality: Quality::Low,
            shadow_quality: Quality::High,
            grass_blades: false,
            render_distance: 87.0,
            detail_distance: 32.0,
            shadow_distance: 96.0,
            ..default()
        };
        let decoded: GraphicsSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(decoded.resolution, 12);
        assert_eq!(decoded.anti_aliasing, AntiAliasing::Msaa4);
        assert_eq!(decoded.texture_quality, Quality::Low);
        assert!(decoded.show_coordinates);
        assert!(!decoded.grass_blades);
        assert_eq!(decoded.render_distance, 87.0);
        assert_eq!(decoded.detail_distance, 32.0);
        assert_eq!(decoded.shadow_distance, 96.0);
    }
}
