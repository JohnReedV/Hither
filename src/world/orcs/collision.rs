//! Conservative rock and shoring barriers built from the rendered vertices.
//! Final vertices include the Rootwarren warp; no second placement recipe.
use super::*;

pub(super) const BODY_HEIGHT: f32 = 1.65;

#[derive(Clone, Copy, Debug)]
pub(super) struct Barrier {
    pub min: Vec3,
    pub max: Vec3,
}
impl Barrier {
    fn overlaps(&self, at: Vec2, radius: f32) -> bool {
        at.distance_squared(at.clamp(self.min.xz(), self.max.xz())) < radius * radius
    }
    fn bank_fill(&self) -> Option<Barrier> {
        if self.min.z < -8. || self.max.z > 4.8 || self.min.x * self.max.x <= 0. {
            return None;
        }
        // Seal the below-ground pocket behind a bank rock. Otherwise a body
        // can land between the rock and the wall with no route back out.
        let mut bank = *self;
        bank.min.y = -8.;
        bank.max.y = self.max.y.min(0.);
        if self.max.x < 0. {
            bank.min.x = bank.min.x.min(-3.);
        } else {
            bank.max.x = bank.max.x.max(3.);
        }
        Some(bank)
    }
}

pub(super) struct Barriers {
    pub boxes: Vec<Barrier>,
    bins: HashMap<IVec2, Vec<usize>>,
}
impl Barriers {
    fn build(kind: usize) -> Self {
        let mut ranges = Vec::new();
        let (earth, _, _) = den_variant_geometry(kind, &mut ranges, &mut Vec::new());
        let boxes: Vec<_> = ranges
            .into_iter()
            .map(|range| {
                earth.positions[range].iter().fold(
                    Barrier {
                        min: Vec3::splat(f32::INFINITY),
                        max: Vec3::splat(f32::NEG_INFINITY),
                    },
                    |bounds, point| {
                        let point = Vec3::from_array(*point);
                        Barrier {
                            min: bounds.min.min(point),
                            max: bounds.max.max(point),
                        }
                    },
                )
            })
            .collect();
        let mut bins: HashMap<IVec2, Vec<usize>> = HashMap::new();
        for (index, bounds) in boxes.iter().enumerate() {
            let extent = bounds.bank_fill().unwrap_or(*bounds);
            let low = extent.min.xz().floor().as_ivec2();
            let high = extent.max.xz().floor().as_ivec2();
            for x in low.x..=high.x {
                for z in low.y..=high.y {
                    bins.entry(IVec2::new(x, z)).or_default().push(index);
                }
            }
        }
        Self { boxes, bins }
    }
    fn nearby(&self, at: Vec2, radius: f32) -> impl Iterator<Item = &Barrier> {
        let low = (at - Vec2::splat(radius)).floor().as_ivec2();
        let high = (at + Vec2::splat(radius)).floor().as_ivec2();
        (low.x..=high.x).flat_map(move |x| {
            (low.y..=high.y).flat_map(move |z| {
                self.bins
                    .get(&IVec2::new(x, z))
                    .into_iter()
                    .flatten()
                    .map(|index| &self.boxes[*index])
            })
        })
    }
    pub fn blocks(&self, feet: Vec3, radius: f32) -> bool {
        self.blocks_sized(feet, radius, BODY_HEIGHT)
    }
    pub fn blocks_sized(&self, feet: Vec3, radius: f32, height: f32) -> bool {
        self.nearby(feet.xz(), radius).any(|b| {
            let hits = |b: &Barrier| {
                b.overlaps(feet.xz(), radius)
                    && feet.y < b.max.y - 0.001
                    && feet.y + height > b.min.y + 0.001
            };
            hits(b) || b.bank_fill().is_some_and(|bank| hits(&bank))
        })
    }
    pub fn floors(&self, at: Vec2, radius: f32) -> impl Iterator<Item = f32> {
        self.nearby(at, radius)
            .filter(move |b| b.overlaps(at, radius))
            .map(|b| b.max.y + 0.002)
    }
    pub fn ceiling(&self, eye: Vec3, radius: f32) -> Option<f32> {
        self.nearby(eye.xz(), radius)
            .filter(move |b| b.overlaps(eye.xz(), radius) && b.min.y >= eye.y)
            .map(|b| b.min.y)
            .min_by(f32::total_cmp)
    }
    pub fn boundaries(&self, a: Vec2, b: Vec2, radius: f32) -> Vec<f32> {
        let low = (a.min(b) - Vec2::splat(radius)).floor().as_ivec2();
        let high = (a.max(b) + Vec2::splat(radius)).floor().as_ivec2();
        let mut indices = Vec::new();
        for x in low.x..=high.x {
            for z in low.y..=high.y {
                if let Some(bin) = self.bins.get(&IVec2::new(x, z)) {
                    indices.extend(bin.iter().copied());
                }
            }
        }
        indices.sort_unstable();
        indices.dedup();
        let d = b - a;
        let mut result = Vec::new();
        for index in indices {
            let bounds = &self.boxes[index];
            for axis in 0..2 {
                if d[axis].abs() > f32::EPSILON {
                    for edge in [
                        bounds.min.xz()[axis] - radius,
                        bounds.max.xz()[axis] + radius,
                    ] {
                        result.push((edge - a[axis]) / d[axis]);
                    }
                }
            }
            if d.length_squared() < f32::EPSILON {
                continue;
            }
            for x in [bounds.min.x, bounds.max.x] {
                for z in [bounds.min.z, bounds.max.z] {
                    let q = a - Vec2::new(x, z);
                    let aa = d.length_squared();
                    let bb = 2. * q.dot(d);
                    let cc = q.length_squared() - radius * radius;
                    let disc = bb * bb - 4. * aa * cc;
                    if disc >= 0. {
                        result.extend([
                            (-bb - disc.sqrt()) / (2. * aa),
                            (-bb + disc.sqrt()) / (2. * aa),
                        ]);
                    }
                }
            }
        }
        result
    }
}
pub(super) fn barriers(kind: usize) -> &'static Barriers {
    static BARRIERS: OnceLock<[Barriers; 3]> = OnceLock::new();
    &BARRIERS.get_or_init(|| std::array::from_fn(Barriers::build))[kind]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::navigation::Geometry as _;

    #[test]
    fn authored_rocks_block_both_players_and_orcs() {
        for kind in 0..3 {
            let home = (100..500)
                .find_map(|x| {
                    let cell = IVec2::new(x, 161);
                    site(cell).filter(|_| den_kind(cell) == kind)
                })
                .unwrap();
            let obstacles = barriers(kind);
            assert!(obstacles.boxes.len() >= 110);
            for b in &obstacles.boxes {
                let feet = home + (b.min + b.max) * 0.5 - Vec3::Y * 0.2;
                assert!(
                    crate::player::movement::character_collides(
                        feet + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
                        crate::player::movement::PLAYER_RADIUS
                    ),
                    "player passed through den {kind} rock {b:?}"
                );
                assert!(
                    !navigation::World.body_clear(feet, crate::world::navigation::Agent::default()),
                    "navigation passed through den {kind} rock {b:?}"
                );
            }
        }
    }

    #[test]
    fn scripted_entries_exits_and_stone_lift_clear_authored_rocks() {
        for kind in 0..3 {
            for frame in 0..=750 {
                let feet = emergence_position(frame as f32 / 100.);
                assert!(
                    !barriers(kind).blocks(feet, crate::player::movement::PLAYER_RADIUS),
                    "script crosses rock: kind={kind} feet={feet:?}"
                );
                if kind == 1 {
                    let feet = stone_carrier(frame as f32 / 750.);
                    assert!(
                        !barriers(kind).blocks(feet, crate::player::movement::PLAYER_RADIUS),
                        "stone lifter crosses rock: feet={feet:?}"
                    );
                }
            }
        }
    }
}
