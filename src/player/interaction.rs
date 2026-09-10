//! Crosshair picking for the ray-marched scene (no enlarged aim-assist cone).
use bevy::prelude::*;

const REACH: f32 = 3.5;

#[cfg(test)]
pub(crate) fn target_apple(
    origin: Vec3,
    direction: Vec3,
    player: Vec3,
    apples: &[Vec3],
) -> Option<usize> {
    let direction = direction.try_normalize()?;
    let (index, travel) = apple_candidate(origin, direction, apples)?;
    fruit_reachable(origin, direction, player, apples[index], travel).then_some(index)
}

pub(crate) fn apple_candidate(
    origin: Vec3,
    direction: Vec3,
    apples: &[Vec3],
) -> Option<(usize, f32)> {
    apples
        .iter()
        .enumerate()
        .filter_map(|(index, center)| {
            crate::world::orchard::apple_hit(origin, direction, *center).map(|t| (index, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

pub(crate) fn fruit_reachable(
    origin: Vec3,
    direction: Vec3,
    player: Vec3,
    center: Vec3,
    travel: f32,
) -> bool {
    if player.distance(center) > REACH || terrain_occludes(origin, origin + direction * travel) {
        return false;
    }
    true
}

/// The terrain is affine inside each grid triangle. Check the segment endpoints
/// and every crossed x/z grid edge and x+z diagonal; no sampling can skip a ridge.
fn terrain_occludes(start: Vec3, end: Vec3) -> bool {
    let below = |p: Vec3| p.y - crate::world::terrain::height(p.xz()) < 0.001;
    if below(start) || below(end) {
        return true;
    }
    for (a, b) in [
        (start.x, end.x),
        (start.z, end.z),
        (start.x + start.z, end.x + end.z),
    ] {
        if (b - a).abs() < 1e-6 {
            continue;
        }
        for boundary in a.min(b).ceil() as i32..=a.max(b).floor() as i32 {
            let t = (boundary as f32 - a) / (b - a);
            if below(start.lerp(end, t)) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn must_hit_visible_fruit_not_old_aim_cone() {
        let eye = Vec3::new(2.0, 1.25, 3.0);
        let center = eye - Vec3::Z * 2.0;
        assert_eq!(target_apple(eye, -Vec3::Z, eye, &[center]), Some(0));
        assert_eq!(
            target_apple(eye, -Vec3::Z, eye, &[center + Vec3::X * 0.13]),
            Some(0)
        );
        assert_eq!(
            target_apple(eye, -Vec3::Z, eye, &[center + Vec3::X * 0.15]),
            None
        );
        assert_eq!(target_apple(eye, -Vec3::Z, eye, &[eye + Vec3::Z]), None);
    }

    #[test]
    fn nearest_hit_wins_and_reach_is_from_player_not_camera() {
        let eye = Vec3::new(2.0, 1.25, 3.0);
        let near = eye - Vec3::Z;
        let far = eye - Vec3::Z * 2.0;
        assert_eq!(target_apple(eye, -Vec3::Z, eye, &[far, near]), Some(1));
        assert_eq!(
            target_apple(eye, -Vec3::Z, eye + Vec3::Z * 3.0, &[near]),
            None
        );
        let camera = eye + Vec3::Z * 3.0;
        // Center gate is open, and an offset follow camera does not reduce reach.
        assert_eq!(target_apple(camera, -Vec3::Z, eye, &[near]), Some(0));
        let eye = Vec3::new(0.0, 1.25, 3.0);
        assert_eq!(
            target_apple(eye + Vec3::Z * 3.0, -Vec3::Z, eye, &[eye - Vec3::Z]),
            Some(0)
        );
    }

    #[test]
    fn removed_spawn_tree_does_not_occlude_apples() {
        let eye = Vec3::new(0.0, 1.25, 1.0);
        assert_eq!(
            target_apple(eye, -Vec3::Z, eye, &[Vec3::new(0.0, 1.25, -1.0)]),
            Some(0)
        );
        // The removed trunk must not block picking through the origin.
        let eye = Vec3::new(0.0, 1.1, 2.0);
        assert_eq!(
            target_apple(eye, -Vec3::Z, eye, &[Vec3::new(0.0, 1.1, -1.2)]),
            Some(0)
        );
        assert_eq!(
            target_apple(eye, -Vec3::Z, eye, &[Vec3::new(0.0, 1.1, 1.45)]),
            Some(0)
        );
    }

    #[test]
    fn empty_scene_and_invalid_direction_do_not_pick() {
        assert_eq!(target_apple(Vec3::Y, Vec3::Z, Vec3::Y, &[]), None);
        assert_eq!(target_apple(Vec3::Y, Vec3::ZERO, Vec3::Y, &[Vec3::Y]), None);
    }
}
