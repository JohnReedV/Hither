use crate::app::GameState;
use crate::player::camera::CameraRig;
use crate::ui::chat;
use bevy::prelude::*;
use bevy::window::CursorGrabMode;
use bevy::window::CursorOptions;

#[derive(Resource)]
pub(crate) struct InitialCursorLock(pub(crate) Timer);

pub(crate) fn initialize_cursor_lock(
    time: Res<Time>,
    game: Res<GameState>,
    chat: Res<chat::ChatState>,
    mut initial_lock: ResMut<InitialCursorLock>,
    mut cursor: Single<&mut CursorOptions>,
) {
    if initial_lock.0.is_finished() || game.paused || chat.blocks_gameplay() {
        return;
    }

    initial_lock.0.tick(time.delta());
    if initial_lock.0.just_finished() {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

pub(crate) fn recapture_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    game: Res<GameState>,
    chat: Res<chat::ChatState>,
    mut cursor: Single<&mut CursorOptions>,
    mut rig: ResMut<CameraRig>,
) {
    if !game.paused
        && !chat.blocks_gameplay()
        && !rig.mouse_captured
        && (buttons.just_pressed(MouseButton::Left) || buttons.just_pressed(MouseButton::Right))
    {
        rig.mouse_captured = true;
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}
