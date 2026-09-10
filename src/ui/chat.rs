//! Local chat UI and outgoing-message boundary for a future multiplayer transport.
use bevy::{
    input::keyboard::{Key, KeyboardInput},
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    ui::{RelativeCursorPosition, UiSystems},
    window::{CursorGrabMode, CursorOptions},
};
use std::collections::VecDeque;

const MAX_CHARACTERS: usize = 256;
const MAX_HISTORY: usize = 100;
const RECENT_SECONDS: f32 = 12.0;

const TELEPORT_USAGE: &str = "Usage: /teleport <x> <y> <z> (three finite numbers) or /tp <target> (same targets as /locate).";

struct SearchResult {
    message: String,
    destination: Option<Vec3>,
}
impl SearchResult {
    fn apply(self, rig: &mut crate::player::camera::CameraRig) -> String {
        if let Some(position) = self.destination {
            rig.teleport(position);
        }
        self.message
    }
}

// One registry drives dispatch and completion, so newly registered commands
// cannot silently lack Tab support.
struct Command {
    name: &'static str,
    arguments: &'static [&'static str],
    run: fn(
        &mut crate::player::camera::CameraRig,
        &str,
        &mut Option<std::thread::JoinHandle<SearchResult>>,
    ) -> String,
}
fn teleport_command(
    rig: &mut crate::player::camera::CameraRig,
    args: &str,
    pending: &mut Option<std::thread::JoinHandle<SearchResult>>,
) -> String {
    if let Some(target) = crate::world::locate::Target::parse(args) {
        if pending.is_some() {
            return "A locate search is already running.".into();
        }
        let origin = rig.position;
        *pending = Some(std::thread::spawn(move || match target.find(origin) {
            Ok(ground) => {
                let position = ground + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
                SearchResult {
                    message: format!(
                        "Teleported to nearest {}: X {:.2}, Y {:.2}, Z {:.2}.",
                        target.name(),
                        position.x,
                        position.y,
                        position.z
                    ),
                    destination: Some(position),
                }
            }
            Err(message) => SearchResult {
                message,
                destination: None,
            },
        }));
        return format!("Searching for {}…", target.name());
    }
    let mut coordinates = args.split_whitespace();
    let mut position = Vec3::ZERO;
    for axis in 0..3 {
        let Some(value) = coordinates
            .next()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite())
        else {
            return TELEPORT_USAGE.into();
        };
        position[axis] = value;
    }
    if coordinates.next().is_some() {
        return TELEPORT_USAGE.into();
    }
    // A newer explicit teleport supersedes any outstanding destination search.
    *pending = None;
    rig.teleport(position);
    format!(
        "Teleported to {} {} {}.",
        position.x, position.y, position.z
    )
}

const COMMANDS: &[Command] = &[
    Command {
        name: "teleport",
        arguments: crate::world::locate::Target::NAMES,
        run: teleport_command,
    },
    Command {
        name: "tp",
        arguments: crate::world::locate::Target::NAMES,
        run: teleport_command,
    },
    Command {
        name: "spectate",
        arguments: &[],
        run: |rig, args, _| {
            if !args.is_empty() {
                return "Unknown command. Use /spectate, /locate <target>, /tp <target>, or /teleport <x> <y> <z>.".into();
            }
            rig.toggle_spectator();
            if rig.is_spectating() {
                "Spectator mode enabled.".into()
            } else {
                "Player mode restored.".into()
            }
        },
    },
    Command {
        name: "locate",
        arguments: crate::world::locate::Target::NAMES,
        run: |rig, args, pending| {
            let Some(target) = crate::world::locate::Target::parse(args) else {
                return crate::world::locate::USAGE.into();
            };
            if pending.is_some() {
                return "A locate search is already running.".into();
            }
            let origin = rig.position;
            *pending = Some(std::thread::spawn(move || SearchResult {
                message: target.locate(origin),
                destination: None,
            }));
            format!("Searching for {}…", target.name())
        },
    },
];

struct Completion {
    prefix: String,
    next: usize,
}

struct Recall {
    index: usize,
    draft: Vec<char>,
    caret: usize,
}

pub struct ChatPlugin;
impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatState>()
            .add_message::<ChatSubmitted>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                input.before(crate::player::input::initialize_cursor_lock),
            )
            .add_systems(Update, scroll_messages.after(input))
            .add_systems(PostUpdate, position_log.after(UiSystems::Layout))
            .add_systems(
                Update,
                display.after(input).after(crate::ui::menu::pause_on_escape),
            );
    }
}

/// Only nonempty, length-limited messages reach this boundary. No network yet.
#[derive(Message, Clone, Debug)]
pub(crate) struct ChatSubmitted {
    pub text: String,
}

#[derive(Resource, Default)]
pub(crate) struct ChatState {
    active: bool,
    // Keep Escape/Enter and any other keys in the closing frame out of gameplay.
    consumed_frame: bool,
    draft: Vec<char>,
    caret: usize,
    history: VecDeque<String>,
    // Raw submissions only: no <You> prefix, command feedback or future incoming messages.
    sent: VecDeque<String>,
    recall: Option<Recall>,
    // Logical pixels above the newest message; zero follows the bottom.
    scroll_back: f32,
    recent: f32,
    completion: Option<Completion>,
}

impl ChatState {
    pub(crate) fn blocks_gameplay(&self) -> bool {
        self.active || self.consumed_frame
    }

    fn edit(&mut self, event: &KeyboardInput) {
        if event.logical_key == Key::Tab {
            self.complete_command(COMMANDS);
            return;
        }
        self.completion = None;
        match event.logical_key {
            Key::ArrowUp => self.recall_message(true),
            Key::ArrowDown => self.recall_message(false),
            Key::Backspace if self.caret > 0 => {
                self.caret -= 1;
                self.draft.remove(self.caret);
            }
            Key::Delete if self.caret < self.draft.len() => {
                self.draft.remove(self.caret);
            }
            Key::ArrowLeft => self.caret = self.caret.saturating_sub(1),
            Key::ArrowRight => self.caret = (self.caret + 1).min(self.draft.len()),
            Key::Home => self.caret = 0,
            Key::End => self.caret = self.draft.len(),
            _ => {
                let text = event.text.as_deref().or_else(|| match &event.logical_key {
                    Key::Character(value) => Some(value.as_str()),
                    _ => None,
                });
                if let Some(text) = text {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if self.draft.len() >= MAX_CHARACTERS {
                            break;
                        }
                        self.draft.insert(self.caret, c);
                        self.caret += 1;
                    }
                }
            }
        }
    }

    fn recall_message(&mut self, older: bool) {
        if self.sent.is_empty() {
            return;
        }
        if older {
            if let Some(recall) = &mut self.recall {
                recall.index = recall.index.saturating_sub(1);
            } else {
                self.recall = Some(Recall {
                    index: self.sent.len() - 1,
                    draft: self.draft.clone(),
                    caret: self.caret,
                });
            }
        } else if let Some(recall) = &mut self.recall {
            recall.index += 1;
            if recall.index == self.sent.len() {
                let recall = self.recall.take().unwrap();
                self.draft = recall.draft;
                self.caret = recall.caret;
                return;
            }
        } else {
            return;
        }
        self.draft = self.sent[self.recall.as_ref().unwrap().index]
            .chars()
            .collect();
        self.caret = self.draft.len();
    }

    // First match is the bottom suggestion and the next value Tab will insert.
    fn suggestions(&self, commands: &[Command]) -> Vec<String> {
        if self.caret != self.draft.len() {
            return Vec::new();
        }
        let typed = self.draft.iter().collect::<String>().to_ascii_lowercase();
        let (prefix, next) = self
            .completion
            .as_ref()
            .map_or((typed.as_str(), 0), |c| (c.prefix.as_str(), c.next));
        let Some(raw) = prefix.strip_prefix('/') else {
            return Vec::new();
        };
        let mut matches: Vec<String> =
            if let Some((name, argument)) = raw.split_once(char::is_whitespace) {
                let argument = argument.trim_start();
                commands
                    .iter()
                    .find(|c| c.name == name)
                    .into_iter()
                    .flat_map(|c| {
                        c.arguments
                            .iter()
                            .filter(move |a| a.starts_with(argument))
                            .map(move |a| format!("/{} {}", c.name, a))
                    })
                    .collect()
            } else {
                commands
                    .iter()
                    .filter(|c| c.name.starts_with(raw))
                    .map(|c| format!("/{}", c.name))
                    .collect()
            };
        if !matches.is_empty() {
            let count = matches.len();
            matches.rotate_left(next % count);
        }
        matches
    }

    fn suggestion_text(&self) -> String {
        self.suggestions(COMMANDS)
            .iter()
            .take(5)
            .rev()
            .map(|value| {
                value.rsplit_once(' ').map_or_else(
                    || {
                        let name = value.trim_start_matches('/');
                        format!("{}{}", name[..1].to_ascii_uppercase(), &name[1..])
                    },
                    |(_, argument)| argument.to_owned(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn complete_command(&mut self, commands: &[Command]) {
        let Some(value) = self.suggestions(commands).into_iter().next() else {
            return;
        };
        let prefix = self.draft.iter().collect::<String>().to_ascii_lowercase();
        let completion = self
            .completion
            .get_or_insert(Completion { prefix, next: 0 });
        completion.next += 1;
        self.draft = value.chars().collect();
        self.caret = self.draft.len();
    }

    fn finish(&mut self, send: bool) -> Option<ChatSubmitted> {
        let text = self.draft.iter().collect::<String>().trim().to_owned();
        self.draft.clear();
        self.caret = 0;
        self.active = false;
        self.completion = None;
        self.recall = None;
        self.scroll_back = 0.0;
        if !send || text.is_empty() {
            return None;
        }
        let message = ChatSubmitted { text };
        self.sent.push_back(message.text.clone());
        if self.sent.len() > MAX_HISTORY {
            self.sent.pop_front();
        }
        if !message.text.starts_with('/') {
            self.remember(format!("<You> {}", message.text));
        }
        Some(message)
    }

    fn remember(&mut self, text: String) {
        self.history.push_back(text);
        if self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        self.recent = RECENT_SECONDS;
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects independent input resources and message streams."
)]
fn input(
    mut events: MessageReader<KeyboardInput>,
    mut outgoing: MessageWriter<ChatSubmitted>,
    game: Res<crate::app::GameState>,
    time: Res<Time>,
    mut chat: ResMut<ChatState>,
    mut rig: ResMut<crate::player::camera::CameraRig>,
    mut cursor: Single<&mut CursorOptions>,
    mut pending: Local<Option<std::thread::JoinHandle<SearchResult>>>,
) {
    if pending.as_ref().is_some_and(|task| task.is_finished()) {
        let result = pending
            .take()
            .unwrap()
            .join()
            .unwrap_or_else(|_| SearchResult {
                message: "Locate search failed. Please try again.".into(),
                destination: None,
            });
        chat.remember(result.apply(&mut rig));
    }
    chat.consumed_frame = chat.active;
    if !game.paused {
        chat.recent = (chat.recent - time.delta_secs()).max(0.0);
    }
    let mut closed = false;
    // Always drain input: keys typed in menus must never replay into chat.
    for event in events.read() {
        if game.paused || closed || !event.state.is_pressed() {
            continue;
        }
        if !chat.active {
            let slash = matches!(&event.logical_key, Key::Character(c) if c.as_str() == "/")
                || event.text.as_deref() == Some("/");
            if (event.key_code == KeyCode::KeyT || slash) && !event.repeat {
                chat.active = true;
                chat.consumed_frame = true;
                if slash {
                    chat.draft.push('/');
                    chat.caret = 1;
                }
                rig.mouse_captured = false;
                cursor.visible = true;
                cursor.grab_mode = CursorGrabMode::None;
            }
            continue; // Opening key is consumed; slash is inserted exactly once.
        }
        match event.logical_key {
            Key::Enter | Key::Escape => {
                if let Some(message) = chat.finish(event.logical_key == Key::Enter) {
                    if let Some(raw) = message.text.strip_prefix('/') {
                        let (name, args) = raw.split_once(char::is_whitespace).unwrap_or((raw, ""));
                        let response = COMMANDS
                            .iter()
                            .find(|c| name.eq_ignore_ascii_case(c.name))
                            .map(|command| (command.run)(&mut rig, args.trim(), &mut pending))
                            .unwrap_or_else(|| {
                                "Unknown command. Use /spectate, /locate <target>, /tp <target>, or /teleport <x> <y> <z>.".into()
                            });
                        chat.remember(response);
                    } else {
                        outgoing.write(message);
                    }
                }
                rig.mouse_captured = true;
                cursor.visible = false;
                cursor.grab_mode = CursorGrabMode::Locked;
                closed = true;
            }
            _ => chat.edit(event),
        }
    }
}

#[derive(Component)]
struct ChatRoot;
#[derive(Component)]
struct ChatEntry;
#[derive(Component)]
struct ChatDraft;
#[derive(Component)]
struct ChatSuggestions;
#[derive(Component)]
struct ChatHistory;
#[derive(Component)]
struct ChatLog;

fn scroll_messages(
    mut wheel: MessageReader<MouseWheel>,
    mut chat: ResMut<ChatState>,
    game: Res<crate::app::GameState>,
    root: Single<&RelativeCursorPosition, With<ChatRoot>>,
) {
    for event in wheel.read() {
        if chat.active && !game.paused && root.cursor_over() && event.y.is_finite() {
            let delta = event.y
                * match event.unit {
                    MouseScrollUnit::Line => 36.0,
                    MouseScrollUnit::Pixel => 1.0,
                };
            chat.scroll_back = (chat.scroll_back + delta).max(0.0);
        }
    }
}

// Use actual laid-out height, not message counts: long wrapped messages and
// window/UI scaling must scroll correctly too. Layout applies this next frame.
fn position_log(
    mut chat: ResMut<ChatState>,
    log: Single<(&ComputedNode, &mut ScrollPosition), With<ChatLog>>,
) {
    let (node, mut position) = log.into_inner();
    let maximum = ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
    chat.scroll_back = if chat.active {
        chat.scroll_back.min(maximum)
    } else {
        0.0
    };
    position.0.y = maximum - chat.scroll_back;
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load("fonts/Almendra-Regular.ttf");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(12),
                bottom: px(12),
                width: percent(65),
                max_width: px(760),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(20),
            RelativeCursorPosition::default(),
            ChatRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_height: px(200),
                    overflow: Overflow::scroll_y(),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(px(8), px(4)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.02, 0.025, 0.68)),
                ChatLog,
            ))
            .with_child((
                Text::new(""),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.92, 0.81)),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                ChatHistory,
            ));
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    padding: UiRect::all(px(9)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.025, 0.03, 0.93)),
                ChatEntry,
            ))
            .with_children(|entry| {
                entry.spawn((
                    Text::new(""),
                    TextFont {
                        font: font.clone().into(),
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.77, 0.65)),
                    Node {
                        display: Display::None,
                        ..default()
                    },
                    ChatSuggestions,
                ));
                entry.spawn((
                    Text::new("> |"),
                    TextFont {
                        font: font.clone().into(),
                        font_size: FontSize::Px(22.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.96, 0.86)),
                    ChatDraft,
                ));
            });
        });
}

#[allow(clippy::type_complexity)]
fn display(
    chat: Res<ChatState>,
    game: Res<crate::app::GameState>,
    window: Single<&Window>,
    suggestions_ui: Single<
        (&mut Text, &mut Node),
        (
            With<ChatSuggestions>,
            Without<ChatRoot>,
            Without<ChatEntry>,
            Without<ChatLog>,
        ),
    >,
    mut panels: Query<
        (&mut Node, Option<&ChatRoot>, Option<&ChatEntry>),
        Or<(With<ChatRoot>, With<ChatEntry>, With<ChatLog>)>,
    >,
    mut draft: Single<
        &mut Text,
        (
            With<ChatDraft>,
            Without<ChatHistory>,
            Without<ChatSuggestions>,
        ),
    >,
    mut history: Single<
        &mut Text,
        (
            With<ChatHistory>,
            Without<ChatDraft>,
            Without<ChatSuggestions>,
        ),
    >,
) {
    let suggestions = chat.suggestion_text();
    let (mut suggestion_text, mut suggestion_node) = suggestions_ui.into_inner();
    suggestion_text.set_if_neq(Text::new(suggestions.clone()));
    suggestion_node.display = if suggestions.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    let root_display = if !game.paused && (chat.active || chat.recent > 0.0) {
        Display::Flex
    } else {
        Display::None
    };
    let entry_display = if chat.active {
        Display::Flex
    } else {
        Display::None
    };
    for (mut node, root, entry) in &mut panels {
        let desired = if root.is_some() {
            root_display
        } else if entry.is_some() {
            entry_display
        } else if chat.history.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if node.display != desired {
            node.display = desired;
        }
    }
    // Scroll a single-line editor around its caret; retain the whole message.
    let columns = ((window.width() * 0.65).min(760.0) / 18.0 - 6.0).max(8.0) as usize;
    let start = chat.caret.saturating_sub(columns);
    let end = (start + columns).min(chat.draft.len());
    let before: String = chat.draft[start..chat.caret].iter().collect();
    let after: String = chat.draft[chat.caret..end].iter().collect();
    draft.set_if_neq(Text::new(format!(
        "> {}{}|{}",
        if start > 0 { "…" } else { "" },
        before,
        after
    )));
    let recent = chat
        .history
        .iter()
        .skip(if chat.active {
            0
        } else {
            chat.history.len().saturating_sub(6)
        })
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    history.set_if_neq(Text::new(recent));
}

#[cfg(test)]
mod tests;
