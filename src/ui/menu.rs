use crate::app::GameState;
use crate::app::settings::{
    DisplayMode, GraphicsSettings, MAX_MOUSE_SENSITIVITY, MIN_MOUSE_SENSITIVITY,
};
use crate::player::camera::CameraRig;
use crate::ui::chat;
use bevy::app::AppExit;
use bevy::input::keyboard::Key;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::CursorGrabMode;
use bevy::window::CursorOptions;

pub(crate) const BUTTON_NORMAL: Color = Color::srgb(0.12, 0.18, 0.24);
pub(crate) const BUTTON_HOVERED: Color = Color::srgb(0.18, 0.34, 0.42);
pub(crate) const BUTTON_PRESSED: Color = Color::srgb(0.10, 0.55, 0.62);
#[derive(Resource, Default)]
pub(crate) struct EditingFps {
    pub(crate) active: bool,
    pub(crate) replace_on_type: bool,
    pub(crate) buffer: String,
}

#[derive(Resource, Default)]
pub(crate) struct EditingSensitivity {
    pub(crate) active: bool,
    pub(crate) replace_on_type: bool,
    pub(crate) buffer: String,
}

#[derive(Component)]
pub(crate) struct PauseOverlay;

#[derive(Component, Clone, Copy)]
pub(crate) enum MenuAction {
    Resume,
    Options,
    Quit,
    ToggleFps,
    ToggleCoordinates,
    Graphics,
    SetDisplayMode(DisplayMode),
    Back,
}

#[derive(Component)]
pub(crate) struct PausePanel;

#[derive(Component)]
pub(crate) struct OptionsPanel;

#[derive(Component)]
pub(crate) struct FpsLimitValue;

#[derive(Component)]
pub(crate) struct FpsTextInput;

#[derive(Component)]
pub(crate) struct FpsSlider;

#[derive(Component)]
pub(crate) struct SliderFill;

#[derive(Component)]
pub(crate) struct SliderThumb;

#[derive(Component)]
pub(crate) struct FpsToggleLabel;

#[derive(Component)]
pub(crate) struct SensitivityValue;

#[derive(Component)]
pub(crate) struct SensitivityTextInput;

#[derive(Component)]
pub(crate) struct SensitivitySlider;

#[derive(Component)]
pub(crate) struct SensitivitySliderFill;

#[derive(Component)]
pub(crate) struct SensitivitySliderThumb;

#[derive(Component)]
pub(crate) struct DisplayModeButton(pub(crate) DisplayMode);

#[derive(Component)]
pub(crate) struct DisplayModeLabel(pub(crate) DisplayMode);

// Explicit Bevy system parameters preserve ECS access/conflict information.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn pause_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    chat: Res<chat::ChatState>,
    mut game: ResMut<GameState>,
    mut editing_fps: ResMut<EditingFps>,
    mut editing_sensitivity: ResMut<EditingSensitivity>,
    mut cursor: Single<&mut CursorOptions>,
    mut rig: ResMut<CameraRig>,
    mut overlay: Single<&mut Visibility, With<PauseOverlay>>,
    mut panels: Query<
        (&mut Node, Option<&PausePanel>, Option<&OptionsPanel>),
        Or<(With<PausePanel>, With<OptionsPanel>)>,
    >,
) {
    if chat.blocks_gameplay() || !keys.just_pressed(KeyCode::Escape) {
        return;
    }

    if !game.paused {
        game.paused = true;
        rig.mouse_captured = false;
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
        **overlay = Visibility::Visible;
        return;
    }

    let options_open = panels
        .iter()
        .any(|(node, _, options_panel)| options_panel.is_some() && node.display != Display::None);

    editing_fps.active = false;
    editing_sensitivity.active = false;
    if options_open {
        for (mut node, pause_panel, _) in &mut panels {
            node.display = if pause_panel.is_some() {
                Display::Flex
            } else {
                Display::None
            };
        }
    } else {
        game.paused = false;
        rig.mouse_captured = true;
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
        **overlay = Visibility::Hidden;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn menu_buttons(
    mut buttons: Query<
        (&Interaction, &MenuAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut game: ResMut<GameState>,
    mut settings: ResMut<GraphicsSettings>,
    mut editing_fps: ResMut<EditingFps>,
    mut editing_sensitivity: ResMut<EditingSensitivity>,
    mut rig: ResMut<CameraRig>,
    mut cursor: Single<&mut CursorOptions>,
    mut window: Single<&mut Window>,
    mut overlay: Single<&mut Visibility, With<PauseOverlay>>,
    mut panels: Query<
        (&mut Node, Option<&PausePanel>, Option<&OptionsPanel>),
        Or<(With<PausePanel>, With<OptionsPanel>)>,
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
    for (interaction, action, mut color) in &mut buttons {
        *color = match *interaction {
            Interaction::Pressed => {
                match action {
                    MenuAction::Resume => {
                        editing_fps.active = false;
                        editing_sensitivity.active = false;
                        game.paused = false;
                        rig.mouse_captured = true;
                        cursor.visible = false;
                        cursor.grab_mode = CursorGrabMode::Locked;
                        **overlay = Visibility::Hidden;

                        for (mut node, pause_panel, _) in &mut panels {
                            node.display = if pause_panel.is_some() {
                                Display::Flex
                            } else {
                                Display::None
                            };
                        }
                    }
                    MenuAction::Options => {
                        editing_fps.active = false;
                        editing_sensitivity.active = false;
                        for (mut node, _, options_panel) in &mut panels {
                            node.display = if options_panel.is_some() {
                                Display::Flex
                            } else {
                                Display::None
                            };
                        }
                    }
                    MenuAction::Quit => {
                        app_exit.write(AppExit::Success);
                    }
                    MenuAction::ToggleFps => {
                        settings.show_fps = !settings.show_fps;
                    }
                    MenuAction::ToggleCoordinates => {
                        settings.show_coordinates = !settings.show_coordinates;
                    }
                    MenuAction::Graphics => {
                        editing_fps.active = false;
                        editing_sensitivity.active = false;
                        // The graphics panel is opened by its own system.
                        for (mut node, _, _) in &mut panels {
                            node.display = Display::None;
                        }
                    }
                    MenuAction::SetDisplayMode(mode) => {
                        settings.display_mode = *mode;
                        window.mode = mode.window_mode();
                    }
                    MenuAction::Back => {
                        editing_fps.active = false;
                        editing_sensitivity.active = false;
                        for (mut node, pause_panel, _) in &mut panels {
                            node.display = if pause_panel.is_some() {
                                Display::Flex
                            } else {
                                Display::None
                            };
                        }
                    }
                }
                BUTTON_PRESSED.into()
            }
            Interaction::Hovered => BUTTON_HOVERED.into(),
            Interaction::None => match action {
                MenuAction::SetDisplayMode(mode) if *mode == settings.display_mode => {
                    BUTTON_PRESSED.into()
                }
                _ => BUTTON_NORMAL.into(),
            },
        };
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn focus_fps_input(
    mut inputs: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<FpsTextInput>),
    >,
    settings: Res<GraphicsSettings>,
    mut editing: ResMut<EditingFps>,
    mut editing_sensitivity: ResMut<EditingSensitivity>,
) {
    for (interaction, mut color) in &mut inputs {
        match *interaction {
            Interaction::Pressed => {
                editing.active = true;
                editing.replace_on_type = true;
                editing.buffer = settings.max_fps.to_string();
                editing_sensitivity.active = false;
                *color = BUTTON_PRESSED.into();
            }
            Interaction::Hovered => *color = Color::srgb(0.08, 0.16, 0.21).into(),
            Interaction::None => *color = Color::srgb(0.055, 0.085, 0.12).into(),
        }
    }
}

pub(crate) fn type_fps_value(
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut settings: ResMut<GraphicsSettings>,
    mut editing: ResMut<EditingFps>,
) {
    if !editing.active {
        return;
    }

    for event in keyboard_events.read() {
        if !event.state.is_pressed() {
            continue;
        }

        match &event.logical_key {
            Key::Character(characters) => {
                for character in characters.chars().filter(char::is_ascii_digit) {
                    if editing.replace_on_type {
                        editing.buffer.clear();
                        editing.replace_on_type = false;
                    }
                    if editing.buffer.len() < 4 {
                        editing.buffer.push(character);
                    }
                }
            }
            Key::Backspace => {
                if editing.replace_on_type {
                    editing.buffer.clear();
                    editing.replace_on_type = false;
                } else {
                    editing.buffer.pop();
                }
            }
            Key::Enter => {
                if let Ok(value) = editing.buffer.parse::<u32>() {
                    settings.max_fps = value.clamp(1, 1000);
                }
                editing.buffer = settings.max_fps.to_string();
                editing.active = false;
            }
            Key::Escape => {
                editing.buffer = settings.max_fps.to_string();
                editing.active = false;
            }
            _ => {}
        }

        if let Ok(value) = editing.buffer.parse::<u32>() {
            settings.max_fps = value.clamp(1, 1000);
        }
    }
}

pub(crate) fn update_fps_slider(
    slider: Single<(&Interaction, &RelativeCursorPosition), With<FpsSlider>>,
    mut settings: ResMut<GraphicsSettings>,
    mut editing: ResMut<EditingFps>,
    mut editing_sensitivity: ResMut<EditingSensitivity>,
) {
    let (interaction, cursor) = slider.into_inner();
    if *interaction != Interaction::Pressed {
        return;
    }

    if let Some(position) = cursor.normalized {
        let normalized = (position.x + 0.5).clamp(0.0, 1.0);
        settings.max_fps = (1.0 + normalized * 999.0).round() as u32;
        editing.active = false;
        editing.buffer = settings.max_fps.to_string();
        editing_sensitivity.active = false;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn focus_sensitivity_input(
    mut inputs: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<SensitivityTextInput>),
    >,
    settings: Res<GraphicsSettings>,
    mut editing: ResMut<EditingSensitivity>,
    mut editing_fps: ResMut<EditingFps>,
) {
    for (interaction, mut color) in &mut inputs {
        match *interaction {
            Interaction::Pressed => {
                editing.active = true;
                editing.replace_on_type = true;
                editing.buffer = format!("{:.2}", settings.mouse_sensitivity);
                editing_fps.active = false;
                *color = BUTTON_PRESSED.into();
            }
            Interaction::Hovered => *color = Color::srgb(0.08, 0.16, 0.21).into(),
            Interaction::None => *color = Color::srgb(0.055, 0.085, 0.12).into(),
        }
    }
}

pub(crate) fn type_sensitivity_value(
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut settings: ResMut<GraphicsSettings>,
    mut editing: ResMut<EditingSensitivity>,
) {
    if !editing.active {
        return;
    }

    for event in keyboard_events.read() {
        if !event.state.is_pressed() {
            continue;
        }

        match &event.logical_key {
            Key::Character(characters) => {
                for character in characters
                    .chars()
                    .filter(|character| character.is_ascii_digit() || *character == '.')
                {
                    if editing.replace_on_type {
                        editing.buffer.clear();
                        editing.replace_on_type = false;
                    }
                    if editing.buffer.len() < 5
                        && (character != '.' || !editing.buffer.contains('.'))
                    {
                        editing.buffer.push(character);
                    }
                }
            }
            Key::Backspace => {
                if editing.replace_on_type {
                    editing.buffer.clear();
                    editing.replace_on_type = false;
                } else {
                    editing.buffer.pop();
                }
            }
            Key::Enter => {
                if let Ok(value) = editing.buffer.parse::<f32>() {
                    settings.mouse_sensitivity =
                        value.clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY);
                }
                editing.buffer = format!("{:.2}", settings.mouse_sensitivity);
                editing.active = false;
            }
            Key::Escape => {
                editing.buffer = format!("{:.2}", settings.mouse_sensitivity);
                editing.active = false;
            }
            _ => {}
        }

        if let Ok(value) = editing.buffer.parse::<f32>() {
            settings.mouse_sensitivity = value.clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY);
        }
    }
}

pub(crate) fn update_sensitivity_slider(
    slider: Single<(&Interaction, &RelativeCursorPosition), With<SensitivitySlider>>,
    mut settings: ResMut<GraphicsSettings>,
    mut editing: ResMut<EditingSensitivity>,
    mut editing_fps: ResMut<EditingFps>,
) {
    let (interaction, cursor) = slider.into_inner();
    if *interaction != Interaction::Pressed {
        return;
    }

    if let Some(position) = cursor.normalized {
        let normalized = (position.x + 0.5).clamp(0.0, 1.0);
        settings.mouse_sensitivity =
            MIN_MOUSE_SENSITIVITY + normalized * (MAX_MOUSE_SENSITIVITY - MIN_MOUSE_SENSITIVITY);
        editing.active = false;
        editing.buffer = format!("{:.2}", settings.mouse_sensitivity);
        editing_fps.active = false;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_options_ui(
    settings: Res<GraphicsSettings>,
    editing_fps: Res<EditingFps>,
    editing_sensitivity: Res<EditingSensitivity>,
    mut labels: Query<
        (
            &mut Text,
            &mut TextColor,
            Option<&FpsLimitValue>,
            Option<&SensitivityValue>,
            Option<&FpsToggleLabel>,
            Option<&DisplayModeLabel>,
        ),
        Or<(
            With<FpsLimitValue>,
            With<SensitivityValue>,
            With<FpsToggleLabel>,
            With<DisplayModeLabel>,
        )>,
    >,
    mut slider_parts: Query<
        (
            &mut Node,
            Option<&SliderFill>,
            Option<&SliderThumb>,
            Option<&SensitivitySliderFill>,
            Option<&SensitivitySliderThumb>,
        ),
        Or<(
            With<SliderFill>,
            With<SliderThumb>,
            With<SensitivitySliderFill>,
            With<SensitivitySliderThumb>,
        )>,
    >,
    mut mode_buttons: Query<(&DisplayModeButton, &mut BackgroundColor, &mut BorderColor)>,
) {
    if !settings.is_changed() && !editing_fps.is_changed() && !editing_sensitivity.is_changed() {
        return;
    }

    for (mut text, mut text_color, limit_value, sensitivity_value, toggle_label, mode_label) in
        &mut labels
    {
        if limit_value.is_some() {
            **text = if editing_fps.active {
                format!("{}|", editing_fps.buffer)
            } else {
                settings.max_fps.to_string()
            };
        } else if sensitivity_value.is_some() {
            **text = if editing_sensitivity.active {
                format!("{}|", editing_sensitivity.buffer)
            } else {
                format!("{:.2}", settings.mouse_sensitivity)
            };
        } else if toggle_label.is_some() {
            **text = if settings.show_fps {
                "SHOW FPS: ON"
            } else {
                "SHOW FPS: OFF"
            }
            .into();
        } else if let Some(mode_label) = mode_label {
            *text_color = TextColor(if mode_label.0 == settings.display_mode {
                Color::WHITE
            } else {
                Color::srgb(0.72, 0.78, 0.82)
            });
        }
    }

    let slider_percent = (settings.max_fps.saturating_sub(1) as f32 / 999.0) * 100.0;
    let sensitivity_percent = (settings.mouse_sensitivity - MIN_MOUSE_SENSITIVITY)
        / (MAX_MOUSE_SENSITIVITY - MIN_MOUSE_SENSITIVITY)
        * 100.0;
    for (mut node, fill, thumb, sensitivity_fill, sensitivity_thumb) in &mut slider_parts {
        if fill.is_some() {
            node.width = percent(slider_percent);
        } else if thumb.is_some() {
            node.left = percent(slider_percent);
        } else if sensitivity_fill.is_some() {
            node.width = percent(sensitivity_percent);
        } else if sensitivity_thumb.is_some() {
            node.left = percent(sensitivity_percent);
        }
    }

    for (mode_button, mut background, mut border) in &mut mode_buttons {
        let selected = mode_button.0 == settings.display_mode;
        *background = BackgroundColor(if selected {
            BUTTON_PRESSED
        } else {
            BUTTON_NORMAL
        });
        *border = BorderColor::all(if selected {
            Color::srgb(0.36, 0.82, 0.88)
        } else {
            Color::srgb(0.18, 0.28, 0.34)
        });
    }
}
