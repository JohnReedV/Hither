//! Seeded continuous climate shared by CPU vegetation and GPU ground.
use bevy::prelude::*;
use std::sync::OnceLock;

pub(crate) fn world_seed() -> u64 {
    static SEED: OnceLock<u64> = OnceLock::new();
    *SEED.get_or_init(|| {
        std::env::var("HITHER_WORLD_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| {
                if cfg!(test) {
                    return 721;
                }
                use std::hash::{BuildHasher, Hasher};
                std::collections::hash_map::RandomState::new()
                    .build_hasher()
                    .finish()
            })
    })
}

pub(crate) fn shader_seed() -> UVec4 {
    let seed = world_seed();
    UVec4::new(seed as u32 ^ (seed >> 32) as u32, 0, 0, 0)
}

fn lattice(p: IVec2, seed: u32) -> f32 {
    let mut h =
        (p.x as u32).wrapping_mul(0x9e3779b9) ^ (p.y as u32).wrapping_mul(0x85ebca6b) ^ seed;
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
    h ^= h >> 16;
    (h >> 8) as f32 / 16777215.0
}

fn noise(p: Vec2, seed: u32) -> f32 {
    let cell = p.floor();
    let f = p - cell;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let c = cell.as_ivec2();
    let a = lattice(c, seed).lerp(lattice(c + IVec2::X, seed), u.x);
    let b = lattice(c + IVec2::Y, seed).lerp(lattice(c + IVec2::ONE, seed), u.x);
    a.lerp(b, u.y)
}

pub(crate) fn snow_for_seed(p: Vec2, seed: u64) -> f32 {
    let s = seed as u32 ^ (seed >> 32) as u32;
    let q = p / 180.0 + Vec2::new(0.37, 0.61);
    let climate =
        noise(q, s) * 0.75 + noise(q * 2.03 + Vec2::new(19.1, -7.3), s ^ 0xa511e9b3) * 0.25;
    let t = ((climate - 0.47) / 0.06).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// No castle exemption: origin can be grassland, winter or a transition.
/// Mirror integer hashing/interpolation in sdf_scene.wgsl.
pub(crate) fn snow_amount(p: Vec2) -> f32 {
    snow_for_seed(p, world_seed())
}

pub(crate) fn forest_for_seed(p: Vec2, seed: u64) -> f32 {
    let s = seed as u32 ^ (seed >> 32) as u32 ^ 0x666f7265;
    let q = p / 240.0 + Vec2::new(8.17, -3.41);
    let cover = noise(q, s) * 0.78 + noise(q * 2.71 + Vec2::new(-9.3, 17.8), s ^ 0xa511e9b3) * 0.22;
    let t = ((cover - 0.46) / 0.08).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Independent, continuous woodland regions. Mirror in sdf_scene.wgsl.
pub(crate) fn forest_amount(p: Vec2) -> f32 {
    forest_for_seed(p, world_seed())
}

/// Within woodland, coherent gaps and regeneration stands at a smaller scale
/// than the biome. Entire patches open up, not independent missing trees.
pub(crate) fn stand_density(p: Vec2) -> f32 {
    let seed = shader_seed().x ^ 0x7374616e;
    let clearing = noise(p / 28.0 + Vec2::new(5.7, -3.1), seed);
    let gap = ((clearing - 0.60) / 0.15).clamp(0.0, 1.0);
    (1.0 - gap * gap * (3.0 - 2.0 * gap)) * (0.82 + 0.18 * noise(p / 9.0, seed ^ 0xa511e9b3))
}

/// Continental mountain belts are independent of woodland and temperature.
/// A broad foothill apron preserves the castle and avoids a hard biome seam.
pub(crate) fn mountain_for_seed(p: Vec2, seed: u64) -> f32 {
    let s = seed as u32 ^ (seed >> 32) as u32 ^ 0x616c7069;
    let region = noise(p / 700.0 + Vec2::new(-4.7, 9.2), s) * 0.8
        + noise(p / 290.0 + Vec2::new(7.1, -2.8), s ^ 0xa511e9b3) * 0.2;
    smooth_range(0.53, 0.73, region) * smooth_range(100.0, 280.0, p.length())
}

pub(crate) fn mountain_amount(p: Vec2) -> f32 {
    mountain_for_seed(p, world_seed())
}

/// Branching, domain-warped ridges with subordinate spurs and erosion gullies.
/// No periodic cross-section or saturated summit mask: relief continues across
/// the crest instead of turning entire ranges into level, sheer-sided mesas.
/// Mirror this formula in WGSL so collision and visible terrain stay aligned.
fn mountain_relief(p: Vec2, seed: u64) -> f32 {
    let s = seed as u32 ^ (seed >> 32) as u32 ^ 0x7065616b;
    let warp = Vec2::new(
        noise(p / 310.0, s),
        noise(p / 310.0 + Vec2::new(37.2, -19.4), s ^ 0xa511e9b3),
    ) - Vec2::splat(0.5);
    let q = p + warp * 150.0;
    let axis = Vec2::new(q.x * 0.86 + q.y * 0.51, q.y * 0.86 - q.x * 0.51);
    let main = noise(axis / Vec2::new(240.0, 390.0), s) * 2.0 - 1.0;
    let ridge = 1.0 - (main * main + 0.0036).sqrt();
    let spur = noise((axis + Vec2::new(83.1, -27.7)) / 105.0, s ^ 0x73706972) * 2.0 - 1.0;
    let secondary = 1.0 - (spur * spur + 0.0064).sqrt();
    let ribs = noise(q / 38.0, s ^ 0x72696273) * 2.0 - 1.0;
    let rib = 1.0 - (ribs * ribs + 0.0144).sqrt();
    // Regional rock strength changes the whole profile: broad eroded massifs,
    // angular ranges, and occasional sharper alpine chains coexist.
    let character = noise(p / 580.0 + Vec2::new(17.2, -9.6), s ^ 0x7374796c);
    let broad = ridge * ridge * (0.65 + 0.35 * ridge);
    let sharp = ridge * ridge * ridge * ridge;
    let mass = broad.lerp(sharp, smooth_range(0.30, 0.75, character));
    let crown = noise(q / 145.0, s ^ 0x63726f77);
    let backbone = mass * (135.0 + secondary * 145.0 + crown * 65.0);
    // Smaller spurs taper into talus fans at the foot of the mountain.
    let foothills = ridge * ridge * (rib * 32.0 + secondary * 24.0);
    let gullies = (noise(q / 17.0, s ^ 0x67756c6c) - 0.5) * 17.0;
    let rubble = (noise(p / 5.0, s ^ 0x73637265) - 0.5) * 1.5;
    5.0 + noise(p / 90.0, s ^ 0x76616c6c) * 9.0
        + backbone
        + foothills
        + ridge * ridge * (gullies + rubble)
}

/// Habitat follows the same rendered support triangles. Bare cliffs and the
/// alpine zone cannot grow full-size trees or grass through exposed rock.
pub(crate) fn mountain_habitat(p: Vec2) -> f32 {
    let mountain = mountain_amount(p);
    if mountain == 0.0 {
        return 1.0;
    }
    let h = terrain_height(p);
    let dx = (terrain_height(p + Vec2::X) - terrain_height(p - Vec2::X)) * 0.5;
    let dz = (terrain_height(p + Vec2::Y) - terrain_height(p - Vec2::Y)) * 0.5;
    let up = 1.0 / (1.0 + dx * dx + dz * dz).sqrt();
    let exposed = (1.0 - smooth_range(0.55, 0.83, up)) * smooth_range(0.02, 0.20, mountain);
    1.0 - mountain.max(exposed)
        * (1.0 - smooth_range(0.72, 0.92, up) * (1.0 - smooth_range(55.0, 115.0, h)))
}

/// Mountain snow starts above the same 200 m cutoff as snow-laden trees,
/// including every nonzero mountain transition weight.
/// Mirror surface_snow_at in sdf_scene.wgsl.
pub(crate) const MOUNTAIN_SNOW_MIN_Y: f32 = 200.0;
/// Shared altitude restriction for mountain surface snow and snow-laden trees.
pub(crate) fn mountain_snow_allowed(mountain: f32, y: f32) -> bool {
    mountain == 0.0 || (y.is_finite() && y > MOUNTAIN_SNOW_MIN_Y)
}

fn surface_snow_at(p: Vec2, mountain: f32, h: f32, normal: Vec3, climate: f32) -> f32 {
    if !mountain_snow_allowed(mountain, h) {
        return 0.0;
    }
    if mountain == 0.0 {
        return ground_snow(p, climate);
    }
    let line = (MOUNTAIN_SNOW_MIN_Y + noise(p / 31.0, shader_seed().x ^ 0x736e6c6e) * 38.0
        - normal.z * 14.0)
        .max(MOUNTAIN_SNOW_MIN_Y);
    let alpine = climate.max(smooth_range(line, line + 25.0, h))
        * smooth_range(MOUNTAIN_SNOW_MIN_Y, MOUNTAIN_SNOW_MIN_Y + 25.0, h);
    // Blend accumulation BEFORE patch formation: snow retreats into irregular
    // islands, instead of fading an entire snowfield uniformly to green.
    let accumulation = climate.lerp(alpine, smooth_range(0.0, 1.0, mountain));
    let retention = 1.0_f32.lerp(
        smooth_range(0.58, 0.88, normal.y),
        smooth_range(0.02, 0.20, mountain),
    );
    ground_snow(p, accumulation) * retention
}

/// Shared contact coverage for vegetation burial and footprints.
pub(crate) fn surface_snow(p: Vec2) -> f32 {
    let climate = snow_amount(p);
    let mountain = mountain_amount(p);
    if mountain == 0.0 {
        return ground_snow(p, climate);
    }
    let gradient = crate::world::terrain::gradient(p);
    let normal = Vec3::new(-gradient.x, 1.0, -gradient.y).normalize();
    surface_snow_at(p, mountain, terrain_height(p), normal, climate)
}

/// Metres above datum. Keep the formula in sync with terrain_height in WGSL.
/// Low rolling forests blend into independent alpine ranges.
pub(crate) fn terrain_for_seed(p: Vec2, seed: u64) -> f32 {
    let s = seed as u32 ^ (seed >> 32) as u32 ^ 0x68696c6c;
    let wooded = forest_for_seed(p, seed);
    let mountain = mountain_for_seed(p, seed);
    if wooded == 0.0 && mountain == 0.0 {
        return 0.0;
    }
    let roll = 3.8 * noise(p / 38.0, s) + 0.65 * noise(p / 15.0 + Vec2::splat(7.3), s ^ 0xa511e9b3);
    let region = smooth_range(
        0.64,
        0.84,
        noise(p / 310.0 + Vec2::new(13.7, -8.1), s ^ 0x72617265),
    );
    let warp = (noise(p / 125.0, s ^ 0x77617270) - 0.5) * 1.6;
    let across = p.dot(Vec2::new(0.8, 0.6)) / 48.0 + warp;
    let along = p.dot(Vec2::new(-0.6, 0.8)) / 190.0;
    let ridge = noise(Vec2::new(across, along), s ^ 0x72696467);
    let hills = 28.0 * region * ridge * ridge;
    let lowlands = wooded * (roll + hills) * smooth_range(12.0, 52.0, p.length());
    if mountain == 0.0 {
        return lowlands;
    }
    lowlands.lerp(mountain_relief(p, seed), mountain)
}

pub(crate) fn smooth_range(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(crate) fn terrain_height(p: Vec2) -> f32 {
    crate::world::terrain::height(p)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Biome {
    Plains,
    TemperateForest,
    Tundra,
    BorealForest,
    Mountains,
}

pub(crate) fn at(p: Vec2) -> Biome {
    if mountain_amount(p) >= 0.5 {
        return Biome::Mountains;
    }
    match (snow_amount(p) >= 0.5, forest_amount(p) >= 0.5) {
        (false, false) => Biome::Plains,
        (false, true) => Biome::TemperateForest,
        (true, false) => Biome::Tundra,
        (true, true) => Biome::BorealForest,
    }
}

/// Small coherent snow patches, rather than green ground uniformly tinted white.
/// Shared with ground_snow in WGSL. Does not change regional biome placement.
pub(crate) fn ground_snow(p: Vec2, climate: f32) -> f32 {
    if climate <= 0.0 || climate >= 1.0 {
        return climate;
    }
    let seed = shader_seed().x ^ 0x736e6f77;
    // Snow cannot appear before the dormant-grass stage (climate 0.08).
    let patch = (noise(p * 0.7, seed) * 0.7 + noise(p * 2.1 + Vec2::splat(13.7), seed) * 0.3)
        .clamp(0.20, 0.80);
    let t = ((climate - patch + 0.12) / 0.24).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mountain_snow_obeys_tree_cutoff_and_feathers_above_snowline() {
        for x in -20..20 {
            let p = Vec2::new(x as f32 * 0.37, 4.2);
            for mountain in [f32::MIN_POSITIVE, 0.001, 0.1, 0.5, 1.0] {
                for h in [
                    -100.0,
                    0.0,
                    199.999,
                    MOUNTAIN_SNOW_MIN_Y,
                    f32::NAN,
                    f32::INFINITY,
                ] {
                    assert!(!mountain_snow_allowed(mountain, h));
                    for climate in [0.0, 0.5, 1.0] {
                        assert_eq!(surface_snow_at(p, mountain, h, Vec3::Y, climate), 0.0);
                    }
                }
                assert!(mountain_snow_allowed(mountain, 200.001));
                assert!(surface_snow_at(p, mountain, 300.0, Vec3::Y, 1.0) > 0.0);
            }
            for climate in [0.0, 0.5, 1.0] {
                assert_eq!(
                    surface_snow_at(p, 0.0, 20.0, Vec3::Y, climate),
                    ground_snow(p, climate)
                );
                let mut previous = 0.0;
                for i in 0..=1000 {
                    let h = 190.0 + i as f32 * 0.1;
                    let cover = surface_snow_at(p, 1.0, h, Vec3::Y, climate);
                    assert!((cover - previous).abs() < 0.05, "abrupt alpine snowline");
                    if h <= 200.0 {
                        assert_eq!(cover, 0.0);
                    }
                    previous = cover;
                }
                assert_eq!(previous, 1.0);
            }
        }
    }

    #[test]
    fn mountain_ranges_have_peaks_valleys_cliffs_and_continuous_borders() {
        for seed in [1, 721, u64::MAX] {
            let (mut peaks, mut valleys, mut cliffs, mut transitions) = (0, 0, 0, 0);
            for x in -150..150 {
                for z in -150..150 {
                    let p = Vec2::new(x as f32 * 23.7, z as f32 * 23.7);
                    let m = mountain_for_seed(p, seed);
                    let h = terrain_for_seed(p, seed);
                    assert!(h.is_finite() && (0.0..=360.0).contains(&h));
                    assert_eq!(h, terrain_for_seed(p, seed));
                    let adjacent = terrain_for_seed(p + Vec2::splat(0.01), seed);
                    assert!((h - adjacent).abs() < 0.4, "discontinuous range at {p}");
                    transitions += usize::from(m > 0.05 && m < 0.95);
                    if m > 0.99 {
                        peaks += usize::from(h > 200.0);
                        valleys += usize::from(h < 18.0);
                        let slope = (terrain_for_seed(p + Vec2::X, seed) - h).abs();
                        cliffs += usize::from(slope > 2.0);
                    }
                }
            }
            assert!(
                peaks > 20 && valleys > 20 && cliffs > 20 && transitions > 100,
                "missing mountain features: {peaks}, {valleys}, {cliffs}, {transitions}"
            );
            assert_eq!(mountain_for_seed(Vec2::ZERO, seed), 0.0);
        }
    }

    #[test]
    fn high_ranges_have_varied_slopes_instead_of_flat_crowns() {
        for seed in [1, 721, u64::MAX] {
            let (mut high, mut flat, mut slopes, mut rugged) = (0, 0, 0, 0);
            for x in -130..130 {
                for z in -130..130 {
                    let p = Vec2::new(x as f32 * 27.3, z as f32 * 27.3);
                    if mountain_for_seed(p, seed) < 0.999 {
                        continue;
                    }
                    let h = terrain_for_seed(p, seed);
                    let slope = Vec2::new(
                        terrain_for_seed(p + Vec2::X, seed) - h,
                        terrain_for_seed(p + Vec2::Y, seed) - h,
                    )
                    .length();
                    if h > 150.0 {
                        high += 1;
                        flat += usize::from(slope < 0.15);
                        slopes += usize::from((0.3..1.5).contains(&slope));
                        rugged += usize::from(slope > 1.5);
                    }
                }
            }
            assert!(
                high > 100 && slopes > 20 && rugged > 20,
                "seed {seed}: high={high}, slopes={slopes}, rugged={rugged}"
            );
            assert!(
                flat * 10 < high,
                "seed {seed}: too many flat crowns: {flat}/{high}"
            );
        }
    }

    #[test]
    fn alpine_summits_are_bare_and_snow_collects_on_gentle_slopes() {
        let mut summits = 0;
        let mut snowfields = 0;
        for x in -70..70 {
            for z in -70..70 {
                let p = Vec2::new(x as f32 * 23.7, z as f32 * 23.7);
                if mountain_amount(p) < 1.0 || terrain_height(p) < 185.0 {
                    continue;
                }
                summits += 1;
                assert_eq!(mountain_habitat(p), 0.0);
                let cover = surface_snow(p);
                assert!((0.0..=1.0).contains(&cover));
                snowfields += usize::from(cover > 0.8);
            }
        }
        assert!(
            summits > 10 && snowfields > 5,
            "missing alpine habitat: {summits}, {snowfields}"
        );
    }

    #[test]
    fn forests_have_common_low_relief_and_rare_smooth_hills() {
        for seed in [1, 721, u64::MAX] {
            let mut counts = [[0usize; 3]; 2];
            for x in -160..160 {
                for z in -160..160 {
                    let p = Vec2::new(x as f32 * 19.7, z as f32 * 19.7);
                    let h = terrain_for_seed(p, seed);
                    assert_eq!(h, terrain_for_seed(p, seed));
                    if mountain_for_seed(p, seed) > 0.0 {
                        continue;
                    }
                    assert!((0.0..=33.0).contains(&h));
                    let dx = (terrain_for_seed(p + Vec2::X * 0.05, seed) - h) / 0.05;
                    let dz = (terrain_for_seed(p + Vec2::Y * 0.05, seed) - h) / 0.05;
                    assert!(
                        Vec2::new(dx, dz).length() < 3.0,
                        "abrupt terrain at {p}: {dx}, {dz}"
                    );
                    if forest_for_seed(p, seed) == 0.0 {
                        assert_eq!(h, 0.0);
                    }
                    if forest_for_seed(p, seed) < 0.99 {
                        continue;
                    }
                    let cold = usize::from(snow_for_seed(p, seed) >= 0.5);
                    counts[cold][0] += 1;
                    counts[cold][1] += usize::from(h > 0.3 && h < 5.0);
                    counts[cold][2] += usize::from(h > 8.0);
                }
            }
            for [total, rolling, hills] in counts {
                assert!(
                    rolling > total * 3 / 4,
                    "most forest should roll gently: {counts:?}"
                );
                assert!(
                    hills > total / 300 && hills < total / 5,
                    "hills should be rare but present: {counts:?}"
                );
            }
            assert_eq!(terrain_for_seed(Vec2::ZERO, seed), 0.0);
        }
    }

    #[test]
    fn terrain_support_and_vegetation_datum_agree_on_slopes() {
        let mut elevated = 0;
        for x in -40..40 {
            for z in -40..40 {
                let p = Vec2::new(x as f32 * 23.0, z as f32 * 23.0);
                let h = terrain_height(p);
                let eye = Vec3::new(p.x, h + crate::player::movement::PLAYER_EYE_HEIGHT, p.y);
                if crate::world::orcs::entrance_overlap(eye, 1.0) {
                    continue;
                }
                assert!(
                    crate::player::movement::world_support_height(
                        eye,
                        crate::player::movement::PLAYER_RADIUS
                    ) >= h - 0.001
                );
                elevated += usize::from(h > 2.0);
                if crate::world::orcs::terrain_clearance(p) == 0.0 {
                    assert_eq!(h, 0.0);
                }
            }
        }
        assert!(elevated > 100);
    }

    #[test]
    fn all_four_biomes_can_contain_the_castle_and_form_coherent_regions() {
        let mut origins = [0; 4];
        for seed in 0..512 {
            let cold = snow_for_seed(Vec2::ZERO, seed) >= 0.5;
            let wooded = forest_for_seed(Vec2::ZERO, seed) >= 0.5;
            origins[cold as usize * 2 + wooded as usize] += 1;
        }
        assert!(origins.iter().all(|n| *n > 50));
        for seed in [1, 721, u64::MAX] {
            let mut regions = [0; 4];
            for x in -30..30 {
                for z in -30..30 {
                    let p = Vec2::new(x as f32 * 37.0, z as f32 * 37.0);
                    let forest = forest_for_seed(p, seed);
                    assert_eq!(forest, forest_for_seed(p, seed));
                    assert!((forest - forest_for_seed(p + Vec2::splat(0.01), seed)).abs() < 0.005);
                    regions[(snow_for_seed(p, seed) >= 0.5) as usize * 2
                        + (forest >= 0.5) as usize] += 1;
                }
            }
            assert!(regions.iter().all(|n| *n > 100));
        }
    }
    #[test]
    fn snow_patches_preserve_endpoints_are_coherent_and_fill_monotonically() {
        let mut patches = Vec::new();
        for x in -10..10 {
            let p = Vec2::new(x as f32 * 0.27, -1.3);
            assert_eq!(ground_snow(p, 0.0), 0.0);
            assert_eq!(ground_snow(p, 1.0), 1.0);
            let mut previous = 0.0;
            for i in 0..=100 {
                let cover = ground_snow(p, i as f32 / 100.0);
                assert!(cover >= previous);
                previous = cover;
            }
            let cover = ground_snow(p, 0.5);
            assert!((cover - ground_snow(p + Vec2::splat(0.001), 0.5)).abs() < 0.025);
            patches.push(cover);
        }
        assert!(patches.iter().any(|c| *c < 0.25));
        assert!(patches.iter().any(|c| *c > 0.75));
    }
    #[test]
    fn seeds_change_castle_biome_and_world_layout() {
        let origins: Vec<_> = (0..256).map(|s| snow_for_seed(Vec2::ZERO, s)).collect();
        assert!(origins.contains(&0.0));
        assert!(origins.contains(&1.0));
        assert!(origins.iter().any(|v| *v > 0.0 && *v < 1.0));
        for seed in [1, 721, u64::MAX] {
            let mut grass = 0;
            let mut winter = 0;
            for x in -20..20 {
                for z in -20..20 {
                    let p = Vec2::new(x as f32 * 37.0, z as f32 * 37.0);
                    let a = snow_for_seed(p, seed);
                    assert_eq!(a, snow_for_seed(p, seed));
                    assert!((a - snow_for_seed(p + Vec2::splat(0.01), seed)).abs() < 0.005);
                    grass += usize::from(a == 0.0);
                    winter += usize::from(a == 1.0);
                }
            }
            assert!(grass > 100 && winter > 100);
        }
    }
}
