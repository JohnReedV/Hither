//! Seeded, locally inhibited forest sites. Strong jitter breaks rows; nearby
//! candidates compete for growing room while coherent stand masks form glades.
use super::*;

pub(super) struct Site {
    pub position: Vec2,
    pub hash: u64,
}

fn raw(seed: u64, cell: IVec2, spacing: f32) -> Site {
    let mut h = seed
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    let mut rng = Rng(h);
    Site {
        position: (cell.as_vec2() + Vec2::new(rng.range(0.04, 0.96), rng.range(0.04, 0.96)))
            * spacing,
        hash: h,
    }
}

pub(super) fn site(seed: u64, cell: IVec2, spacing: f32, separation: f32) -> Option<Site> {
    site_with_alpine(seed, cell, spacing, separation, false)
}

/// Snow spruces may occupy stable alpine ground above their own snowline.
/// Keep the same seeded candidates, spacing, stand masks and lowland density.
pub(super) fn site_with_alpine(
    seed: u64,
    cell: IVec2,
    spacing: f32,
    separation: f32,
    alpine: bool,
) -> Option<Site> {
    debug_assert!(separation < spacing);
    let s = raw(seed, cell, spacing);
    // Neighbor priority uses the unmasked candidate set: no exploration-order
    // dependence and only nine cheap hashes, even in collision hot paths.
    for x in -1..=1 {
        for z in -1..=1 {
            if x == 0 && z == 0 {
                continue;
            }
            let other_cell = cell + IVec2::new(x, z);
            let other = raw(seed, other_cell, spacing);
            if (other.hash, other_cell.x, other_cell.y) < (s.hash, cell.x, cell.y)
                && s.position.distance_squared(other.position) < separation * separation
            {
                return None;
            }
        }
    }
    let edge = ((crate::world::biome::forest_amount(s.position) - 0.5) * 2.0).clamp(0.0, 1.0);
    let habitat = if alpine
        && crate::world::biome::mountain_amount(s.position) > 0.0
        && crate::world::terrain::height(s.position) > crate::world::biome::MOUNTAIN_SNOW_MIN_Y
    {
        1.0
    } else {
        crate::world::biome::mountain_habitat(s.position)
    };
    let chance = crate::world::biome::stand_density(s.position) * edge * habitat;
    let roll = ((s.hash >> 32) as u32) as f32 / u32::MAX as f32;
    (roll < chance).then_some(s)
}

pub(super) fn age_scale(hash: u64) -> f32 {
    let mut rng = Rng(hash ^ 0x616765);
    let age = rng.unit();
    if age < 0.12 {
        rng.range(0.55, 0.85)
    } else if age < 0.30 {
        rng.range(0.90, 1.18)
    } else {
        rng.range(1.25, 1.70)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forests_have_dense_stands_clearings_and_mixed_ages_without_rows() {
        let (mut candidates, mut trees, mut gaps, mut young, mut mature) = (0, 0, 0, 0, 0);
        let mut aligned = 0;
        for x in -150..150 {
            for z in -150..150 {
                let cell = IVec2::new(x, z);
                let raw = raw(721, cell, 3.0);
                if crate::world::biome::forest_amount(raw.position) < 0.99 {
                    continue;
                }
                let density = crate::world::biome::stand_density(raw.position);
                if density == 0.0 {
                    gaps += 1;
                    assert!(site(721, cell, 3.0, 2.0).is_none());
                }
                if density < 0.8 {
                    continue;
                }
                candidates += 1;
                if let Some(p) = site(721, cell, 3.0, 2.0) {
                    trees += 1;
                    assert_eq!(p.position, site(721, cell, 3.0, 2.0).unwrap().position);
                    aligned += usize::from(p.position.x.fract() == 0.5);
                    let scale = age_scale(p.hash);
                    young += usize::from(scale < 0.85);
                    mature += usize::from(scale > 1.25);
                }
            }
        }
        let density = trees as f32 / candidates as f32;
        assert!(
            (0.50..0.90).contains(&density),
            "dense stands must retain most sites: {density}"
        );
        assert!(gaps > 100, "coherent open glades must exist");
        assert!(young > trees / 15 && mature > trees / 2);
        assert!(
            aligned < trees / 100,
            "positions must not line up on a regular grid"
        );
    }
}
