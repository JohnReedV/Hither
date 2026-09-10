//! Bounded, ground-contact footprints; 30 seconds of simulation time.
use bevy::prelude::*;
use std::collections::VecDeque;

// Covers continuous walking plus repeated short-jump landings for 30 seconds.
pub(crate) const MAX_TRACKS: usize = 512;
const LIFETIME: f32 = 30.0;
const STEP: f32 = 0.85;

// One-metre spatial hash. Each print occurs in one chain; hash collisions are
// harmless because the shader still checks the exact footprint radius.
#[derive(bevy::render::render_resource::ShaderType, Clone, Copy, Debug, PartialEq)]
pub(crate) struct TrackBins {
    pub heads: [UVec4; 256],
    pub next: [UVec4; MAX_TRACKS / 4],
}
impl Default for TrackBins {
    fn default() -> Self {
        Self {
            heads: [UVec4::splat(u32::MAX); 256],
            next: [UVec4::splat(u32::MAX); MAX_TRACKS / 4],
        }
    }
}
fn track_bucket(cell: IVec2) -> usize {
    (((cell.x as u32).wrapping_mul(0x9e3779b9) ^ (cell.y as u32).wrapping_mul(0x85ebca6b)) & 1023)
        as usize
}
impl TrackBins {
    pub(crate) fn build(prints: &[Vec4]) -> Self {
        let mut bins = Self::default();
        for (i, print) in prints.iter().enumerate() {
            let bucket = track_bucket(print.xy().floor().as_ivec2());
            bins.next[i / 4][i % 4] = bins.heads[bucket / 4][bucket % 4];
            bins.heads[bucket / 4][bucket % 4] = i as u32;
        }
        bins
    }
}

#[derive(Resource, Default)]
pub(crate) struct Tracks {
    pub(crate) revision: u64,
    prints: VecDeque<Vec4>, // x, z, facing yaw, age
    previous: Option<Vec2>,
    distance: f32,
    left: bool,
    airborne: bool,
}

impl Tracks {
    fn stamp(&mut self, p: Vec2, yaw: f32, left: bool) {
        self.stamp_with_stance(p, yaw, left, 0.105);
    }

    pub(crate) fn stamp_turn(&mut self, p: Vec2, yaw: f32, left: bool) {
        // Same hip-width stance as the turn clips, stamped only at foot contact.
        self.stamp_with_stance(p, yaw, left, 0.123);
    }

    fn stamp_with_stance(&mut self, p: Vec2, yaw: f32, left: bool, stance: f32) {
        self.stamp_kind(p, yaw, left, stance, false);
    }

    pub(crate) fn stamp_orc(&mut self, p: Vec2, yaw: f32, left: bool) {
        self.stamp_kind(p, yaw, left, 0.15, true);
    }

    fn stamp_kind(&mut self, p: Vec2, yaw: f32, left: bool, stance: f32, orc: bool) {
        let side = Vec2::new(yaw.cos(), yaw.sin()) * if left { -stance } else { stance };
        let p = p + side;
        if crate::world::biome::surface_snow(p) < 0.1 {
            return;
        }
        if self.prints.len() == MAX_TRACKS {
            self.prints.pop_front();
        }
        // Store handedness in an extra full revolution, leaving sin/cos intact.
        let facing = (yaw + if left { -0.065 } else { 0.065 }).rem_euclid(std::f32::consts::TAU)
            + if left { std::f32::consts::TAU } else { 0.0 }
            + if orc {
                2.0 * std::f32::consts::TAU
            } else {
                0.0
            };
        self.prints.push_back(Vec4::new(p.x, p.y, facing, 0.0));
        self.revision = self.revision.wrapping_add(1);
    }

    fn advance(&mut self, p: Vec2, yaw: f32, grounded: bool, dt: f32) {
        for print in &mut self.prints {
            print.w += dt;
        }
        let count = self.prints.len();
        self.prints.retain(|print| print.w < LIFETIME);
        if self.prints.len() != count {
            self.revision = self.revision.wrapping_add(1);
        }
        let previous = self.previous.replace(p).unwrap_or(p);
        let traveled = previous.distance(p);
        if !grounded || traveled > 2.0 {
            self.distance = 0.0;
            self.airborne = !grounded;
            return;
        }
        if self.airborne {
            self.stamp(p, yaw, true);
            self.stamp(p, yaw, false);
            self.airborne = false;
            self.distance = 0.0;
        } else if traveled > 0.0001 {
            let mut next = STEP - self.distance;
            while next <= traveled {
                self.left = !self.left;
                self.stamp(previous.lerp(p, next / traveled), yaw, self.left);
                next += STEP;
            }
            self.distance = (self.distance + traveled) % STEP;
        }
    }

    pub(crate) fn uniforms(&self) -> ([Vec4; MAX_TRACKS], Vec4, Vec4) {
        let mut prints = [Vec4::ZERO; MAX_TRACKS];
        let mut lo = Vec2::splat(f32::MAX);
        let mut hi = Vec2::splat(f32::MIN);
        for (i, print) in self.prints.iter().enumerate() {
            prints[i] = *print;
            lo = lo.min(print.xy() - Vec2::splat(0.3));
            hi = hi.max(print.xy() + Vec2::splat(0.3));
        }
        (
            prints,
            Vec4::new(self.prints.len() as f32, 0.0, 0.0, 0.0),
            Vec4::new(lo.x, lo.y, hi.x, hi.y),
        )
    }
}

pub(crate) fn update(
    time: Res<Time>,
    game: Res<crate::app::GameState>,
    rig: Res<crate::player::camera::CameraRig>,
    mut tracks: ResMut<Tracks>,
    mut previewed: Local<bool>,
) {
    if !*previewed {
        *previewed = true;
        if std::env::var_os("HITHER_TRACKS_PREVIEW").is_some() {
            let p = rig.position.xz();
            for i in 0..6 {
                let point = p + Vec2::new(-0.45, -1.0 - i as f32 * 0.6);
                tracks.stamp(point, 0., i % 2 == 0);
                tracks.stamp_orc(point + Vec2::X * 0.9, 0., i % 2 == 0);
            }
        }
    }
    if !game.paused {
        tracks.advance(
            rig.position.xz(),
            rig.yaw,
            rig.grounded && !rig.is_spectating(),
            time.delta_secs(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spatial_bins_match_full_scan_at_boundaries_and_hash_collisions() {
        let prints: Vec<_> = (0..MAX_TRACKS)
            .map(|i| {
                Vec4::new(
                    (i % 23) as f32 * 0.37 - 4.0,
                    (i / 23) as f32 * 0.41 - 4.0,
                    0.,
                    0.,
                )
            })
            .collect();
        let bins = TrackBins::build(&prints);
        for x in -51..51 {
            for y in -51..51 {
                let p = Vec2::new(x as f32 * 0.1, y as f32 * 0.1);
                let mut found = std::collections::BTreeSet::new();
                let lo = (p - Vec2::splat(0.3)).floor().as_ivec2();
                let hi = (p + Vec2::splat(0.3)).floor().as_ivec2();
                for by in lo.y..=hi.y {
                    for bx in lo.x..=hi.x {
                        let bucket = track_bucket(IVec2::new(bx, by));
                        let mut link = bins.heads[bucket / 4][bucket % 4];
                        let mut visited = 0;
                        while link != u32::MAX {
                            assert!(visited < MAX_TRACKS);
                            visited += 1;
                            let i = link as usize;
                            if prints[i].xy().distance_squared(p) <= 0.08 {
                                found.insert(i);
                            }
                            link = bins.next[i / 4][i % 4];
                        }
                    }
                }
                let reference = prints
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.xy().distance_squared(p) <= 0.08)
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(found, reference);
            }
        }
    }
    #[test]
    fn orc_prints_encode_species_and_handedness_and_expire() {
        let p = (-100..100)
            .map(|x| Vec2::new(x as f32 * 10., 0.))
            .find(|p| crate::world::biome::snow_amount(*p) == 1.)
            .unwrap();
        let mut tracks = Tracks::default();
        tracks.stamp_orc(p, 0.2, false);
        tracks.stamp_orc(p, 0.2, true);
        assert_eq!(tracks.prints.len(), 2);
        let tau = std::f32::consts::TAU;
        assert!((2. * tau..3. * tau).contains(&tracks.prints[0].z));
        assert!((3. * tau..4. * tau).contains(&tracks.prints[1].z));
        assert!((tracks.prints[0].xy().distance(tracks.prints[1].xy()) - 0.3).abs() < 0.001);
        tracks.advance(p, 0., false, 30.);
        assert!(tracks.prints.is_empty());
    }
    #[test]
    fn spectators_leave_no_tracks_but_existing_tracks_still_expire() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.0));
        let mut rig = crate::player::camera::CameraRig::default();
        rig.toggle_spectator();
        let mut tracks = Tracks::default();
        tracks.prints.push_back(Vec4::ZERO);
        app.insert_resource(time)
            .insert_resource(rig)
            .insert_resource(tracks)
            .init_resource::<crate::app::GameState>()
            .add_systems(Update, update);
        for _ in 0..29 {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position
                .x += 0.5;
            app.update();
        }
        assert_eq!(app.world().resource::<Tracks>().prints.len(), 1);
        app.update();
        assert!(app.world().resource::<Tracks>().prints.is_empty());
    }
    #[test]
    fn tracks_are_grounded_stationary_safe_bounded_and_expire() {
        // Seed 721 starts in winter; choose a snowy point without assuming origin.
        let origin = (-100..100)
            .map(|x| Vec2::new(x as f32 * 10.0, 0.0))
            .find(|p| crate::world::biome::snow_amount(*p) == 1.0)
            .unwrap();
        let mut t = Tracks::default();
        t.advance(origin, 0.0, true, 0.0);
        t.advance(origin + Vec2::Y, 0.0, true, 0.2);
        assert_eq!(t.prints.len(), 1);
        let mark = t.prints[0];
        t.advance(origin + Vec2::Y, 2.0, true, 1.0);
        assert_eq!(t.prints.len(), 1); // turning/standing doesn't draw trails
        assert_eq!(t.prints[0].xyz(), mark.xyz());
        t.advance(origin, 0.0, false, 0.2);
        assert_eq!(t.prints.len(), 1);
        t.advance(origin, 0.0, true, 0.2);
        assert_eq!(t.prints.len(), 3); // landing, both feet
        for _ in 0..MAX_TRACKS * 2 {
            t.stamp(origin, 0.0, true);
        }
        assert_eq!(t.prints.len(), MAX_TRACKS);
        t.advance(origin, 0.0, true, 29.9);
        assert_eq!(t.prints.len(), MAX_TRACKS);
        t.advance(origin, 0.0, true, 0.11);
        assert!(t.prints.is_empty());
        assert_eq!(t.uniforms().1.x, 0.0);
    }
}
