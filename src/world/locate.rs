//! Seeded queries independent of streamed entities, run off the game thread.
use super::{biome, goblin_dens, orcs, terrain};
use bevy::prelude::*;

pub(crate) const USAGE: &str = "Usage: /locate <target>. Targets: goblin_den, orc_den, plains, temperate_forest, tundra, boreal_forest, mountains.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    GoblinDen,
    OrcDen,
    Plains,
    TemperateForest,
    Tundra,
    BorealForest,
    Mountains,
}
impl Target {
    pub(crate) const NAMES: &[&str] = &[
        "goblin_den",
        "orc_den",
        "plains",
        "temperate_forest",
        "tundra",
        "boreal_forest",
        "mountains",
        "forest",
        "boreal",
        "mountain",
    ];
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value.to_ascii_lowercase().as_str() {
            "goblin_den" => Self::GoblinDen,
            "orc_den" => Self::OrcDen,
            "plains" => Self::Plains,
            "temperate_forest" | "forest" => Self::TemperateForest,
            "tundra" => Self::Tundra,
            "boreal_forest" | "boreal" => Self::BorealForest,
            "mountains" | "mountain" => Self::Mountains,
            _ => return None,
        })
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::GoblinDen => "goblin_den",
            Self::OrcDen => "orc_den",
            Self::Plains => "plains",
            Self::TemperateForest => "temperate_forest",
            Self::Tundra => "tundra",
            Self::BorealForest => "boreal_forest",
            Self::Mountains => "mountains",
        }
    }

    fn biome_at(p: Vec2) -> Self {
        match biome::at(p) {
            biome::Biome::Plains => Self::Plains,
            biome::Biome::TemperateForest => Self::TemperateForest,
            biome::Biome::Tundra => Self::Tundra,
            biome::Biome::BorealForest => Self::BorealForest,
            biome::Biome::Mountains => Self::Mountains,
        }
    }

    pub(crate) fn find(self, origin: Vec3) -> Result<Vec3, String> {
        if !origin.is_finite() || origin.xz().abs().max_element() > 100_000_000.0 {
            return Err("Cannot locate from this position.".into());
        }
        let structure = matches!(self, Self::GoblinDen | Self::OrcDen);
        let radius = if structure { 32768.0 } else { 4096.0 };
        let found = match self {
            Self::GoblinDen => nearest(origin.xz(), goblin_dens::CELL, radius, |cell| {
                goblin_dens::locate_center(cell)
            }),
            Self::OrcDen => nearest(origin.xz(), orcs::CELL, radius, |cell| {
                orcs::site(cell).filter(|p| (terrain::height(p.xz()) - p.y).abs() < 0.01)
            }),
            _ if Self::biome_at(origin.xz()) == self => {
                Some(Vec3::new(origin.x, terrain::height(origin.xz()), origin.z))
            }
            _ => nearest(origin.xz(), 2.0, radius, |cell| {
                let p = cell.as_vec2() * 2.0;
                (Self::biome_at(p) == self).then(|| Vec3::new(p.x, terrain::height(p), p.y))
            }),
        };
        found.ok_or_else(|| {
            format!(
                "No {} found within {:.0} m. Try searching from another location.",
                self.name(),
                radius
            )
        })
    }

    pub(crate) fn locate(self, origin: Vec3) -> String {
        match self.find(origin) {
            Ok(p) => format!(
                "Nearest {}: X {:.2}, Y {:.2}, Z {:.2} ({:.0} m horizontally{}).",
                self.name(),
                p.x,
                p.y,
                p.z,
                origin.xz().distance(p.xz()),
                if matches!(self, Self::GoblinDen | Self::OrcDen) {
                    ""
                } else {
                    "; biome sampled every 2 m"
                },
            ),
            Err(message) => message,
        }
    }
}

/// Each candidate must lie within its cell (including its boundary). Expand
/// square rings until every unvisited cell is farther than the best candidate.
/// Unlike stopping at the first occupied ring, this proves horizontal proximity.
fn nearest(
    origin: Vec2,
    cell_size: f32,
    radius: f32,
    mut candidate: impl FnMut(IVec2) -> Option<Vec3>,
) -> Option<Vec3> {
    let center = (origin / cell_size).floor().as_ivec2();
    let mut best = None;
    let mut distance = radius;
    for ring in 0..=(radius / cell_size).ceil() as i32 + 1 {
        let mut visit = |offset: IVec2| {
            let cell = center + offset;
            let lo = cell.as_vec2() * cell_size;
            let hi = lo + Vec2::splat(cell_size);
            if origin.distance(origin.clamp(lo, hi)) > distance {
                return;
            }
            if let Some(p) = candidate(cell) {
                let d = origin.distance(p.xz());
                if d <= distance {
                    best = Some(p);
                    distance = d;
                }
            }
        };
        if ring == 0 {
            visit(IVec2::ZERO);
        } else {
            for x in -ring..=ring {
                visit(IVec2::new(x, -ring));
                visit(IVec2::new(x, ring));
            }
            for z in (-ring + 1)..ring {
                visit(IVec2::new(-ring, z));
                visit(IVec2::new(ring, z));
            }
        }
        let lo = (center - IVec2::splat(ring)).as_vec2() * cell_size;
        let hi = (center + IVec2::splat(ring + 1)).as_vec2() * cell_size;
        let unvisited_distance = (origin - lo).min(hi - origin).min_element();
        if unvisited_distance > distance {
            break;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_checks_beyond_first_occupied_ring_and_negative_cells() {
        for shift in [IVec2::ZERO, IVec2::new(-12, -9)] {
            let offset = shift.as_vec2() * 10.0;
            let origin = offset + Vec2::splat(9.0);
            let found = nearest(origin, 10.0, 100.0, |cell| {
                let p = match cell - shift {
                    IVec2::ZERO => offset,
                    IVec2::ONE => offset + Vec2::splat(10.0),
                    _ => return None,
                };
                Some(Vec3::new(p.x, 0.0, p.y))
            })
            .unwrap();
            assert_eq!(found.xz(), offset + Vec2::splat(10.0));
        }
        assert!(nearest(Vec2::ZERO, 10.0, 20.0, |_| None).is_none());
    }

    #[test]
    fn goblin_search_returns_a_real_generated_den() {
        let found = nearest(Vec2::ZERO, goblin_dens::CELL, 32768.0, |cell| {
            goblin_dens::locate_center(cell)
        })
        .expect("seeded world should contain a goblin den in search range");
        let cell = (found.xz() / goblin_dens::CELL).floor().as_ivec2();
        let den = goblin_dens::site(cell).unwrap();
        assert_eq!(den.center, found);
        assert!(terrain::height(found.xz()) > found.y + 40.0);
        assert!(
            Target::GoblinDen
                .locate(found)
                .contains("(0 m horizontally)")
        );
    }

    #[test]
    fn seeded_orc_result_matches_exhaustive_search() {
        let origin = Vec2::new(-312.0, 173.0);
        let found = nearest(origin, orcs::CELL, 2000.0, orcs::site).unwrap();
        let expected = (-10..=10)
            .flat_map(|x| (-10..=10).filter_map(move |z| orcs::site(IVec2::new(x, z))))
            .min_by(|a, b| {
                origin
                    .distance_squared(a.xz())
                    .total_cmp(&origin.distance_squared(b.xz()))
            })
            .unwrap();
        assert_eq!(found, expected);
        assert!((terrain::height(found.xz()) - found.y).abs() < 0.01);
        assert!(
            orcs::hole_uniforms(found, 48.0)
                .iter()
                .any(|hole| { hole.z > 0.0 && hole.xy().distance(found.xz()) < 0.01 })
        );
    }

    #[test]
    fn every_biome_is_locatable_and_current_biome_returns_current_position() {
        let origin = Vec3::new(-43.0, 90.0, 27.0);
        for target in [
            Target::Plains,
            Target::TemperateForest,
            Target::Tundra,
            Target::BorealForest,
            Target::Mountains,
        ] {
            let response = target.locate(origin);
            assert!(response.starts_with("Nearest"), "{response}");
            let p = nearest(origin.xz(), 2.0, 4096.0, |cell| {
                let p = cell.as_vec2() * 2.0;
                (Target::biome_at(p) == target).then_some(Vec3::new(p.x, 0.0, p.y))
            })
            .unwrap();
            assert_eq!(Target::biome_at(p.xz()), target);
        }
        let response = Target::biome_at(origin.xz()).locate(origin);
        assert!(response.contains("X -43.00"));
        assert!(response.contains("Z 27.00 (0 m"));
    }
}
