use crate::app::settings::{
    DisplayMode, GraphicsSettings, MAX_MOUSE_SENSITIVITY, MIN_MOUSE_SENSITIVITY,
};
use crate::rendering::graphics;
use crate::ui::hud::FpsCounter;
use crate::ui::menu::{
    BUTTON_NORMAL, BUTTON_PRESSED, DisplayModeButton, DisplayModeLabel, FpsLimitValue, FpsSlider,
    FpsTextInput, FpsToggleLabel, MenuAction, OptionsPanel, PauseOverlay, PausePanel,
    SensitivitySlider, SensitivitySliderFill, SensitivitySliderThumb, SensitivityTextInput,
    SensitivityValue, SliderFill, SliderThumb,
};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

pub(crate) fn setup_ui(commands: &mut Commands, settings: &GraphicsSettings) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(5),
            height: px(5),
            margin: UiRect::new(px(-2.5), px(0), px(-2.5), px(0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgba(0.85, 0.95, 1.0, 0.65)),
    ));

    commands.spawn((
        Text::new("--"),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(Color::srgba(0.78, 0.9, 0.94, 0.72)),
        Node {
            position_type: PositionType::Absolute,
            top: px(7),
            right: px(9),
            ..default()
        },
        GlobalZIndex(90),
        Visibility::Hidden,
        FpsCounter,
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.015, 0.025, 0.72)),
            GlobalZIndex(100),
            Visibility::Hidden,
            PauseOverlay,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(360),
                        padding: UiRect::all(px(28)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(14),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.035, 0.05, 0.075, 0.97)),
                    PausePanel,
                ))
                .with_children(|menu| {
                    menu.spawn((
                        Text::new("PAUSED"),
                        TextFont {
                            font_size: FontSize::Px(42.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.72, 0.94, 1.0)),
                    ));
                    menu.spawn((
                        Text::new("The world is waiting."),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.66, 0.72, 0.78)),
                    ));

                    for (label, action) in [
                        ("RESUME", MenuAction::Resume),
                        ("OPTIONS", MenuAction::Options),
                        ("QUIT", MenuAction::Quit),
                    ] {
                        menu.spawn((
                            Button,
                            Node {
                                width: percent(100),
                                height: px(54),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(px(7)),
                                ..default()
                            },
                            BackgroundColor(BUTTON_NORMAL),
                            action,
                        ))
                        .with_child((
                            Text::new(label),
                            TextFont {
                                font_size: FontSize::Px(19.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    }
                });

            overlay
                .spawn((
                    Node {
                        width: px(420),
                        display: Display::None,
                        padding: UiRect::all(px(28)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(14),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.035, 0.05, 0.075, 0.97)),
                    OptionsPanel,
                ))
                .with_children(|menu| {
                    menu.spawn((
                        Text::new("OPTIONS"),
                        TextFont {
                            font_size: FontSize::Px(36.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.72, 0.94, 1.0)),
                    ));
                    menu.spawn(Node {
                        width: percent(100),
                        height: px(46),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Text::new("MAXIMUM FPS"),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.66, 0.72, 0.78)),
                        ));

                        row.spawn((
                            Button,
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
                            FpsTextInput,
                        ))
                        .with_child((
                            Text::new("144"),
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.82, 0.95, 1.0)),
                            FpsLimitValue,
                        ));
                    });

                    menu.spawn((
                        Button,
                        Node {
                            position_type: PositionType::Relative,
                            width: percent(100),
                            height: px(28),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        RelativeCursorPosition::default(),
                        FpsSlider,
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
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(0),
                                top: px(10),
                                width: percent(14.3),
                                height: px(8),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.10, 0.62, 0.70)),
                            SliderFill,
                        ));
                        slider.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: percent(14.3),
                                top: px(5),
                                width: px(18),
                                height: px(18),
                                margin: UiRect::left(px(-9)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.72, 0.94, 1.0)),
                            SliderThumb,
                        ));
                    });

                    menu.spawn(Node {
                        width: percent(100),
                        justify_content: JustifyContent::SpaceBetween,
                        margin: UiRect::top(px(-10)),
                        ..default()
                    })
                    .with_children(|range| {
                        for label in ["1", "1000"] {
                            range.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.42, 0.52, 0.58)),
                            ));
                        }
                    });

                    menu.spawn(Node {
                        width: percent(100),
                        height: px(46),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Text::new("MOUSE SENSITIVITY"),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.66, 0.72, 0.78)),
                        ));

                        row.spawn((
                            Button,
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
                            SensitivityTextInput,
                        ))
                        .with_child((
                            Text::new(format!("{:.2}", settings.mouse_sensitivity)),
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.82, 0.95, 1.0)),
                            SensitivityValue,
                        ));
                    });

                    let sensitivity_percent = (settings.mouse_sensitivity - MIN_MOUSE_SENSITIVITY)
                        / (MAX_MOUSE_SENSITIVITY - MIN_MOUSE_SENSITIVITY)
                        * 100.0;
                    menu.spawn((
                        Button,
                        Node {
                            position_type: PositionType::Relative,
                            width: percent(100),
                            height: px(28),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        RelativeCursorPosition::default(),
                        SensitivitySlider,
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
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(0),
                                top: px(10),
                                width: percent(sensitivity_percent),
                                height: px(8),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.10, 0.62, 0.70)),
                            SensitivitySliderFill,
                        ));
                        slider.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: percent(sensitivity_percent),
                                top: px(5),
                                width: px(18),
                                height: px(18),
                                margin: UiRect::left(px(-9)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.72, 0.94, 1.0)),
                            SensitivitySliderThumb,
                        ));
                    });

                    menu.spawn(Node {
                        width: percent(100),
                        justify_content: JustifyContent::SpaceBetween,
                        margin: UiRect::top(px(-10)),
                        ..default()
                    })
                    .with_children(|range| {
                        for label in ["0.10", "5.00"] {
                            range.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.42, 0.52, 0.58)),
                            ));
                        }
                    });

                    menu.spawn((
                        Text::new("DISPLAY MODE"),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.66, 0.72, 0.78)),
                        Node {
                            align_self: AlignSelf::FlexStart,
                            margin: UiRect::top(px(2)),
                            ..default()
                        },
                    ));

                    menu.spawn(Node {
                        width: percent(100),
                        height: px(44),
                        column_gap: px(8),
                        ..default()
                    })
                    .with_children(|row| {
                        for (label, mode) in [
                            ("WINDOWED", DisplayMode::Windowed),
                            ("BORDERLESS", DisplayMode::Borderless),
                            ("FULLSCREEN", DisplayMode::Fullscreen),
                        ] {
                            let selected = settings.display_mode == mode;
                            row.spawn((
                                Button,
                                Node {
                                    flex_grow: 1.0,
                                    height: px(44),
                                    border: UiRect::all(px(1)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(px(7)),
                                    ..default()
                                },
                                BackgroundColor(if selected {
                                    BUTTON_PRESSED
                                } else {
                                    BUTTON_NORMAL
                                }),
                                BorderColor::all(if selected {
                                    Color::srgb(0.36, 0.82, 0.88)
                                } else {
                                    Color::srgb(0.18, 0.28, 0.34)
                                }),
                                MenuAction::SetDisplayMode(mode),
                                DisplayModeButton(mode),
                            ))
                            .with_child((
                                Text::new(label),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(if selected {
                                    Color::WHITE
                                } else {
                                    Color::srgb(0.72, 0.78, 0.82)
                                }),
                                DisplayModeLabel(mode),
                            ));
                        }
                    });

                    menu.spawn(Node {
                        width: percent(100),
                        height: px(50),
                        column_gap: px(8),
                        ..default()
                    })
                    .with_children(|row| {
                        for (label, action) in [
                            ("SHOW FPS: ON", MenuAction::ToggleFps),
                            ("COORDINATES: OFF", MenuAction::ToggleCoordinates),
                        ] {
                            row.spawn((
                                Button,
                                action,
                                Node {
                                    flex_basis: px(0),
                                    flex_grow: 1.0,
                                    height: px(50),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(px(7)),
                                    ..default()
                                },
                                BackgroundColor(BUTTON_NORMAL),
                            ))
                            .with_children(|button| {
                                let mut text = button.spawn((
                                    Text::new(label),
                                    TextFont {
                                        font_size: FontSize::Px(13.0),
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                                if matches!(action, MenuAction::ToggleFps) {
                                    text.insert(FpsToggleLabel);
                                } else {
                                    text.insert(graphics::Setting::Coordinates);
                                }
                            });
                        }
                    });
                    for (label, action) in [
                        ("GRAPHICS", MenuAction::Graphics),
                        ("BACK", MenuAction::Back),
                    ] {
                        let mut button = menu.spawn((
                            Button,
                            Node {
                                width: percent(100),
                                height: px(50),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(px(7)),
                                ..default()
                            },
                            BackgroundColor(BUTTON_NORMAL),
                            action,
                        ));
                        button.with_children(|button| {
                            let mut label_entity = button.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: FontSize::Px(17.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                            if matches!(action, MenuAction::ToggleFps) {
                                label_entity.insert(FpsToggleLabel);
                            }
                        });
                    }
                });
        });
}
