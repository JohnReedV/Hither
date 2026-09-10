use crate::app::GameState;
use crate::player::camera::{CameraRig, CameraView};
use crate::player::interaction;
use crate::ui::chat;
use crate::world::orchard;
use bevy::prelude::*;
#[cfg(test)]
use std::time::SystemTime;
#[cfg(test)]
use std::time::UNIX_EPOCH;

#[cfg(test)]
pub(crate) const MAX_APPLES: usize = 5;
pub(crate) const APPLE_RADIUS: f32 = 0.14;
#[cfg(test)]
pub(crate) const MIN_APPLE_SPACING: f32 = 0.34;
// Legacy spawn-tree growth is retained only for geometry regression tests.
#[derive(Resource)]
#[cfg_attr(not(test), derive(Default))]
pub(crate) struct OrchardState {
    pub(crate) apples: Vec<Vec3>,
    pub(crate) harvested: u32,
    #[cfg(test)]
    pub(crate) growth_timer: Timer,
    #[cfg(test)]
    pub(crate) random_state: u64,
}

#[cfg(test)]
impl Default for OrchardState {
    fn default() -> Self {
        let time_seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos() as u64);

        Self {
            // The spawn tree is removed; only wild trees supply fruit.
            apples: Vec::new(),
            harvested: 0,
            growth_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            random_state: time_seed
                ^ (u64::from(std::process::id()).rotate_left(32))
                ^ 0x6d2b_79f5_aa12_3c47,
        }
    }
}

#[cfg(test)]
impl OrchardState {
    pub(crate) fn next_random(&mut self) -> u32 {
        self.random_state = self
            .random_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.random_state >> 32) as u32
    }

    pub(crate) fn next_random_unit(&mut self) -> f32 {
        self.next_random() as f32 / u32::MAX as f32
    }

    pub(crate) fn random_apple_position(&mut self) -> Option<Vec3> {
        for _ in 0..256 {
            let candidate = orchard::hanging_position(
                self.next_random(),
                self.next_random_unit(),
                self.next_random_unit(),
            );
            let spaced_out = self
                .apples
                .iter()
                .all(|apple| candidate.distance(*apple) >= MIN_APPLE_SPACING);

            if spaced_out && orchard::fruit_has_clearance(candidate) {
                return Some(candidate);
            }
        }

        None
    }
}

#[cfg(test)]
pub(crate) fn grow_apples(
    time: Res<Time>,
    game: Res<GameState>,
    mut orchard: ResMut<OrchardState>,
) {
    if game.paused {
        return;
    }

    orchard.growth_timer.tick(time.delta());
    let checks = orchard.growth_timer.times_finished_this_tick();
    for _ in 0..checks {
        if orchard.apples.len() >= MAX_APPLES || !orchard.next_random().is_multiple_of(100) {
            continue;
        }

        if let Some(apple) = orchard.random_apple_position() {
            orchard.apples.push(apple);
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects independent player and orchard resources."
)]
pub(crate) fn pick_fruit(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<GameState>,
    chat: Res<chat::ChatState>,
    rig: Res<CameraRig>,
    view: Res<CameraView>,
    mut orchard: ResMut<OrchardState>,
    trees: Query<&orchard::wild::WildTree>,
    assets: Option<Res<orchard::wild::WildAssets>>,
    mut wild: Option<ResMut<orchard::wild::Streaming>>,
) {
    if rig.is_spectating()
        || game.paused
        || chat.blocks_gameplay()
        || !rig.mouse_captured
        || !keys.just_pressed(KeyCode::KeyE)
    {
        return;
    }

    let Some(direction) = view.forward.try_normalize() else {
        return;
    };
    let spawn = interaction::apple_candidate(view.position, direction, &orchard.apples);
    let target = assets
        .as_deref()
        .zip(wild.as_deref())
        .and_then(|(assets, state)| {
            orchard::wild::target_fruit(view.position, direction, &trees, assets, state)
        });
    // Compare surfaces before reach/occlusion: foreground fruit must block fruit behind it.
    if let Some((cell, slot, center, travel, apple)) =
        target.filter(|t| spawn.is_none_or(|s| t.3 < s.1))
    {
        if interaction::fruit_reachable(view.position, direction, rig.position, center, travel)
            && !orchard::wild::occludes(view.position, direction, travel, &trees)
        {
            orchard::wild::harvest(wild.as_deref_mut().unwrap(), cell, slot, apple);
            if apple {
                orchard.harvested += 1;
            }
        }
    } else if let Some((index, travel)) = spawn
        && interaction::fruit_reachable(
            view.position,
            direction,
            rig.position,
            orchard.apples[index],
            travel,
        )
        && !orchard::wild::occludes(view.position, direction, travel, &trees)
    {
        orchard.apples.swap_remove(index);
        orchard.harvested += 1;
    }
}
