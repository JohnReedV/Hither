//! Inspectable environmental storytelling. Relic geometry and text share positions.
use super::*;
use crate::{
    app::GameState,
    player::camera::{CameraRig, CameraView},
    ui::chat::ChatState,
};
pub(super) struct Relic {
    pub at: Vec3,
    pub radius: f32,
    pub entry: usize,
}
const ENTRIES: &[(&str, &str)] = &[
    (
        "The Many-Handed Thief",
        "Mismatched hands reach from this idol. One holds a broken ring; another has been carefully repaired with a child's tiny nails.\n\nThe goblins say their first god stole hands from his own shadows so no useful thing would ever fall beyond his reach. His cleverest theft was not gold, but the secret of sharing: a lone thief can carry only a little. A clan can carry a kingdom.\n\nAt the idol's feet, a scratched warning reads: 'Take from the high folk. Take from the careless. Never take the last bowl from your own fire.'",
    ),
    (
        "Offerings to the Deep Maw",
        "The stone mouth is lined with shed animal teeth, polished by generations of fingers. Coins and bent nails lie in the black bowl beneath it.\n\nThe Deep Maw is the hunger of the mountain. Goblins believe every tunnel is a breath stolen from its sleeping body. Before cutting new rock, miners feed the Maw one thing taken in danger and one thing freely given.\n\nA smaller inscription contradicts the priest's careful carving: 'It likes copper best.' Someone else has scratched below it: 'It likes quiet best. Stop hammering.'",
    ),
    (
        "The Spore Mother's Children",
        "Tiny mushrooms grow from a nest of cord and old bowls beneath the great cap. The oldest bowl has been mended so often that its repairs form a second vessel around the first.\n\nThe Spore Mother teaches that nothing in the dark is wasted. Fallen wood becomes food; a worn hide becomes a nest; the remembered dead become advice. New broods receive a pinch of sacred growing soil, never a coin.\n\nA ring of miniature caps names the children born since the last lean season. One space has been left empty for the next arrival.",
    ),
    (
        "The Ladder of Remembered Names",
        "Each bone rung bears a different mask, crooked ear or missing tooth. The names are knotted into cord instead of written.\n\nThe dead are believed to climb down into the mountain, not up into the sky. During an ancestor vigil, a relative touches each rung and repeats a story: an excellent theft, a foolish quarrel, a meal saved for someone late. A name is added only when three living goblins can tell the same story.\n\nThe newest mask has a grin much larger than the rest. Its cord still smells of fresh smoke.",
    ),
    (
        "The Bowl of the Hidden Moon",
        "The inside of this shallow bowl has been rubbed bright with ash. A sliver of pale stone rests at its center.\n\nMost of these goblins rarely see the moon. They believe it is a stolen lamp which the surface gods keep searching for. Once each cycle, a scout carries its reflection home in a bowl of water. The reflection is poured before the ancestors so they will know that night still belongs to the small and quiet.\n\nA thumbprint beside the rim means the scout returned safely.",
    ),
    (
        "The First Broken Lock",
        "This iron ring once secured a storehouse. The break has been gilded with beaten scraps of copper; the keyhole is stuffed with faded red thread.\n\nA clan tale says a hungry founder opened a lord's granary without spilling a drop of blood. The stolen grain fed three rival broods. Their chiefs tied their knife cords together here and agreed to eat before arguing.\n\nLater raid tallies cover the base, but the oldest instruction remains readable: 'A clever hand brings home more than an angry blade.'",
    ),
    (
        "The Ward Against Unwelcome Eyes",
        "Knots, closed eyes and crossed pupils repeat down the little ladder. A visitor might mistake them for curses.\n\nThey are prayers to be overlooked. Goblin priests cover the idol's eyes during a dangerous expedition, promising to show it everything the raiders saw when they return. A failed raid is mourned with silence; a successful one is retold loudly, including every embarrassing detail.\n\nOne tiny eye has been left open. A note beneath it translates roughly as: 'Someone must watch the soup.'",
    ),
    (
        "The Cauldron That Never Empties",
        "The iron pot has at least a dozen repairs. Its handle chains are made from different kinds of links, and the stirring pole carries hundreds of shallow notches.\n\nThere is no single recipe. Every returning gatherer contributes something: roots, cave caps, stolen barley, or a marrow bone. A ladle is left in the pot overnight for the Spore Mother. The cook insists this is why the stew survives even when the stores run thin.\n\nScratched into a handle is the hall's oldest law: 'Count mouths before counting spoils.'",
    ),
    (
        "A Cook's Record of the Lean Dark",
        "Rows of bowls are crossed out on this board, then drawn again in a smaller hand. Between them is the image of a closed mountain mouth.\n\nA collapse once sealed the high tunnels for an entire growing season. The clan scraped old beams for edible fungus and divided each bowl by age, giving the youngest their share first. When the last passage opened, their first raid took seed, salt and cooking pots rather than weapons.\n\nThe empty space at the bottom is deliberate: the cooks leave room to record the next hard season.",
    ),
    (
        "The Knotted Knife Truce",
        "Knife cords are tied beneath this table, out of sight of anyone standing tall. Some knots are so old they have become part of the wood.\n\nAt a communal meal, every blade must be bound. Grievances may be shouted, mocked, sung or scratched onto the wall, but no blood may be drawn beside the shared pot. Breaking the truce earns a season of washing every bowl and repairing every bridge rope.\n\nOne diner has carved an enormous victorious goblin above a very small rival. The rival has given it a ridiculous nose.",
    ),
    (
        "The Empty Place at the Feast",
        "One bowl contains only a smooth pale counter. No cup has been set beside it.\n\nThis place belongs to those still outside: scouts, gatherers and raiders. Their portions are saved until the last lantern is lowered. If a missing goblin returns, the hall begins the meal again, however late it is. If they do not, their food is returned to the growing beds and their name is sung down the ancestor ladder.\n\nThe counter is warm from being passed from hand to hand.",
    ),
    (
        "Stores for the Next Brood",
        "A barrel lid carries three symbols: a mushroom, a closed fist and a small nest. Beneath them, dozens of owners have added their own marks.\n\nThese are the brood stores. Whatever a raider's reputation, some of each haul must be put aside for children and injured clanmates. Goblins boast about the rarest thing they have stolen, but a second set of hidden marks records how much they gave away.\n\nA crooked note on the rim reads: 'If you steal from this barrel, even the Thief will know your name.'",
    ),
    (
        "The Listening Line",
        "Stolen cups and bent scraps hang in a tangled bundle. Each makes a different note; one has been repaired with a strip of boot leather.\n\nScouts ring two soft notes when they return. Three means a wounded companion. A continuous rattle means falling rock, and every goblin must leave their loot and help. A little bone clacker belongs to children learning the route.\n\nBeside the rope, a warning has been scratched over an older warning: 'Do not pull for jokes. The cook remembers.'",
    ),
    (
        "The Shift That Left Its Tools",
        "A bent pick rests beside a basket of reddish stone. Four sets of tally marks end at different heights.\n\nGoblin miners follow cracks by sound and the taste of damp air. The eldest listens with one ear against the wall; the youngest carries the wedges. They change direction whenever the mountain begins to sing. Straight tunnels are called a boast, and boasts wake the Deep Maw.\n\nA charcoal message explains the abandoned shift: 'Found water. Went around. Keep this wall asleep.'",
    ),
    (
        "The Small Cairn",
        "Each stone bears a thumbprint. Bone charms carry the crooked features of miners who never came home. A bowl beneath them holds a single bright pebble.\n\nThe clan does not reopen a fatal collapse. They build around it, leaving the dead their own quiet passage. Travelers add a pebble and borrow a remembered trick: how to hear loose shale, how to smell rain above the mountain, how to tie a knot with numb fingers.\n\nThe newest mark is tiny: 'He carried my basket when I was small.'",
    ),
    (
        "The Count of Returning Feet",
        "Small footprints lead toward a bowl; large boots point away. The wall is crowded with corrections, crossed-out bags and little triumphant faces.\n\nAt the last rest stop, raiders count companions before spoils. They leave a handful of food for the next exhausted traveler and hang wet bundles above the ground. Missing scouts are marked with an unfinished footprint, never a crossed-out name.\n\nSomeone has added a rule in angry red clay: 'If you can carry a silver plate, you can carry your brother.'",
    ),
];
#[derive(Resource, Default)]
struct Reader {
    open: Option<(IVec2, usize)>,
}
#[derive(Component)]
struct Prompt;
#[derive(Component)]
struct Page;
pub(super) struct LorePlugin;
impl Plugin for LorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Reader>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                inspect
                    .after(crate::player::camera::update_camera_view)
                    .after(crate::ui::menu::pause_on_escape),
            );
    }
}
fn setup(mut commands: Commands) {
    commands.spawn((
        Prompt,
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(19.),
            ..default()
        },
        TextColor(Color::srgb(0.94, 0.84, 0.63)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(48),
            left: percent(30.),
            ..default()
        },
        GlobalZIndex(35),
        Visibility::Hidden,
    ));
    commands.spawn((
        Page,
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.),
            ..default()
        },
        TextColor(Color::srgb(0.91, 0.85, 0.70)),
        BackgroundColor(Color::srgba(0.045, 0.033, 0.024, 0.97)),
        Node {
            position_type: PositionType::Absolute,
            top: px(65),
            right: px(28),
            width: px(440),
            padding: UiRect::all(px(22)),
            ..default()
        },
        GlobalZIndex(40),
        Visibility::Hidden,
    ));
}
fn target(built: &Built, origin: Vec3, direction: Vec3, player: Vec3) -> Option<usize> {
    let direction = direction.try_normalize()?;
    built
        .relics
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            if r.at.distance(player) > 4.5 {
                return None;
            }
            let v = r.at - origin;
            let t = v.dot(direction);
            if t < 0. {
                return None;
            }
            let perpendicular = v.length_squared() - t * t;
            if perpendicular > r.radius * r.radius {
                return None;
            }
            let near = (t - (r.radius * r.radius - perpendicular).max(0.).sqrt()).max(0.);
            if built
                .collision
                .hit(origin, direction, near)
                .is_some_and(|hit| hit + 0.035 < near)
            {
                return None;
            }
            Some((i, near))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}
type RelicText<'w, 's, Marker, Other> =
    Single<'w, 's, (&'static mut Text, &'static mut Visibility), (With<Marker>, Without<Other>)>;

#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects independent input resources and UI queries."
)]
fn inspect(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<GameState>,
    chat: Res<ChatState>,
    rig: Res<CameraRig>,
    view: Res<CameraView>,
    mut reader: ResMut<Reader>,
    mut prompt: RelicText<'_, '_, Prompt, Page>,
    mut page: RelicText<'_, '_, Page, Prompt>,
) {
    *prompt.1 = Visibility::Hidden;
    *page.1 = Visibility::Hidden;
    if game.paused || chat.blocks_gameplay() || !rig.mouse_captured {
        return;
    }
    let Some(den) = near(rig.position.xz()).filter(|d| d.ready()) else {
        reader.open = None;
        return;
    };
    let built = den.built();
    if let Some((cell, i)) = reader.open
        && (cell != den.cell
            || built
                .relics
                .get(i)
                .is_none_or(|r| (den.center + r.at).distance(rig.position) > 5.5))
    {
        reader.open = None;
    }
    let selected = target(
        &built,
        view.position - den.center,
        view.forward,
        rig.position - den.center,
    );
    if keys.just_pressed(KeyCode::KeyE) {
        reader.open = if reader.open.is_some() {
            None
        } else {
            selected.map(|i| (den.cell, i))
        };
    }
    if let Some((_, i)) = reader.open {
        let (title, text) = ENTRIES[built.relics[i].entry];
        page.0
            .set_if_neq(Text::new(format!("{title}\n\n{text}\n\n[E] Close")));
        *page.1 = Visibility::Visible;
    } else if let Some(i) = selected {
        prompt.0.set_if_neq(Text::new(format!(
            "[E] Inspect {}",
            ENTRIES[built.relics[i].entry].0
        )));
        *prompt.1 = Visibility::Visible;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn goblin_lore_can_be_inspected_and_is_occluded_by_walls() {
        let mut built = Built {
            meshes: vec![],
            collision: RayMesh::new(std::iter::empty()),
            floors: RayMesh::new(std::iter::empty()),
            lamps: vec![],
            relics: vec![Relic {
                at: Vec3::new(0., 1., -2.),
                radius: 0.3,
                entry: 0,
            }],
        };
        assert_eq!(target(&built, Vec3::Y, -Vec3::Z, Vec3::ZERO), Some(0));
        assert_eq!(target(&built, Vec3::Y, Vec3::X, Vec3::ZERO), None);
        assert_eq!(target(&built, Vec3::Y, -Vec3::Z, Vec3::Z * 5.), None);
        use crate::rendering::geometry::Triangle;
        built.collision = RayMesh::new(
            [Triangle([
                Vec3::new(-2., -1., -1.),
                Vec3::new(2., -1., -1.),
                Vec3::new(0., 3., -1.),
            ])]
            .into_iter(),
        );
        assert_eq!(target(&built, Vec3::Y, -Vec3::Z, Vec3::ZERO), None);
        assert_eq!(ENTRIES.len(), 16);
    }
}
