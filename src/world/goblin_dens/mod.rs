//! Seeded mountain settlements. Layout, rendered stone and traversal share one
//! continuous excavation; no prefab layouts or finite den-variant catalogue.
use super::{biome, random::Rng, terrain};
use crate::rendering::geometry::{Geometry, RayMesh};
use crate::world::streaming::{Coordinator, Layer};
use bevy::prelude::*;
use std::f32::consts::TAU;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};
mod art;
mod excavation;
mod geology;
mod layout;
mod lore;
mod routes;

pub(crate) const CELL: f32 = super::orchard::oak::CELL;
pub(crate) const MAX_PORTAL_SEGMENTS: usize = 128;
const REACH: f32 = 68.;
#[derive(Clone, Debug)]
pub(super) struct House {
    pub at: Vec3,
    pub yaw: f32,
    pub width: f32,
    pub depth: f32,
    pub rise: f32,
    pub roof: Vec4,
    pub edges: Vec4,
    pub window: Vec2,
}
impl House {
    fn porch(&self) -> Vec3 {
        self.at + Quat::from_rotation_y(self.yaw) * Vec3::Z * (self.depth * 0.5 + 0.35)
    }
    fn port(&self) -> Vec3 {
        self.at + Quat::from_rotation_y(self.yaw) * Vec3::Z * (self.depth * 0.5 + 2.0)
    }
    fn roof_height(&self, x: f32, z: f32) -> f32 {
        let run = if x < self.roof.z {
            self.width * 0.5 + self.roof.z
        } else {
            self.width * 0.5 - self.roof.z
        };
        self.roof.x - (self.roof.x - self.roof.y) * (x - self.roof.z).abs() / run + self.roof.w * z
    }
}
#[derive(Clone, Debug)]
pub(super) struct Landing {
    pub at: Vec3,
    pub size: Vec2,
    pub yaw: f32,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Span {
    pub a: Vec3,
    pub b: Vec3,
    pub width: f32,
    pub sag: f32,
}
impl Span {
    fn point(self, t: f32) -> Vec3 {
        let length = self.a.distance(self.b);
        if length <= 3.1 {
            return self.a.lerp(self.b, t);
        }
        let apron = 1.55 / length;
        let u = ((t - apron) / (1. - 2. * apron)).clamp(0., 1.);
        let sag = self.sag * (length - 3.1) / length;
        self.a.lerp(self.b, t) - Vec3::Y * (4. * sag * u * (1. - u))
    }
}
pub(crate) struct Den {
    pub cell: IVec2,
    pub center: Vec3,
    seed: u64,
    radii: Vec3,
    houses: Vec<House>,
    shrine: House,
    food_hall: House,
    landings: Vec<Landing>,
    spans: Vec<Span>,
    tunnels: Vec<Vec<Vec3>>,
    built: Mutex<Option<Arc<Built>>>,
    exposed: OnceLock<Vec<(Vec3, Vec3)>>,
    air_segments: OnceLock<AirSegments>,
}
// The excavation field only considers segments within sqrt(15) metres.
// Index that exact support, including cell boundaries, without approximating
// either the field or collision geometry.
struct AirSegments {
    cells: HashMap<IVec3, Vec<(Vec3, Vec3)>>,
}
impl AirSegments {
    fn new(paths: &[Vec<Vec3>]) -> Self {
        let mut cells: HashMap<IVec3, Vec<(Vec3, Vec3)>> = HashMap::new();
        for path in paths {
            for w in path.windows(2) {
                let radius = Vec3::splat(15_f32.sqrt() + 0.001);
                let lo = ((w[0].min(w[1]) - radius) / 8.).floor().as_ivec3();
                let hi = ((w[0].max(w[1]) + radius) / 8.).floor().as_ivec3();
                for x in lo.x..=hi.x {
                    for y in lo.y..=hi.y {
                        for z in lo.z..=hi.z {
                            cells
                                .entry(IVec3::new(x, y, z))
                                .or_default()
                                .push((w[0], w[1]));
                        }
                    }
                }
            }
        }
        Self { cells }
    }
    fn at(&self, p: Vec3) -> &[(Vec3, Vec3)] {
        self.cells
            .get(&(p / 8.).floor().as_ivec3())
            .map_or(&[], Vec::as_slice)
    }
}
pub(super) struct Built {
    meshes: Vec<Geometry>,
    collision: RayMesh,
    floors: RayMesh,
    lamps: Vec<Vec3>,
    relics: Vec<lore::Relic>,
}
fn hash(cell: IVec2, seed: u64) -> u64 {
    let mut h = seed
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b)
        ^ 0x676f626c696e6465;
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}
fn generate(cell: IVec2, seed: u64) -> Option<Den> {
    // Exactly the oak's regional density, independently salted. Suitability
    // reduces realized density, just as the oak's plains-only habitat does.
    if !super::orchard::oak::region_enabled(cell, seed ^ 0x676f626c696e) {
        return None;
    }
    let mut rng = Rng(hash(cell, seed));
    let origin = cell.as_vec2() * CELL;
    let raw = |p| biome::terrain_for_seed(p, seed);
    let ground = |p: Vec2| {
        let a = p.floor();
        let f = p - a;
        let b = raw(a + Vec2::X);
        let c = raw(a + Vec2::Y);
        if f.x + f.y <= 1. {
            raw(a) * (1. - f.x - f.y) + b * f.x + c * f.y
        } else {
            b * (1. - f.y) + c * (1. - f.x) + raw(a + Vec2::ONE) * (f.x + f.y - 1.)
        }
    };
    let mut summit = None;
    for x in 0..11 {
        for z in 0..11 {
            let p = origin + Vec2::splat(76.) + Vec2::new(x as f32, z as f32) * 13.6;
            let y = raw(p);
            if biome::mountain_for_seed(p, seed) >= 0.85
                && y >= 175.
                && summit.is_none_or(|(_, h)| y > h)
            {
                summit = Some((p, y));
            }
        }
    }
    let (mut p, _) = summit?;
    // Refine the crest inside the region's safe inset, not a random flank.
    for step in [6., 3., 1.5] {
        let start = p;
        for x in -1..=1 {
            for z in -1..=1 {
                let q = start + Vec2::new(x as f32, z as f32) * step;
                let local = q - origin;
                if local.min_element() > 70. && local.max_element() < CELL - 70. && raw(q) > raw(p)
                {
                    p = q;
                }
            }
        }
    }
    // A cell boundary is not a summit. Reject a clipped uphill search so
    // settlements stay under real crests rather than high mountain flanks.
    for i in 0..16 {
        let angle = i as f32 * TAU / 16.;
        if raw(p + Vec2::new(angle.cos(), angle.sin()) * 8.) > raw(p) + 1. {
            return None;
        }
    }
    let radii = Vec3::new(
        rng.range(23., 29.),
        rng.range(18., 23.),
        rng.range(22., 28.),
    );
    let center = Vec3::new(p.x, raw(p) - rng.range(62., 75.), p.y);
    // A complete cover survey prevents a chamber breaking through a flank.
    for x in -5..=5 {
        for z in -5..=5 {
            let q = p + Vec2::new(x as f32, z as f32) * 6.;
            if raw(q) < center.y + radii.y + 16. || biome::mountain_for_seed(q, seed) < 0.75 {
                return None;
            }
        }
    }
    let count = 2 + (rng.unit() * 4.).floor().min(3.) as usize;
    let phase = rng.range(0., TAU);
    let layout::Plan {
        houses,
        shrine,
        food_hall,
        landings,
        spans,
    } = layout::plan(count, radii, hash(cell, seed), &mut rng)?;
    // Validate the same candidate pool regardless of the eventual count roll.
    // Otherwise rejecting difficult three-route sites biases the spawn weights.
    let tunnel_count = 3;
    let mut tunnels: Vec<Vec<Vec3>> = Vec::new();
    for i in 0..tunnel_count {
        let mut selected = None;
        for attempt in 0..96 {
            let angle =
                phase + TAU * ((i % count) as f32 + 0.5) / count as f32 + attempt as f32 * 0.2618;
            // Depart from the outward-most landing, so a tunnel cannot cut
            // across the interior bridge network on its way to the rock wall.
            let outward = Vec2::new(angle.cos(), angle.sin());
            let hub = landings
                .iter()
                .max_by(|a, b| a.at.xz().dot(outward).total_cmp(&b.at.xz().dot(outward)))
                .unwrap()
                .at;
            let twist = rng.range(3.6, 4.5);
            let exit = Vec2::new((angle + twist).cos(), (angle + twist).sin()) * 56.;
            let end_y = ground(p + exit) - center.y + 1.5;
            let path = routes::excavate(hub + Vec3::Y * 1.45, angle, twist, end_y, &mut rng);
            let covered = path.iter().enumerate().all(|(j, v)| {
                j > 94
                    || (0..8).all(|k| {
                        let angle = k as f32 * TAU / 8.;
                        let offset = Vec2::new(angle.cos(), angle.sin()) * 2.65;
                        raw(p + v.xz() + offset) > center.y + v.y + 2.7
                    })
            });
            let walkable = path
                .windows(2)
                .all(|w| (w[1].y - w[0].y).abs() / w[1].xz().distance(w[0].xz()) < 0.65);
            // A passage must leave the hub through open cavern, never through
            // somebody's bed or back wall. Check its entire approach corridor.
            let avoids_houses = (0..=80).all(|j| {
                let p = path[0].lerp(path[1], j as f32 / 80.) - Vec3::Y * 1.45;
                houses.iter().chain([&shrine, &food_hall]).all(|h| {
                    let q = Quat::from_rotation_y(-h.yaw) * (p - h.at);
                    q.x.abs() > h.width * 0.5 + 1.5 || q.z.abs() > h.depth * 0.5 + 1.5
                })
            });
            let open: Vec<_> = path
                .iter()
                .map(|v| ground(p + v.xz()) < center.y + v.y + 2.25)
                .collect();
            let one_mouth = !open.windows(2).any(|w| w[0] && !w[1]);
            // Separate winding passages outside the chamber. Otherwise one
            // tunnel's crossbeam can intersect another tunnel's walking space.
            let separated = tunnels.iter().all(|existing| {
                path.iter().skip(2).all(|p| {
                    existing.windows(2).skip(1).all(|w| {
                        let edge = w[1] - w[0];
                        let t = ((*p - w[0]).dot(edge) / edge.length_squared()).clamp(0., 1.);
                        p.distance(w[0] + edge * t) > 5.4
                    })
                })
            });
            if covered && walkable && avoids_houses && one_mouth && separated {
                selected = Some(path);
                break;
            }
        }
        tunnels.push(selected?);
    }
    tunnels.truncate(routes::count(hash(cell, seed ^ 0x726f757465636f75)));
    Some(Den {
        cell,
        center,
        seed: hash(cell, seed),
        radii,
        houses,
        shrine,
        food_hall,
        landings,
        spans,
        tunnels,
        built: Mutex::new(None),
        exposed: OnceLock::new(),
        air_segments: OnceLock::new(),
    })
}
// Both streaming and location queries accept only complete layouts that fit
// the current world's collision/rendered terrain, including its clearings.
fn world_site(cell: IVec2) -> Option<Den> {
    generate(cell, biome::world_seed()).filter(Den::fits_world_terrain)
}

#[derive(Default)]
struct Cache {
    entries: HashMap<IVec2, Option<Arc<Den>>>,
    order: VecDeque<IVec2>,
}
// Locator queries must not evict streamed dens or hold their cache lock while
// searching distant cells. Use the same generation rules without building art.
pub(crate) fn locate_center(cell: IVec2) -> Option<Vec3> {
    world_site(cell).map(|den| den.center)
}
pub(crate) fn site(cell: IVec2) -> Option<Arc<Den>> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some(den) = cache.entries.get(&cell) {
        return den.clone();
    }
    let den = world_site(cell).map(Arc::new);
    if cache.entries.len() >= 128 {
        let key = cache.order.pop_front().unwrap();
        cache.entries.remove(&key);
    }
    cache.order.push_back(cell);
    cache.entries.insert(cell, den.clone());
    den
}
impl Den {
    fn fits_world_terrain(&self) -> bool {
        self.center.is_finite()
            && biome::at(self.center.xz()) == biome::Biome::Mountains
            && terrain::height(self.center.xz()) >= self.center.y + self.radii.y + 16.0
    }

    fn built(&self) -> Arc<Built> {
        let mut built = self.built.lock().unwrap();
        built
            .get_or_insert_with(|| Arc::new(art::build(self)))
            .clone()
    }
    pub(crate) fn ready(&self) -> bool {
        self.built.try_lock().is_ok_and(|built| built.is_some())
    }
    /// Negative in excavated air. Irregular rock chamber unioned with tunnels.
    fn air(&self, p: Vec3) -> f32 {
        let q = p / self.radii;
        let smooth = ((q.length() - 1.) * self.radii.min_element()).max(-17. - p.y);
        if smooth < -5. {
            return smooth;
        }
        let mut d = ((q.length() - 1.) * self.radii.min_element() + geology::relief(p, self.seed))
            .max(-17. - p.y);
        // Deep chamber points are already air; tunnels cannot change their
        // sign or any nearby isosurface. This is the hot path for resident AI.
        if d < -3. {
            return d;
        }
        let mut profile = None;
        for &(a, b) in self
            .air_segments
            .get_or_init(|| AirSegments::new(&self.tunnels))
            .at(p)
        {
            let edge = b - a;
            let t = ((p - a).dot(edge) / edge.length_squared()).clamp(0., 1.);
            let offset = p - (a + edge * t);
            if offset.length_squared() > 15. {
                continue;
            }
            let profile =
                *profile.get_or_insert_with(|| geology::burrow_profile(p + self.center, self.seed));
            d = d.min(geology::burrow_field(
                offset,
                edge.with_y(0.).normalize(),
                profile,
            ));
        }
        d
    }
    #[cfg(test)]
    fn air_reference(&self, p: Vec3) -> f32 {
        let q = p / self.radii;
        let smooth = ((q.length() - 1.) * self.radii.min_element()).max(-17. - p.y);
        if smooth < -5. {
            return smooth;
        }
        let mut d = ((q.length() - 1.) * self.radii.min_element() + geology::relief(p, self.seed))
            .max(-17. - p.y);
        // Deep chamber points are already air; tunnels cannot change their
        // sign or any nearby isosurface. This is the hot path for resident AI.
        if d < -3. {
            return d;
        }
        let mut profile = None;
        for path in &self.tunnels {
            for w in path.windows(2) {
                let edge = w[1] - w[0];
                let t = ((p - w[0]).dot(edge) / edge.length_squared()).clamp(0., 1.);
                let offset = p - (w[0] + edge * t);
                if offset.length_squared() > 15. {
                    continue;
                }
                let profile = *profile
                    .get_or_insert_with(|| geology::burrow_profile(p + self.center, self.seed));
                d = d.min(geology::burrow_field(
                    offset,
                    edge.with_y(0.).normalize(),
                    profile,
                ));
            }
        }
        d
    }
    pub(crate) fn resident_count(&self) -> usize {
        self.houses.len() * 4
    }

    pub(crate) fn residents(&self) -> Vec<Vec3> {
        self.houses
            .iter()
            .flat_map(|h| {
                let rotation = Quat::from_rotation_y(h.yaw);
                [
                    Vec3::new(-0.65, 0.02, -0.3),
                    Vec3::new(0.65, 0.02, -0.3),
                    Vec3::new(-0.65, 0.02, 0.65),
                    Vec3::new(0.65, 0.02, 0.65),
                ]
                .map(|v| self.center + h.at + rotation * v)
            })
            .collect()
    }
}
fn near(p: Vec2) -> Option<Arc<Den>> {
    // Locomotion probes the same cell many times per step. Avoid taking the
    // shared placement-cache lock for every body sphere and support sample.
    thread_local! {
        static LAST: std::cell::RefCell<(IVec2, Option<Arc<Den>>)> = const {
            std::cell::RefCell::new((IVec2::MAX, None))
        };
    }
    let cell = (p / CELL).floor().as_ivec2();
    LAST.with_borrow_mut(|cached| {
        if cached.0 != cell {
            *cached = (cell, site(cell));
        }
        cached
            .1
            .as_ref()
            .filter(|d| p.distance(d.center.xz()) < REACH)
            .cloned()
    })
}
pub(crate) fn shader_seed(camera: Vec3) -> UVec4 {
    let mut seed = biome::shader_seed();
    seed.y = u32::from(near(camera.xz()).is_some_and(|d| {
        camera.y < terrain::height(camera.xz()) - 3. && d.air(camera - d.center) < 0.
    }));
    seed
}
pub(crate) fn surface_open(p: Vec2) -> bool {
    near(p).is_some_and(|d| d.air(Vec3::new(p.x, terrain::height(p), p.y) - d.center) < 0.)
}
impl Den {
    fn floor_levels(&self, p: Vec2, radius: f32) -> Vec<f32> {
        type Slot = Option<((u64, u32, u32, u32), Vec<f32>)>;
        thread_local! { static FLOORS: std::cell::RefCell<Vec<Slot>> = std::cell::RefCell::new(vec![None;16384]); }
        let key = (self.seed, p.x.to_bits(), p.y.to_bits(), radius.to_bits());
        let h = key.1.wrapping_mul(0x9e3779b9) ^ key.2.rotate_left(16) ^ key.3 ^ (key.0 as u32);
        let index = ((h ^ (h >> 16)) as usize) & 16383;
        if let Some(result) = FLOORS.with_borrow(|cache| {
            cache[index]
                .as_ref()
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return result;
        }
        let built = self.built();
        let mut result = Vec::new();
        // Support the actual footprint at landings, not just its center. This
        // keeps a descending capsule on the platform until it clears the lip.
        for i in 0..9 {
            let angle = (i as f32 - 1.) * TAU / 8.;
            let offset = if i == 0 {
                Vec2::ZERO
            } else {
                Vec2::new(angle.cos(), angle.sin()) * radius
            };
            let q = p + offset;
            let local = q - self.center.xz();
            let y = terrain::height(q) - self.center.y + 3.;
            result.extend(
                built
                    .floors
                    .downward_hits(Vec3::new(local.x, y, local.y), -30.)
                    .into_iter()
                    .map(|y| y + self.center.y),
            );
        }
        result.sort_by(f32::total_cmp);
        let mut floors: Vec<f32> = Vec::new();
        for y in result {
            if let Some(last) = floors.last_mut()
                && y - *last < 0.20
            {
                *last = y;
            } else {
                floors.push(y);
            }
        }
        FLOORS.with_borrow_mut(|cache| cache[index] = Some((key, floors.clone())));
        floors
    }
}
pub(crate) fn levels(p: Vec2, radius: f32) -> Option<Vec<f32>> {
    Some(near(p)?.floor_levels(p, radius))
}
pub(crate) fn support(p: Vec3, radius: f32) -> Option<f32> {
    let den = near(p.xz())?;
    // Surface walkers must not snap down to a covered tunnel. At an actual
    // mouth the excavation intersects the surface and support transfers.
    if p.y > terrain::height(p.xz()) + 3. {
        return None;
    }
    if p.y >= terrain::height(p.xz()) - 0.05 && den.air(p - den.center) > 0. {
        return None;
    }
    levels(p.xz(), radius)?
        .into_iter()
        .filter(|y| *y <= p.y - crate::player::movement::PLAYER_EYE_HEIGHT + 0.21)
        .max_by(f32::total_cmp)
}
pub(crate) fn body_clear(feet: Vec3, radius: f32, height: f32) -> Option<bool> {
    let den = near(feet.xz())?;
    if feet.y >= terrain::height(feet.xz()) && den.air(feet - den.center) > 0. {
        return None;
    }
    let local = feet - den.center;
    let built = den.built();
    let surface = terrain::height(feet.xz());
    let steps = ((height - 2. * radius) / 0.1).ceil().max(1.) as usize;
    Some((0..=steps).all(|i| {
        let y = (radius + 0.025).lerp(
            (height - radius).max(radius + 0.025),
            i as f32 / steps as f32,
        );
        let p = local + Vec3::Y * y;
        (den.air(p) < 0. || p.y + den.center.y > surface)
            && !built.collision.overlaps_sphere(p, radius)
    }))
}
pub(crate) fn ceiling(p: Vec3) -> Option<f32> {
    let den = near(p.xz())?;
    if p.y >= terrain::height(p.xz()) {
        return None;
    }
    let start = p - den.center + Vec3::Y * 0.04;
    den.built()
        .collision
        .hit(start, Vec3::Y, 100.)
        .map(|t| p.y + 0.04 + t)
}
// Retain only the current spatial window, not every den ever visited. Exposure
// belongs to the immutable seeded den and is computed once, outside cache locks.
#[derive(Default)]
struct PortalCache {
    key: Option<(IVec2, i32, u64)>,
    segments: Vec<(Vec3, Vec3, f32)>,
    limit_volumes: Vec<(Vec4, Vec4)>,
    limits: Vec<Vec4>,
    view: Option<(Vec3, f32)>,
    result: Vec<Vec4>,
}
thread_local! { static PORTALS: std::cell::RefCell<PortalCache> = std::cell::RefCell::default(); }
pub(crate) fn portal_uniforms(camera: Vec3, distance: f32) -> [Vec4; MAX_PORTAL_SEGMENTS * 2] {
    PORTALS.with_borrow_mut(|cache| {
        let c = (camera.xz() / CELL).floor().as_ivec2();
        let r = ((distance + REACH) / CELL).ceil() as i32;
        let key = (c, r, biome::world_seed());
        if cache.key != Some(key) {
            cache.key = Some(key);
            cache.view = None;
            cache.segments.clear();
            cache.limit_volumes.clear();
            for x in -r..=r {
                for z in -r..=r {
                    if let Some(den) = site(c + IVec2::new(x, z)) {
                        cache.limit_volumes.push((
                            den.center.extend(-1.0),
                            (den.radii + Vec3::splat(3.0)).extend(0.0),
                        ));
                        for path in &den.tunnels {
                            cache.limit_volumes.extend(path.windows(2).map(|w| {
                                (
                                    (den.center + w[0]).extend(4.0),
                                    (den.center + w[1]).extend(0.0),
                                )
                            }));
                        }
                        let exposed = den.exposed.get_or_init(|| {
                            den.tunnels
                                .iter()
                                .flat_map(|path| path.windows(2))
                                .filter_map(|w| {
                                    let a = den.center + w[0];
                                    let b = den.center + w[1];
                                    (![a, a.lerp(b, 0.5), b]
                                        .iter()
                                        .all(|p| p.y + 2.5 < terrain::height(p.xz())))
                                    .then_some((a, b))
                                })
                                .collect()
                        });
                        cache.segments.extend(
                            exposed
                                .iter()
                                .map(|&(a, b)| (a, b, (den.seed % 4096) as f32)),
                        );
                    }
                }
            }
        }
        if cache.view != Some((camera, distance)) {
            cache.view = Some((camera, distance));
            cache.result.clear();
            cache.limits.clear();
            for &(a, b) in &cache.limit_volumes {
                let (center, radius) = if a.w < 0.0 {
                    (a.xyz(), b.xyz().max_element())
                } else {
                    (
                        (a.xyz() + b.xyz()) * 0.5,
                        a.w + a.xyz().distance(b.xyz()) * 0.5,
                    )
                };
                if (center.distance(camera) - distance).abs() <= radius && cache.limits.len() < 128
                {
                    cache.limits.extend([a, b]);
                }
            }
            for &(a, b, seed) in &cache.segments {
                if a.distance_squared(camera) > (distance + 8.).powi(2)
                    && b.distance_squared(camera) > (distance + 8.).powi(2)
                {
                    continue;
                }
                if cache.result.len() == MAX_PORTAL_SEGMENTS * 2 {
                    break;
                }
                cache.result.extend([a.extend(3.5), b.extend(seed)]);
            }
        }
        let mut result = [Vec4::ZERO; MAX_PORTAL_SEGMENTS * 2];
        result[..cache.result.len()].copy_from_slice(&cache.result);
        result
    })
}

/// Physical excavation volumes crossing the radial draw boundary. They cap
/// clipped tunnels without projecting nearby mouths over unrelated outdoor sky.
pub(crate) fn far_limits(camera: Vec3, distance: f32) -> [Vec4; 128] {
    let _ = portal_uniforms(camera, distance);
    PORTALS.with_borrow(|cache| {
        let mut result = [Vec4::ZERO; 128];
        result[..cache.limits.len()].copy_from_slice(&cache.limits);
        result
    })
}

pub(crate) struct DenPlugin;
impl Plugin for DenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stream>()
            .add_systems(Update, stream.in_set(Layer::Dens))
            .add_plugins(lore::LorePlugin);
    }
}
/// Same-binary partition attribution; normal play uses eight-metre cells.
fn mesh_cell_size() -> f32 {
    if std::env::var_os("HITHER_PROFILE_FOREST").is_some()
        && let Ok(value) = std::env::var("HITHER_PROFILE_DEN_CELL_SIZE")
    {
        let size: f32 = value.parse().expect("den cell size in metres");
        assert!(size.is_finite() && (1.0..=64.0).contains(&size));
        return size;
    }
    8.0
}

#[derive(Resource, Default)]
struct Stream {
    roots: HashMap<IVec2, Entity>,
    pending: HashMap<IVec2, crate::world::streaming::BuildTask<Vec<(usize, Mesh)>>>,
    installing: HashMap<IVec2, DenInstall>,
    window: Option<(IVec2, i32)>,
    candidates: Vec<(IVec2, Arc<Den>)>,
    materials: Vec<Handle<StandardMaterial>>,
}
struct DenInstall {
    root: Entity,
    parts: VecDeque<(usize, Mesh)>,
    lamps: VecDeque<Vec3>,
}
#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn stream(
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    settings: Res<crate::app::settings::GraphicsSettings>,
    range: Option<Res<crate::rendering::view_distance::Range>>,
    mut stream: ResMut<Stream>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if stream.materials.is_empty() {
        stream.materials = art::materials(&mut images, &mut materials);
    }
    let distance = range
        .as_ref()
        .map_or(settings.render_distance + 4.0, |r| r.load)
        + REACH;
    stream.roots.retain(|cell, e| {
        let keep = site(*cell).is_some_and(|d| d.center.distance(rig.position) < distance + 60.);
        if !keep {
            crate::world::streaming::retire(&mut commands, *e);
            if let Some(den) = site(*cell) {
                // Never wait on a meshing worker during eviction.
                if let Ok(mut built) = den.built.try_lock() {
                    *built = None;
                }
            }
        }
        keep
    });
    stream.installing.retain(|cell, _| {
        site(*cell).is_some_and(|d| d.center.distance(rig.position) < distance + 60.)
    });
    // Obsolete workers retain permits until completion; never abandon a running mesher.
    stream.pending.retain(|cell, task| {
        !task.is_ready() || site(*cell).is_some_and(|d| d.center.distance(rig.position) <= distance)
    });
    let c = (rig.position.xz() / CELL).floor().as_ivec2();
    let r = (distance / CELL).ceil() as i32;
    if stream.window != Some((c, r)) {
        stream.window = Some((c, r));
        stream.candidates = crate::world::streaming::entering_cells(c, r, None)
            .filter_map(|cell| site(cell).map(|den| (cell, den)))
            .collect();
    }
    let mut candidates = stream.candidates.clone();
    candidates.sort_by(|a, b| {
        a.1.center
            .distance_squared(rig.position)
            .total_cmp(&b.1.center.distance_squared(rig.position))
    });
    for (cell, den) in candidates {
        if stream.roots.contains_key(&cell) {
            continue;
        }
        if den.center.distance(rig.position) > distance {
            continue;
        }
        if !stream.pending.contains_key(&cell) {
            if stream.pending.len() >= 2 {
                continue;
            }
            let Some(permit) = coordinator.worker(Layer::Dens) else {
                continue;
            };
            let den = den.clone();
            stream.pending.insert(
                cell,
                crate::world::streaming::BuildTask::new(permit, move || {
                    den.built()
                        .meshes
                        .iter()
                        .enumerate()
                        .flat_map(|(kind, geometry)| {
                            geometry
                                .spatial_meshes(mesh_cell_size())
                                .into_iter()
                                .map(move |mesh| (kind, mesh))
                        })
                        .collect()
                }),
            );
        }
        let task = stream.pending.get_mut(&cell).unwrap();
        if !task.is_ready() {
            continue;
        }
        let Some(_installation) = coordinator.install_entities(Layer::Dens, 1, 0, 1) else {
            continue;
        };
        let ready_meshes = task.take();
        stream.pending.remove(&cell);
        let built = den.built();
        let root = commands
            .spawn((
                Name::new("Goblin den / procedural mountain settlement"),
                Transform::from_translation(den.center),
                Visibility::default(),
            ))
            .id();
        stream.installing.insert(
            cell,
            DenInstall {
                root,
                parts: ready_meshes.into(),
                lamps: built.lamps.iter().copied().collect(),
            },
        );
        info!(
            "Goblin den {:?}: {} houses, {} residents, {} entrances at {}",
            den.cell,
            den.houses.len(),
            den.houses.len() * 4,
            den.tunnels.len(),
            den.center
        );
        if std::env::var_os("HITHER_GOBLIN_DEN_PREVIEW").is_some() {
            println!(
                "Goblin den ready: {} houses, {} residents, {} entrances",
                den.houses.len(),
                den.houses.len() * 4,
                den.tunnels.len()
            );
        }
        stream.roots.insert(cell, root);
    }
    let material_handles = stream.materials.clone();
    stream.installing.retain(|_, install| {
        for _ in 0..8 {
            if let Some((_, mesh)) = install.parts.front() {
                let bytes = crate::world::streaming::mesh_bytes(mesh);
                let Some(_scope) = coordinator.install_entities(Layer::Dens, 2, bytes, 1) else {
                    break;
                };
                let (kind, mesh) = install.parts.pop_front().unwrap();
                let mut part = commands.spawn((
                    crate::rendering::point_shadow_cache::StaticPointShadowCaster,
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(material_handles[kind].clone()),
                    Transform::default(),
                    ChildOf(install.root),
                ));
                if kind == art::GLOW {
                    part.insert(bevy::light::NotShadowCaster);
                }
            } else if let Some(&at) = install.lamps.front() {
                let Some(_scope) = coordinator.install_entities(Layer::Dens, 1, 0, 1) else {
                    break;
                };
                install.lamps.pop_front();
                commands.spawn((
                    crate::rendering::lighting::LightEmitter {
                        color: [1.0, 0.68, 0.34],
                        range: 14.0,
                        ..crate::rendering::lighting::LightEmitter::new(18000.0)
                    },
                    Transform::from_translation(at),
                    ChildOf(install.root),
                ));
            } else {
                break;
            }
        }
        !install.parts.is_empty() || !install.lamps.is_empty()
    });
}
pub(crate) fn preview() -> Option<Vec3> {
    for r in 0..80_i32 {
        for x in -r..=r {
            for z in -r..=r {
                if x.abs() != r && z.abs() != r {
                    continue;
                }
                if let Some(den) = site(IVec2::new(x, z)) {
                    return Some(den.center);
                }
            }
        }
    }
    None
}
/// Reproducible views of generated spaces, never used by normal launches.
pub(crate) fn preview_pose() -> Option<(Vec3, Vec3)> {
    let center = preview()?;
    let den = near(center.xz())?;
    let mode = std::env::var("HITHER_GOBLIN_DEN_PREVIEW").unwrap_or_default();
    if mode == "tunnel-junction" {
        let path = &den.tunnels[0];
        let incoming = (path[1] - path[0]).with_y(0.).normalize();
        let side = Vec3::Y.cross(incoming);
        return Some((
            center + path[1] - incoming * 3.5 + side * 2.5 + Vec3::Y * 1.2,
            center + path[1] - Vec3::Y * 1.0,
        ));
    }
    if mode == "tunnel-relic" {
        let built = den.built();
        let relic = built.relics.iter().find(|r| r.entry == 13)?;
        let path = &den.tunnels[0];
        let near = path.iter().min_by(|a, b| {
            a.distance_squared(relic.at)
                .total_cmp(&b.distance_squared(relic.at))
        })?;
        return Some((center + *near, center + relic.at));
    }
    if mode == "tunnel" || mode == "entrance" {
        let path = &den.tunnels[0];
        if mode == "tunnel" {
            return Some((
                center + path[20] + Vec3::Y * 0.05,
                center + path[24] - Vec3::Y * 0.25,
            ));
        }
        let end = *path.last()?;
        let forward = (end - path[path.len() - 2]).with_y(0.).normalize();
        return Some((
            center + end + forward * 6. + Vec3::Y * 2.2,
            center + path[path.len() - 4] - Vec3::Y * 0.2,
        ));
    }
    let h = match mode.as_str() {
        "shrine" | "shrine-relic" => &den.shrine,
        "food-hall" => &den.food_hall,
        "interior" => &den.houses[0],
        _ => {
            return Some((
                center + Vec3::new(18., 1., 6.),
                center + Vec3::new(-2., -5., 0.),
            ));
        }
    };
    let front = Quat::from_rotation_y(h.yaw) * Vec3::Z;
    if mode == "shrine-relic" {
        let built = den.built();
        let relic = built.relics.iter().find(|r| r.entry == 0)?;
        let target = center + relic.at;
        return Some((target + front * 2.6 - Vec3::Y * 0.4, target));
    }
    let back = -h.depth * 0.5 + 0.95;
    let distance = if mode == "shrine-relic" {
        back + 2.6
    } else {
        h.depth * 0.5 - 0.4
    };
    let target_height = if mode == "shrine-relic" {
        1.78
    } else if mode == "shrine" {
        1.25
    } else {
        0.65
    };
    Some((
        center + h.at + front * distance + Vec3::Y * 1.30,
        center + h.at + front * back + Vec3::Y * target_height,
    ))
}
#[cfg(test)]
mod tests;

#[cfg(test)]
fn uncached_portal_uniforms(camera: Vec3, distance: f32) -> [Vec4; MAX_PORTAL_SEGMENTS * 2] {
    let mut result = [Vec4::ZERO; MAX_PORTAL_SEGMENTS * 2];
    let mut n = 0;
    let c = (camera.xz() / CELL).floor().as_ivec2();
    let r = ((distance + REACH) / CELL).ceil() as i32;
    for x in -r..=r {
        for z in -r..=r {
            if let Some(den) = site(c + IVec2::new(x, z)) {
                for path in &den.tunnels {
                    for w in path.windows(2) {
                        let a = den.center + w[0];
                        let b = den.center + w[1];
                        if a.distance(camera) > distance + 8. && b.distance(camera) > distance + 8.
                        {
                            continue;
                        }
                        if [a, a.lerp(b, 0.5), b]
                            .iter()
                            .all(|p| p.y + 2.5 < terrain::height(p.xz()))
                        {
                            continue;
                        }
                        if n == MAX_PORTAL_SEGMENTS {
                            return result;
                        }
                        result[n * 2] = a.extend(3.5);
                        result[n * 2 + 1] = b.extend((den.seed % 4096) as f32);
                        n += 1;
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
#[test]
fn cached_portals_match_exposure_and_distance_filters() {
    let den = (-60..=60)
        .flat_map(|x| (-60..=60).map(move |z| IVec2::new(x, z)))
        .find_map(site)
        .expect("seed has a mountain settlement");
    let exit = den.center + *den.tunnels[0].last().unwrap();
    let result = portal_uniforms(exit, 48.);
    assert!(result.iter().any(|p| p.w > 0.));
    for distance in [48., 260., 512., 48.] {
        for p in [
            exit,
            exit + Vec3::new(CELL, 20., 0.),
            exit - Vec3::X * 16.,
            exit,
        ] {
            assert_eq!(
                portal_uniforms(p, distance),
                uncached_portal_uniforms(p, distance)
            );
        }
    }

    for distance in [48., 260., 512., 48.] {
        for p in [
            Vec3::ZERO,
            Vec3::new(592., 30., 1344.),
            Vec3::new(-1500., 200., 850.),
            Vec3::ZERO,
        ] {
            assert_eq!(
                portal_uniforms(p, distance),
                uncached_portal_uniforms(p, distance)
            );
        }
    }
}

#[cfg(test)]
#[test]
fn located_center_streams_a_visible_den() {
    let response = crate::world::locate::Target::GoblinDen.locate(Vec3::ZERO);
    let coordinates = response.split("X ").nth(1).expect("locate must find a den");
    let (x, rest) = coordinates.split_once(", Y ").unwrap();
    let (y, rest) = rest.split_once(", Z ").unwrap();
    let z = rest.split_once(' ').unwrap().0;
    // Use the rounded coordinates the player actually receives in chat.
    let destination = Vec3::new(x.parse().unwrap(), y.parse().unwrap(), z.parse().unwrap());
    let cell = (destination.xz() / CELL).floor().as_ivec2();
    let den = site(cell).unwrap();
    assert!(destination.distance(den.center) < 0.01);
    assert!(
        terrain::height(destination.xz()) > destination.y + 40.0,
        "reported center must be inside a mountain, not in the air"
    );
    assert!(den.air(destination - den.center) < -2.0);

    let mut app = App::new();
    app.add_plugins(crate::world::streaming::StreamingPlugin);
    let mut time = Time::<()>::default();
    time.advance_by(std::time::Duration::from_secs_f32(0.5));
    app.add_plugins(bevy::app::TaskPoolPlugin::default())
        .insert_resource(time)
        .insert_resource(crate::player::camera::CameraRig {
            position: destination,
            ..default()
        })
        .insert_resource(crate::app::settings::GraphicsSettings {
            render_distance: 48.,
            ..default()
        })
        .init_resource::<Stream>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(Update, stream.in_set(Layer::Dens));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        app.update();
        if let Some(&root) = app.world().resource::<Stream>().roots.get(&cell) {
            assert_eq!(
                app.world().get::<Transform>(root).unwrap().translation,
                den.center
            );
            assert!(
                app.world()
                    .get::<Children>(root)
                    .unwrap()
                    .iter()
                    .any(|child| { app.world().get::<Mesh3d>(child).is_some() })
            );
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "den failed to stream at {destination}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(test)]
#[test]
fn world_placement_rejects_a_den_in_the_air_above_plains() {
    let center = preview().expect("seed must contain a den");
    let cell = (center.xz() / CELL).floor().as_ivec2();
    let mut den = generate(cell, biome::world_seed()).unwrap();
    assert!(den.fits_world_terrain());
    // Keep a completely generated layout, but put it at the reported failure
    // height over actual plains. A valid layout alone must not qualify a site.
    let plains = (-100..=100)
        .find_map(|x| {
            (-100..=100).find_map(|z| {
                let p = Vec2::new(x as f32 * 20., z as f32 * 20.);
                (biome::at(p) == biome::Biome::Plains && terrain::height(p) < 1.).then_some(p)
            })
        })
        .unwrap();
    den.center = Vec3::new(plains.x, 125., plains.y);
    assert!(!den.fits_world_terrain());
    // Reject a layout floating above a real mountain as well.
    den.center = center.with_y(terrain::height(center.xz()) + 125.);
    assert!(!den.fits_world_terrain());
}
