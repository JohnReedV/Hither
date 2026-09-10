use super::*;

#[test]
fn teleport_moves_both_modes_and_clears_old_jump_motion() {
    let command = COMMANDS.iter().find(|c| c.name == "teleport").unwrap();
    for spectating in [false, true] {
        let mut rig = crate::player::camera::CameraRig::default();
        if spectating {
            rig.toggle_spectator();
        }
        let mode = rig.game_mode;
        let orientation = (rig.yaw, rig.pitch, rig.camera_mode);
        rig.vertical_velocity = -30.0;
        rig.jump_buffer_remaining = 0.1;
        rig.coyote_remaining = 0.1;
        rig.landing_offset = -0.2;
        rig.landing_offset_velocity = 1.0;
        let reply = (command.run)(&mut rig, "-12.5 200 +30", &mut None);
        assert_eq!(reply, "Teleported to -12.5 200 30.");
        assert_eq!(rig.position, Vec3::new(-12.5, 200.0, 30.0));
        assert_eq!(
            rig.jump_height,
            200.0 - crate::player::movement::PLAYER_EYE_HEIGHT
        );
        assert_eq!(rig.game_mode, mode);
        assert_eq!((rig.yaw, rig.pitch, rig.camera_mode), orientation);
        assert!(!rig.grounded);
        assert_eq!(rig.vertical_velocity, 0.0);
        assert_eq!(rig.jump_buffer_remaining, 0.0);
        assert_eq!(rig.coyote_remaining, 0.0);
        assert_eq!(rig.landing_offset, 0.0);
        assert_eq!(rig.landing_offset_velocity, 0.0);
        if !spectating {
            crate::player::movement::update_jump(&mut rig, &ButtonInput::default(), 0.016);
            assert!(rig.position.y < 200.0 && rig.position.y > 199.9);
        }
    }
}

#[test]
fn teleport_rejects_invalid_coordinates_without_moving() {
    let command = COMMANDS.iter().find(|c| c.name == "teleport").unwrap();
    for args in [
        "", "1", "1 2", "1 2 3 4", "abc 2 3", "1 NaN 3", "1 2 inf", "-inf 2 3", "1e99 2 3",
    ] {
        let mut rig = crate::player::camera::CameraRig::default();
        let origin = rig.position;
        assert!((command.run)(&mut rig, args, &mut None).starts_with("Usage: /teleport"));
        assert_eq!(rig.position, origin);
        assert!(rig.grounded);
    }
}

#[test]
fn teleport_chat_submission_moves_spectator_and_stays_local() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<crate::player::camera::CameraRig>()
        .toggle_spectator();
    frame(
        &mut app,
        vec![
            open(),
            text("/tp -12.5 200 30"),
            key(KeyCode::Enter, Key::Enter),
        ],
    );
    assert_eq!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .position,
        Vec3::new(-12.5, 200.0, 30.0)
    );
    assert_eq!(
        app.world().resource::<ChatState>().history.back().unwrap(),
        "Teleported to -12.5 200 30."
    );
    let messages = app.world().resource::<Messages<ChatSubmitted>>();
    assert_eq!(messages.get_cursor().read(messages).count(), 0);
}

#[test]
fn tab_completes_every_registered_command_and_cycles_matches() {
    for command in COMMANDS {
        let mut chat = ChatState::default();
        chat.edit(&text(&format!("/{}", command.name)));
        chat.edit(&key(KeyCode::Tab, Key::Tab));
        assert_eq!(
            chat.draft.iter().collect::<String>(),
            format!("/{}", command.name)
        );
        assert_eq!(chat.caret, chat.draft.len());
    }
    let commands = [
        Command {
            name: "spectate",
            arguments: &[],
            run: |_, _, _| String::new(),
        },
        Command {
            name: "spawn",
            arguments: &[],
            run: |_, _, _| String::new(),
        },
    ];
    let mut chat = ChatState::default();
    chat.edit(&text("/S"));
    for expected in ["/spectate", "/spawn", "/spectate"] {
        chat.complete_command(&commands);
        assert_eq!(chat.draft.iter().collect::<String>(), expected);
    }
    chat.edit(&key(KeyCode::Backspace, Key::Backspace));
    assert!(chat.completion.is_none());
    chat.complete_command(&commands);
    assert_eq!(chat.draft.iter().collect::<String>(), "/spectate");
}

#[test]
fn tab_requires_command_prefix_and_does_not_send_or_leak_input() {
    for draft in ["hello", "/unknown", "/spectate argument"] {
        let mut chat = ChatState::default();
        chat.edit(&text(draft));
        chat.edit(&key(KeyCode::Tab, Key::Tab));
        assert_eq!(chat.draft.iter().collect::<String>(), draft);
    }
    let mut app = app();
    frame(&mut app, vec![open()]);
    frame(&mut app, vec![text("/s")]);
    frame(&mut app, vec![key(KeyCode::Tab, Key::Tab)]);
    assert!(app.world().resource::<ChatState>().active);
    assert!(
        !app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
    frame(&mut app, vec![key(KeyCode::Enter, Key::Enter)]);
    assert!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
}
use bevy::input::{ButtonState, mouse::AccumulatedMouseMotion};

fn key(code: KeyCode, logical: Key) -> KeyboardInput {
    let text = if let Key::Character(s) = &logical {
        Some(s.clone())
    } else {
        None
    };
    KeyboardInput {
        key_code: code,
        logical_key: logical,
        state: ButtonState::Pressed,
        text,
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}
fn text(value: &str) -> KeyboardInput {
    key(KeyCode::KeyA, Key::Character(value.into()))
}
fn open() -> KeyboardInput {
    key(KeyCode::KeyT, Key::Character("t".into()))
}

fn app() -> App {
    let mut app = App::new();
    let mut time = Time::<()>::default();
    time.advance_by(std::time::Duration::from_millis(16));
    app.insert_resource(time)
        .init_resource::<ChatState>()
        .init_resource::<crate::app::GameState>()
        .init_resource::<crate::player::camera::CameraRig>()
        .insert_resource(crate::player::camera::CameraRig::default().view())
        .init_resource::<crate::app::settings::GraphicsSettings>()
        .init_resource::<crate::ui::menu::EditingFps>()
        .init_resource::<crate::ui::menu::EditingSensitivity>()
        .init_resource::<crate::world::orchard::fruit::OrchardState>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .add_message::<KeyboardInput>()
        .add_message::<ChatSubmitted>()
        .add_systems(
            Update,
            (
                input,
                crate::ui::menu::pause_on_escape,
                crate::player::input::recapture_mouse,
                crate::player::camera::cycle_camera_mode,
                crate::player::movement::move_camera,
                crate::world::orchard::fruit::pick_fruit,
            )
                .chain(),
        );
    app.world_mut().spawn(CursorOptions::default());
    app.world_mut()
        .spawn((Visibility::Hidden, crate::ui::menu::PauseOverlay));
    app.world_mut()
        .spawn((Node::default(), crate::ui::menu::PausePanel));
    app.world_mut().spawn((
        Node {
            display: Display::None,
            ..default()
        },
        crate::ui::menu::OptionsPanel,
    ));
    app
}
fn frame(app: &mut App, events: Vec<KeyboardInput>) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    for event in events {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(event.key_code);
        app.world_mut().write_message(event);
    }
    app.update();
}

#[test]
fn open_send_and_escape_do_not_leak_into_gameplay() {
    let mut app = app();
    let initial = app
        .world()
        .resource::<crate::player::camera::CameraRig>()
        .position;
    let apples = app
        .world()
        .resource::<crate::world::orchard::fruit::OrchardState>()
        .apples
        .len();
    frame(&mut app, vec![open()]);
    assert!(app.world().resource::<ChatState>().active);
    assert!(app.world().resource::<ChatState>().draft.is_empty());
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::splat(100.0);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    frame(
        &mut app,
        vec![
            key(KeyCode::KeyW, Key::Character("w".into())),
            key(KeyCode::KeyE, Key::Character("e".into())),
            key(KeyCode::F3, Key::F3),
            key(KeyCode::Space, Key::Character(" ".into())),
        ],
    );
    let rig = app.world().resource::<crate::player::camera::CameraRig>();
    assert_eq!(rig.position, initial);
    assert_eq!(rig.yaw, 0.0);
    assert_eq!(
        rig.camera_mode,
        crate::player::camera::CameraMode::FirstPerson
    );
    assert!(!rig.mouse_captured);
    assert_eq!(
        app.world()
            .resource::<crate::world::orchard::fruit::OrchardState>()
            .apples
            .len(),
        apples
    );
    frame(&mut app, vec![key(KeyCode::Escape, Key::Escape)]);
    assert!(!app.world().resource::<ChatState>().active);
    assert!(!app.world().resource::<crate::app::GameState>().paused);
    assert_eq!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .yaw,
        0.0
    );
    assert!(app.world().resource::<ChatState>().history.is_empty());
    frame(
        &mut app,
        vec![
            open(),
            text("Hello, courtyard!"),
            key(KeyCode::Enter, Key::Enter),
        ],
    );
    assert_eq!(
        app.world().resource::<ChatState>().history.back().unwrap(),
        "<You> Hello, courtyard!"
    );
    let messages = app.world().resource::<Messages<ChatSubmitted>>();
    let sent: Vec<_> = messages
        .get_cursor()
        .read(messages)
        .map(|m| m.text.clone())
        .collect();
    assert_eq!(sent, ["Hello, courtyard!"]);
    frame(&mut app, vec![key(KeyCode::Escape, Key::Escape)]);
    assert!(app.world().resource::<crate::app::GameState>().paused);
    frame(&mut app, vec![open(), text("ignored")]);
    assert!(!app.world().resource::<ChatState>().active);
    assert!(app.world().resource::<ChatState>().draft.is_empty());
}

#[test]
fn slash_prefills_once_commands_toggle_locally_and_escape_cancels() {
    let mut app = app();
    let slash = || key(KeyCode::Slash, Key::Character("/".into()));
    frame(&mut app, vec![slash()]);
    let chat = app.world().resource::<ChatState>();
    assert!(chat.active);
    assert_eq!(chat.draft, vec!['/']);
    assert_eq!(chat.caret, 1);
    frame(
        &mut app,
        vec![text("spectate"), key(KeyCode::Enter, Key::Enter)],
    );
    assert!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
    assert_eq!(
        app.world().resource::<ChatState>().history.back().unwrap(),
        "Spectator mode enabled."
    );
    let messages = app.world().resource::<Messages<ChatSubmitted>>();
    assert_eq!(messages.get_cursor().read(messages).count(), 0);
    frame(
        &mut app,
        vec![slash(), text("spectate"), key(KeyCode::Escape, Key::Escape)],
    );
    assert!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
    assert!(!app.world().resource::<crate::app::GameState>().paused);
    // T can also submit a command, and surrounding whitespace is harmless.
    frame(
        &mut app,
        vec![open(), text(" /spectate "), key(KeyCode::Enter, Key::Enter)],
    );
    assert!(
        !app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
    assert_eq!(app.world().resource::<ChatState>().history.len(), 2);
    frame(
        &mut app,
        vec![
            slash(),
            text("spectate extra"),
            key(KeyCode::Enter, Key::Enter),
        ],
    );
    assert!(
        !app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
    assert!(
        app.world()
            .resource::<ChatState>()
            .history
            .back()
            .unwrap()
            .starts_with("Unknown command")
    );
}

#[test]
fn spectator_controls_freeze_in_chat_and_pause_without_gravity_or_f3_leaks() {
    let mut app = app();
    frame(
        &mut app,
        vec![open(), text("/spectate"), key(KeyCode::Enter, Key::Enter)],
    );
    frame(&mut app, vec![key(KeyCode::Space, Key::Space)]);
    let high = app
        .world()
        .resource::<crate::player::camera::CameraRig>()
        .position;
    assert!(high.y > crate::player::movement::PLAYER_EYE_HEIGHT);
    frame(&mut app, vec![key(KeyCode::F3, Key::F3)]);
    assert_eq!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .camera_mode,
        crate::player::camera::CameraMode::FirstPerson
    );
    frame(&mut app, vec![open()]);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::splat(50.0);
    frame(
        &mut app,
        vec![
            text("wasd"),
            key(KeyCode::Space, Key::Space),
            key(KeyCode::ShiftLeft, Key::Shift),
        ],
    );
    let rig = app.world().resource::<crate::player::camera::CameraRig>();
    assert_eq!(rig.position, high);
    assert_eq!(rig.yaw, 0.0);
    frame(&mut app, vec![key(KeyCode::Escape, Key::Escape)]);
    frame(&mut app, vec![key(KeyCode::Escape, Key::Escape)]);
    assert!(app.world().resource::<crate::app::GameState>().paused);
    frame(
        &mut app,
        vec![
            key(KeyCode::Space, Key::Space),
            open(),
            text("/spectate"),
            key(KeyCode::Enter, Key::Enter),
        ],
    );
    assert_eq!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .position,
        high
    );
    assert!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
}

#[test]
fn spectator_cannot_harvest_a_targeted_apple() {
    let mut app = app();
    let eye = Vec3::new(0.0, 1.1, 2.0);
    let apple = Vec3::new(0.0, 1.1, 1.45);
    assert_eq!(
        crate::player::interaction::target_apple(eye, -Vec3::Z, eye, &[apple]),
        Some(0)
    );
    {
        let mut rig = app
            .world_mut()
            .resource_mut::<crate::player::camera::CameraRig>();
        rig.position = eye;
        rig.pitch = 0.0;
        rig.toggle_spectator();
    }
    *app.world_mut()
        .resource_mut::<crate::player::camera::CameraView>() = app
        .world()
        .resource::<crate::player::camera::CameraRig>()
        .view();
    app.world_mut()
        .resource_mut::<crate::world::orchard::fruit::OrchardState>()
        .apples = vec![apple];
    frame(
        &mut app,
        vec![key(KeyCode::KeyE, Key::Character("e".into()))],
    );
    assert_eq!(
        app.world()
            .resource::<crate::world::orchard::fruit::OrchardState>()
            .apples,
        vec![apple]
    );
}

#[test]
fn typing_does_not_suspend_an_airborne_player() {
    let mut app = app();
    {
        let mut rig = app
            .world_mut()
            .resource_mut::<crate::player::camera::CameraRig>();
        rig.jump_height = 0.5;
        rig.vertical_velocity = -1.0;
        rig.grounded = false;
    }
    frame(&mut app, vec![open()]);
    assert!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .jump_height
            < 0.5
    );
    assert!(!app.world().resource::<crate::app::GameState>().paused);
}

#[test]
fn recall_walks_sent_history_and_restores_unfinished_unicode_draft() {
    let mut chat = ChatState::default();
    for message in ["first", "hé🌱", "/spectate"] {
        chat.edit(&text(message));
        chat.finish(true);
    }
    chat.remember("Spectator mode enabled.".into());
    chat.edit(&text("unfinished 🌳"));
    chat.edit(&key(KeyCode::ArrowLeft, Key::ArrowLeft));
    let saved_caret = chat.caret;
    for expected in ["/spectate", "hé🌱", "first", "first"] {
        chat.edit(&key(KeyCode::ArrowUp, Key::ArrowUp));
        assert_eq!(chat.draft.iter().collect::<String>(), expected);
        assert_eq!(chat.caret, chat.draft.len());
    }
    for expected in ["hé🌱", "/spectate", "unfinished 🌳", "unfinished 🌳"] {
        chat.edit(&key(KeyCode::ArrowDown, Key::ArrowDown));
        assert_eq!(chat.draft.iter().collect::<String>(), expected);
    }
    assert_eq!(chat.caret, saved_caret);
    assert!(chat.recall.is_none());
}

#[test]
fn recalled_edits_do_not_change_history_and_cancel_does_not_add_entries() {
    let mut chat = ChatState::default();
    chat.edit(&text("original"));
    chat.finish(true);
    chat.edit(&key(KeyCode::ArrowUp, Key::ArrowUp));
    chat.edit(&text(" edited"));
    assert_eq!(chat.sent[0], "original");
    assert_eq!(chat.finish(true).unwrap().text, "original edited");
    chat.edit(&key(KeyCode::ArrowUp, Key::ArrowUp));
    assert_eq!(chat.draft.iter().collect::<String>(), "original edited");
    assert!(chat.finish(false).is_none());
    assert!(chat.recall.is_none());
    chat.edit(&text("   "));
    chat.finish(true);
    assert_eq!(chat.sent.len(), 2);
    chat.edit(&text("/"));
    chat.edit(&key(KeyCode::ArrowUp, Key::ArrowUp));
    chat.edit(&key(KeyCode::ArrowDown, Key::ArrowDown));
    assert_eq!(chat.draft, vec!['/']);
}

#[test]
fn recall_is_bounded_empty_safe_and_resets_tab_completion() {
    let mut chat = ChatState::default();
    chat.edit(&text("/s"));
    chat.edit(&key(KeyCode::ArrowUp, Key::ArrowUp));
    chat.edit(&key(KeyCode::ArrowDown, Key::ArrowDown));
    assert_eq!(chat.draft.iter().collect::<String>(), "/s");
    chat.finish(false);
    for i in 0..MAX_HISTORY + 5 {
        chat.edit(&text(&format!("message {i}")));
        chat.finish(true);
    }
    assert_eq!(chat.sent.len(), MAX_HISTORY);
    assert_eq!(chat.sent[0], "message 5");
    chat.edit(&text("/s"));
    chat.edit(&key(KeyCode::Tab, Key::Tab));
    assert!(chat.completion.is_some());
    chat.edit(&key(KeyCode::ArrowUp, Key::ArrowUp));
    assert!(chat.completion.is_none());
    chat.edit(&key(KeyCode::Tab, Key::Tab));
    assert_eq!(chat.draft.iter().collect::<String>(), "message 104");
}

#[test]
fn recalling_commands_waits_for_enter_and_arrows_outside_chat_do_nothing() {
    let mut app = app();
    frame(
        &mut app,
        vec![open(), text("/spectate"), key(KeyCode::Enter, Key::Enter)],
    );
    frame(&mut app, vec![key(KeyCode::ArrowUp, Key::ArrowUp)]);
    assert!(!app.world().resource::<ChatState>().active);
    frame(&mut app, vec![open(), key(KeyCode::ArrowUp, Key::ArrowUp)]);
    assert!(app.world().resource::<ChatState>().active);
    assert_eq!(
        app.world()
            .resource::<ChatState>()
            .draft
            .iter()
            .collect::<String>(),
        "/spectate"
    );
    assert!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
    frame(&mut app, vec![key(KeyCode::Enter, Key::Enter)]);
    assert!(
        !app.world()
            .resource::<crate::player::camera::CameraRig>()
            .is_spectating()
    );
}

#[test]
fn wheel_scroll_is_hover_gated_clamped_and_uses_laid_out_height() {
    let mut app = App::new();
    app.init_resource::<ChatState>()
        .init_resource::<crate::app::GameState>()
        .add_message::<MouseWheel>()
        .add_systems(Update, (scroll_messages, position_log).chain());
    let root = app
        .world_mut()
        .spawn((
            ChatRoot,
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::splat(0.5)),
            },
        ))
        .id();
    let log = app
        .world_mut()
        .spawn((
            ChatLog,
            ComputedNode {
                size: Vec2::new(500.0, 200.0),
                content_size: Vec2::new(500.0, 1200.0),
                inverse_scale_factor: 0.5,
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();
    let wheel = |app: &mut App, unit, y| {
        app.world_mut().write_message(MouseWheel {
            unit,
            x: 0.0,
            y,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
    };
    wheel(&mut app, MouseScrollUnit::Line, 1.0);
    assert_eq!(app.world().get::<ScrollPosition>(log).unwrap().0.y, 500.0);
    app.world_mut().resource_mut::<ChatState>().active = true;
    wheel(&mut app, MouseScrollUnit::Line, 2.0);
    assert_eq!(app.world().get::<ScrollPosition>(log).unwrap().0.y, 428.0);
    wheel(&mut app, MouseScrollUnit::Pixel, 18.0);
    assert_eq!(app.world().resource::<ChatState>().scroll_back, 90.0);
    app.world_mut()
        .get_mut::<RelativeCursorPosition>(root)
        .unwrap()
        .cursor_over = false;
    wheel(&mut app, MouseScrollUnit::Line, 5.0);
    assert_eq!(app.world().resource::<ChatState>().scroll_back, 90.0);
    app.world_mut()
        .get_mut::<RelativeCursorPosition>(root)
        .unwrap()
        .cursor_over = true;
    app.world_mut()
        .resource_mut::<crate::app::GameState>()
        .paused = true;
    wheel(&mut app, MouseScrollUnit::Line, 5.0);
    assert_eq!(app.world().resource::<ChatState>().scroll_back, 90.0);
    app.world_mut()
        .resource_mut::<crate::app::GameState>()
        .paused = false;
    wheel(&mut app, MouseScrollUnit::Line, 1000.0);
    assert_eq!(app.world().get::<ScrollPosition>(log).unwrap().0.y, 0.0);
    wheel(&mut app, MouseScrollUnit::Line, -1000.0);
    assert_eq!(app.world().get::<ScrollPosition>(log).unwrap().0.y, 500.0);
    app.world_mut()
        .get_mut::<ComputedNode>(log)
        .unwrap()
        .content_size
        .y = 100.0;
    wheel(&mut app, MouseScrollUnit::Line, 1000.0);
    assert_eq!(app.world().get::<ScrollPosition>(log).unwrap().0.y, 0.0);
    assert_eq!(app.world().resource::<ChatState>().scroll_back, 0.0);
}

#[test]
fn unicode_editing_length_limit_and_empty_send() {
    let mut chat = ChatState::default();
    chat.edit(&text("hé🌱"));
    chat.edit(&key(KeyCode::ArrowLeft, Key::ArrowLeft));
    chat.edit(&key(KeyCode::Backspace, Key::Backspace));
    chat.edit(&text("i"));
    chat.edit(&key(KeyCode::Delete, Key::Delete));
    assert_eq!(chat.finish(true).unwrap().text, "hi");
    chat.edit(&text(" \n\t "));
    assert!(chat.finish(true).is_none());
    chat.edit(&text(&"é".repeat(300)));
    assert_eq!(chat.draft.len(), MAX_CHARACTERS);
    assert_eq!(
        chat.finish(true).unwrap().text.chars().count(),
        MAX_CHARACTERS
    );
    for _ in 0..110 {
        chat.edit(&text("bounded"));
        chat.finish(true);
    }
    assert_eq!(chat.history.len(), MAX_HISTORY);
}

#[test]
fn locate_validates_arguments_and_returns_coordinates_locally_without_moving_player() {
    let mut app = app();
    for command in ["/locate", "/locate unknown", "/locate plains extra"] {
        frame(
            &mut app,
            vec![open(), text(command), key(KeyCode::Enter, Key::Enter)],
        );
        assert!(
            app.world()
                .resource::<ChatState>()
                .history
                .back()
                .unwrap()
                .starts_with("Usage: /locate")
        );
    }
    let origin = app
        .world()
        .resource::<crate::player::camera::CameraRig>()
        .position;
    frame(
        &mut app,
        vec![
            open(),
            text("/LOCATE   ORC_DEN  "),
            key(KeyCode::Enter, Key::Enter),
        ],
    );
    assert!(
        app.world()
            .resource::<ChatState>()
            .history
            .back()
            .unwrap()
            .starts_with("Searching for orc_den")
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        frame(&mut app, vec![]);
        if app
            .world()
            .resource::<ChatState>()
            .history
            .back()
            .unwrap()
            .starts_with("Nearest orc_den: X ")
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "locate result never reached chat"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        app.world()
            .resource::<crate::player::camera::CameraRig>()
            .position
            .xz(),
        origin.xz()
    );
    let messages = app.world().resource::<Messages<ChatSubmitted>>();
    assert_eq!(messages.get_cursor().read(messages).count(), 0);
}

#[test]
fn suggestions_match_tab_and_cycle_all_locate_targets() {
    let mut chat = ChatState::default();
    chat.edit(&text("/s"));
    assert_eq!(chat.suggestion_text(), "Spectate");
    chat.edit(&key(KeyCode::Tab, Key::Tab));
    assert_eq!(chat.draft.iter().collect::<String>(), "/spectate");
    chat.finish(false);
    chat.edit(&text("/LOCATE   g"));
    assert_eq!(chat.suggestion_text(), "goblin_den");
    chat.edit(&key(KeyCode::Tab, Key::Tab));
    assert_eq!(chat.draft.iter().collect::<String>(), "/locate goblin_den");
    chat.finish(false);
    chat.edit(&text("/locate "));
    let mut completed = Vec::new();
    for _ in crate::world::locate::Target::NAMES {
        let bottom = chat.suggestions(COMMANDS)[0].clone();
        let expected_label = bottom.split_once(' ').unwrap().1.to_owned();
        assert_eq!(
            chat.suggestion_text().lines().last(),
            Some(expected_label.as_str())
        );
        chat.edit(&key(KeyCode::Tab, Key::Tab));
        let value = chat.draft.iter().collect::<String>();
        assert_eq!(value, bottom);
        assert!(!completed.contains(&value));
        assert!(crate::world::locate::Target::parse(value.split_once(' ').unwrap().1).is_some());
        completed.push(value);
    }
    chat.edit(&key(KeyCode::Tab, Key::Tab));
    assert_eq!(chat.draft.iter().collect::<String>(), completed[0]);
    chat.edit(&key(KeyCode::ArrowLeft, Key::ArrowLeft));
    assert!(chat.suggestions(COMMANDS).is_empty());
    chat.finish(false);
    chat.edit(&text("/locate nonexistent"));
    assert!(chat.suggestion_text().is_empty());
}

#[test]
fn display_shows_suggestions_above_input_and_hides_them_for_regular_chat() {
    let mut app = App::new();
    app.init_resource::<ChatState>()
        .init_resource::<crate::app::GameState>()
        .add_systems(Update, display);
    app.world_mut().spawn(Window::default());
    app.world_mut().spawn((Node::default(), ChatRoot));
    app.world_mut().spawn((Node::default(), ChatEntry));
    app.world_mut().spawn((Node::default(), ChatLog));
    app.world_mut().spawn((Text::new(""), ChatDraft));
    app.world_mut().spawn((Text::new(""), ChatHistory));
    let suggestions = app
        .world_mut()
        .spawn((Text::new(""), Node::default(), ChatSuggestions))
        .id();
    {
        let mut chat = app.world_mut().resource_mut::<ChatState>();
        chat.active = true;
        chat.edit(&text("/s"));
    }
    app.update();
    assert_eq!(app.world().get::<Text>(suggestions).unwrap().0, "Spectate");
    assert_eq!(
        app.world().get::<Node>(suggestions).unwrap().display,
        Display::Flex
    );
    {
        let mut chat = app.world_mut().resource_mut::<ChatState>();
        chat.finish(false);
        chat.active = true;
        chat.edit(&text("hello"));
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(suggestions).unwrap().display,
        Display::None
    );
}

#[test]
fn named_teleports_share_all_locate_targets_and_completion() {
    use crate::{player::movement::PLAYER_EYE_HEIGHT, world::locate::Target};
    for name in ["tp", "teleport"] {
        let command = COMMANDS.iter().find(|c| c.name == name).unwrap();
        let mut chat = ChatState::default();
        chat.edit(&text(&format!("/{name} ")));
        assert_eq!(chat.suggestions(COMMANDS).len(), Target::NAMES.len());
        for target_name in Target::NAMES {
            let mut rig = crate::player::camera::CameraRig::default();
            if name == "teleport" {
                rig.toggle_spectator();
            }
            let mode = rig.game_mode;
            let origin = rig.position;
            let expected = Target::parse(target_name).unwrap().find(origin).unwrap()
                + Vec3::Y * PLAYER_EYE_HEIGHT;
            let mut pending = None;
            assert!(
                (command.run)(&mut rig, &target_name.to_uppercase(), &mut pending)
                    .starts_with("Searching for")
            );
            assert_eq!(rig.position, origin);
            rig.vertical_velocity = -30.0;
            let result = pending.take().unwrap().join().unwrap();
            assert!(result.apply(&mut rig).starts_with("Teleported to nearest"));
            assert_eq!(rig.position, expected);
            assert_eq!(rig.game_mode, mode);
            assert_eq!(rig.vertical_velocity, 0.0);
        }
    }
}

#[test]
fn named_teleport_result_is_applied_by_chat_and_stays_local() {
    let mut app = app();
    let origin = app
        .world()
        .resource::<crate::player::camera::CameraRig>()
        .position;
    let expected = crate::world::locate::Target::GoblinDen
        .find(origin)
        .unwrap()
        + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
    frame(
        &mut app,
        vec![
            open(),
            text("/TP  GOBLIN_DEN  "),
            key(KeyCode::Enter, Key::Enter),
        ],
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        frame(&mut app, vec![]);
        if app
            .world()
            .resource::<ChatState>()
            .history
            .back()
            .unwrap()
            .starts_with("Teleported to nearest goblin_den")
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "teleport result never reached chat"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let arrived = app
        .world()
        .resource::<crate::player::camera::CameraRig>()
        .position;
    assert_eq!(arrived.xz(), expected.xz());
    // Player gravity may run in the same frame that chat applies the result.
    assert!(arrived.y <= expected.y && arrived.y > expected.y - 0.1);
    let messages = app.world().resource::<Messages<ChatSubmitted>>();
    assert_eq!(messages.get_cursor().read(messages).count(), 0);
}

#[test]
fn named_teleport_failures_and_superseded_searches_do_not_move_player() {
    let mut rig = crate::player::camera::CameraRig::default();
    let origin = rig.position;
    let mut pending = None;
    for args in ["unknown", "goblin_den extra", "plains 1 2"] {
        assert!(teleport_command(&mut rig, args, &mut pending).starts_with("Usage:"));
        assert!(pending.is_none());
        assert_eq!(rig.position, origin);
    }
    // Invalid search origins produce feedback without scheduling a teleport.
    rig.position.x = 200_000_000.0;
    teleport_command(&mut rig, "plains", &mut pending);
    let result = pending.take().unwrap().join().unwrap();
    assert!(result.destination.is_none());
    assert_eq!(result.apply(&mut rig), "Cannot locate from this position.");
    rig.position = origin;
    teleport_command(&mut rig, "orc_den", &mut pending);
    assert_eq!(
        teleport_command(&mut rig, "plains", &mut pending),
        "A locate search is already running."
    );
    teleport_command(&mut rig, "1 2 3", &mut pending);
    assert!(pending.is_none());
    assert_eq!(rig.position, Vec3::new(1.0, 2.0, 3.0));
}
