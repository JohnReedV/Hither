//! Seeded, barred underground dens and skinned orc encounters.
use crate::rendering::geometry::Geometry;
use crate::world::random::Rng;
use crate::world::streaming::{Coordinator, Layer};
use bevy::world_serialization::WorldInstanceReady;
use bevy::{asset::RenderAssetUsages, prelude::*};
use bevy::{
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::{
    f32::consts::{PI, TAU},
    sync::OnceLock,
};
mod collision;
mod floor_regions;
mod floors;
pub(crate) mod navigation;
mod torch;

pub(crate) const CELL: f32 = 288.0;
const HOLE_RADIUS: f32 = 2.2;
const HOLE_LENGTH: f32 = 4.0;
const GRATE_HEIGHT: f32 = 0.14;
const EMERGENCE_SECONDS: f32 = 7.5;
const STONE_PUSH_SECONDS: f32 = EMERGENCE_SECONDS;
// Matches the palm-to-center offset baked by tools/build_orcs.py.
const STONE_BEARING_Y: f32 = -1.40;
const TARGET_DISTANCE: f32 = 50.0;
#[cfg(test)]
fn can_target(orc: Vec3, player: Vec3, spectator: bool) -> bool {
    can_pursue(orc, player, spectator, false)
}
const PURSUIT_DISTANCE: f32 = 192.;
fn can_pursue(orc: Vec3, player: Vec3, spectator: bool, engaged: bool) -> bool {
    let distance = if engaged {
        PURSUIT_DISTANCE
    } else {
        TARGET_DISTANCE
    };
    !spectator && orc.xz().distance_squared(player.xz()) <= distance * distance
}
const WALK_SPEED_MULTIPLIER: f32 = 1.4;

fn footprint_yaw(movement: Vec3) -> f32 {
    // Snow's toe axis is (sin(yaw), -cos(yaw)), not the model's +Z axis.
    movement.x.atan2(-movement.z)
}
fn den_kind(cell: IVec2) -> usize {
    (mix(seed(cell) ^ 0x6c6169727374796c) % 3) as usize
}
pub(crate) const MAX_HOLES: usize = 32;
// Physics queries are shared by the player and AI. Publish live hatch state
// under a lock rather than treating every procedural entrance as permanently solid.
static OPEN_GATES: OnceLock<RwLock<HashSet<IVec2>>> = OnceLock::new();
static GATE_OPENNESS: OnceLock<RwLock<HashMap<IVec2, f32>>> = OnceLock::new();
fn gate_openness() -> &'static RwLock<HashMap<IVec2, f32>> {
    GATE_OPENNESS.get_or_init(|| RwLock::new(HashMap::new()))
}
fn stone_contact(phase: f32) -> Vec3 {
    static CONTACT: OnceLock<Vec<[f32; 3]>> = OnceLock::new();
    let samples = CONTACT.get_or_init(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/orcs/stone-contact.json"
        )))
        .expect("baked palm trajectory")
    });
    let frame = (phase.clamp(0., 0.7) * STONE_PUSH_SECONDS * 60.).min((samples.len() - 1) as f32);
    let i = frame.floor() as usize;
    Vec3::from_array(samples[i]).lerp(
        Vec3::from_array(samples[(i + 1).min(samples.len() - 1)]),
        frame.fract(),
    ) + stone_carrier(phase)
}
fn stone_position(phase: f32) -> Vec3 {
    if phase <= 0.7 {
        return stone_contact(phase);
    }
    let release = stone_contact(0.7);
    let velocity = (release - stone_contact(0.7 - 1. / (STONE_PUSH_SECONDS * 60.))) * 60.;
    // Ballistic release inherits the actual palm's velocity, then gravity.
    let g = 9.81;
    let landing = (velocity.y + (velocity.y * velocity.y + 2. * g * (release.y - 1.1)).sqrt()) / g;
    let t = ((phase - 0.7) * STONE_PUSH_SECONDS).min(landing);
    let mut p = release + velocity * t - Vec3::Y * (0.5 * g * t * t);
    let horizontal = velocity.with_y(0.);
    let speed = horizontal.length();
    let roll = (((phase - 0.7) * STONE_PUSH_SECONDS) - landing)
        .max(0.)
        .min(speed / 0.75);
    p += horizontal.normalize_or_zero() * (speed * roll - 0.375 * roll * roll);
    p
}
fn stone_carrier(open: f32) -> Vec3 {
    let carry = ((open - 0.25) / 0.40).clamp(0., 1.);
    let z = 2.6 + 2.7 * carry;
    let lift = (open / 0.25).clamp(0., 1.);
    let x = -1.7 * (1. - lift * lift * (3. - 2. * lift));
    Vec3::new(
        x,
        if z < 4. {
            ramp_height(x, z)
        } else {
            GRATE_HEIGHT
        },
        z,
    )
}
fn gate_pose(kind: usize, open: f32) -> Transform {
    if kind == 1 {
        // Keep the carry orientation until clear of the carrier. Tumbling a
        // wide boulder immediately would sweep its side through his head.
        let flight = ((open - 0.86) * STONE_PUSH_SECONDS).clamp(0., 0.65);
        Transform::from_translation(stone_position(open))
            .with_rotation(Quat::from_rotation_z(-flight * 3.))
    } else if kind == 2 {
        let pull = open * open * (3. - 2. * open);
        // Draw the woven plug into a side pocket below the lip, not upward
        // into a roof. Its actual moving volume is shared with physics.
        Transform::from_xyz(3.65 * pull, -1.48 - 0.35 * pull, 1.0 - 1.5 * pull)
            .with_rotation(Quat::from_rotation_y(-pull * 0.8))
    } else {
        Transform::from_xyz(-2.2, 0.1, 0.).with_rotation(Quat::from_rotation_z(open * 1.45))
    }
}
fn open_gates() -> &'static RwLock<HashSet<IVec2>> {
    OPEN_GATES.get_or_init(|| RwLock::new(HashSet::new()))
}
fn gate_is_open(cell: IVec2) -> bool {
    den_kind(cell) != 0 || open_gates().read().unwrap().contains(&cell)
}
fn boulder_blocks(feet: Vec3, radius: f32, phase: f32) -> bool {
    let center = stone_position(phase);
    let nearest = Vec3::new(
        feet.x,
        center.y.clamp(feet.y + radius, feet.y + 1.65),
        feet.z,
    );
    let size = if phase > 0.86 {
        Vec3::splat(1.9)
    } else {
        Vec3::new(1.9, 1.52, 1.95)
    };
    ((nearest - center) / (size + Vec3::splat(radius))).length_squared() < 1.
}
fn root_plug_blocks(feet: Vec3, radius: f32, phase: f32) -> bool {
    let pose = gate_pose(2, phase);
    let center = feet + Vec3::Y * 0.82;
    let q = pose.rotation.inverse() * (center - pose.translation);
    q.z.abs() < 0.42 + radius
        && (q.x / (1.78 + radius)).powi(2) + (q.y / (1.42 + 0.82)).powi(2) < 1.
}
// Preserve max/min behavior for non-finite inputs; clamp propagates NaN.
#[allow(clippy::manual_clamp)]
fn burrow_width(z: f32) -> f32 {
    if z < -4. {
        let t = ((-4. - z) / 2.).clamp(0., 1.);
        return 1.8 + 0.4 * t * t * (3. - 2. * t);
    }
    let cap = (z.abs() - 3.6).max(0.).min(0.4);
    1.8 + (0.16 - cap * cap).max(0.).sqrt()
}
fn ramp_height(x: f32, z: f32) -> f32 {
    let t = ((4. - z) / 8.).clamp(0., 1.);
    0.045 - 5.845 * t
        + t * (1. - t) * (0.68 * (x * 1.7 + z * 0.8).sin() + 0.20 * (x * 5.1 - z * 3.3).sin())
        + 1.35 * t * (x.abs() / 2.2).min(1.).powi(8)
}
#[cfg(test)]
fn tunnel_floor(p: Vec2) -> Option<f32> {
    (p.y >= -14.6).then(|| floors::mesh(0).floor(p)).flatten()
}
fn tunnel_center(z: f32) -> Vec3 {
    let centers = [
        Vec3::new(0., -5.8, -4.),
        Vec3::new(0., -6., -7.),
        Vec3::new(1., -6.1, -10.),
        Vec3::new(3., -6.2, -13.),
        Vec3::new(6., -6.2, -15.),
    ];
    let i = (0..4).find(|&i| z >= centers[i + 1].z).unwrap_or(3);
    let t = ((z - centers[i].z) / (centers[i + 1].z - centers[i].z)).clamp(0., 1.);
    let a = centers[i.saturating_sub(1)];
    let b = centers[i];
    let c = centers[i + 1];
    let d = centers[(i + 2).min(4)];
    let p = 0.5
        * ((2. * b)
            + (-a + c) * t
            + (2. * a - 5. * b + 4. * c - d) * t * t
            + (-a + 3. * b - 3. * c + d) * t * t * t);
    Vec3::new(p.x, p.y, z)
}
fn ramp_half_width(kind: usize, z: f32) -> f32 {
    if kind == 2 {
        burrow_width(z) - 0.12
    } else {
        2.08
    }
}

// Use the same expanded shapes for landing and side collision. A smaller
// visual-only support ellipsoid leaves a player standing inside the collider.
fn moving_gate_top(q: Vec3, radius: f32, kind: usize, phase: f32) -> Option<f32> {
    if kind == 1 {
        let center = stone_position(phase);
        let size = if phase > 0.86 {
            Vec3::splat(1.9)
        } else {
            Vec3::new(1.9, 1.52, 1.95)
        };
        let d = ((q.x - center.x) / (size.x + radius)).powi(2)
            + ((q.z - center.z) / (size.z + radius)).powi(2);
        (d < 1.).then(|| center.y + (size.y + radius) * (1. - d).sqrt() - radius + 0.002)
    } else if kind == 2 {
        let pose = gate_pose(kind, phase);
        let local = pose.rotation.inverse() * (q - pose.translation);
        let d = (local.x / (1.78 + radius)).powi(2);
        (local.z.abs() < 0.42 + radius && d < 1.)
            .then(|| pose.translation.y + 2.24 * (1. - d).sqrt() - 0.82 + 0.002)
    } else {
        None
    }
}
pub(crate) fn surface_height(p: Vec3, radius: f32) -> Option<f32> {
    let mut levels = floors::levels(p.xz(), radius)?;
    let cell = (p.xz() / CELL).floor().as_ivec2();
    let home = site(cell)?;
    levels.extend(collision::barriers(den_kind(cell)).floors((p - home).xz(), radius));
    let feet = p.y - crate::player::movement::PLAYER_EYE_HEIGHT;
    crate::world::floor::support(levels.iter().copied(), feet, 0.20)
        // Recovery from an old save below its floor uses the lowest layer.
        .or_else(|| levels.into_iter().min_by(f32::total_cmp))
}
#[cfg(test)]
fn base_surface_height(p: Vec3, radius: f32) -> Option<f32> {
    let levels = floors::levels(p.xz(), radius)?;
    crate::world::floor::support(
        levels.iter().copied(),
        p.y - crate::player::movement::PLAYER_EYE_HEIGHT,
        0.20,
    )
    .or_else(|| levels.into_iter().min_by(f32::total_cmp))
}
pub(crate) fn underground_collision(p: Vec3, radius: f32) -> bool {
    let cell = (p.xz() / CELL).floor().as_ivec2();
    let Some(home) = site(cell) else {
        return false;
    };
    let q = p - home - Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
    if q.xz().length_squared() > 23. * 23. {
        return false;
    }
    let kind = den_kind(cell);
    let phase = *gate_openness().read().unwrap().get(&cell).unwrap_or(&0.);
    if (kind == 1 && boulder_blocks(q, radius, phase))
        || (kind == 2 && root_plug_blocks(q, radius, phase))
    {
        return true;
    }
    if kind == 1
        && q.y >= -0.15
        && let Some(top) = surface_height(p, radius)
        && top > GRATE_HEIGHT + 0.2
        && q.y < top - 0.2
    {
        return true;
    }
    floors::body_blocks(q, radius, kind, gate_is_open(cell))
}

/// Resolve an existing overlap (a moving gate, a fall beside a bank, or an
/// already embedded player) before accepting more movement. Recovery stays
/// near the player and uses the underground floor, never the ground overhead.
pub(crate) fn recover_player_position(p: Vec3, radius: f32) -> Option<Vec3> {
    let cell = (p.xz() / CELL).floor().as_ivec2();
    let home = site(cell)?;
    if p.xz().distance_squared(home.xz()) > 23. * 23. {
        return None;
    }
    let feet = p.y - crate::player::movement::PLAYER_EYE_HEIGHT;
    let embedded = surface_height(p, radius).is_some_and(|floor| feet < floor - 0.201);
    let connected =
        floor_regions::connected(p - Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT);
    if !underground_collision(p, radius) && !embedded && connected {
        return None;
    }
    let mut best: Option<Vec3> = None;
    // Only the exceptional overlap path searches. Ordinary walking and falling
    // pay for one collision query, not a neighborhood search.
    for ring in 0..=60 {
        let distance = ring as f32 * 0.05;
        for direction in 0..if ring == 0 { 1 } else { 32 } {
            let angle = direction as f32 * TAU / 32.;
            let at = p + Vec3::new(angle.cos(), 0., angle.sin()) * distance;
            let Some(mut floor) = surface_height(at, radius) else {
                continue;
            };
            if feet < -0.15 && floor >= 0. {
                let Some(below) = floors::levels(at.xz(), radius)
                    .into_iter()
                    .flatten()
                    .filter(|y| *y < 0.)
                    .max_by(f32::total_cmp)
                else {
                    continue;
                };
                floor = below;
            }
            let mut height = feet.max(floor);
            if let Some(ceiling) = ceiling_at(at.with_y(floor)) {
                let maximum = ceiling - collision::BODY_HEIGHT - 0.002;
                if maximum < floor {
                    continue;
                }
                height = height.min(maximum);
            }
            let candidate = at.with_y(height + crate::player::movement::PLAYER_EYE_HEIGHT);
            if crate::player::movement::character_collides(candidate, radius)
                || !floor_regions::connected(
                    candidate - Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
                )
                || ceiling_at(candidate).is_some_and(|ceiling| {
                    candidate.y - crate::player::movement::PLAYER_EYE_HEIGHT
                        + collision::BODY_HEIGHT
                        > ceiling
                })
            {
                continue;
            }
            if best.is_none_or(|best| candidate.distance_squared(p) < best.distance_squared(p)) {
                best = Some(candidate);
            }
        }
        // Prefer the nearest horizontal correction. A deep floor penetration
        // should lift in place, not slide several metres down a sloping ramp.
        if best.is_some() {
            break;
        }
    }
    best
}

fn mix(mut n: u64) -> u64 {
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
fn seed(cell: IVec2) -> u64 {
    mix(crate::world::biome::world_seed()
        ^ (cell.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (cell.y as i64 as u64).wrapping_mul(0x85ebca6b)
        ^ 0x6f726364656e)
}
fn candidate_position(cell: IVec2, hash: u64) -> Vec2 {
    let mut rng = Rng(hash);
    cell.as_vec2() * CELL + Vec2::new(rng.range(48., 240.), rng.range(48., 240.))
}
type SiteSlot = Option<(IVec2, u64, Option<Vec3>)>;
thread_local! {
    static SITES: std::cell::RefCell<Vec<SiteSlot>> = std::cell::RefCell::new(vec![None; 256]);
}
pub(crate) fn site(cell: IVec2) -> Option<Vec3> {
    let world_seed = crate::world::biome::world_seed();
    let index = seed(cell) as usize & 255;
    SITES.with_borrow_mut(|slots| {
        if let Some((key, seed, value)) = slots[index]
            && key == cell
            && seed == world_seed
        {
            return value;
        }
        let value = uncached_site(cell);
        slots[index] = Some((cell, world_seed, value));
        value
    })
}
fn uncached_site(cell: IVec2) -> Option<Vec3> {
    let h = seed(cell);
    // Same 1/4 regional acceptance and 288m regions as solitary oaks, but
    // deliberately no climate/forest eligibility filter.
    if h & 3 != 0 {
        return None;
    }
    let p = candidate_position(cell, h);
    if p.abs().max_element() < 20. || crate::world::biome::mountain_amount(p) > 0.15 {
        return None;
    }
    Some(Vec3::new(p.x, 0., p.y))
}
fn sites(p: Vec3, distance: f32) -> Vec<(IVec2, Vec3)> {
    let center = (p.xz() / CELL).floor().as_ivec2();
    let r = (distance / CELL).ceil() as i32 + 1;
    let mut result = Vec::new();
    for x in -r..=r {
        for z in -r..=r {
            let c = center + IVec2::new(x, z);
            if let Some(s) = site(c)
                && s.xz().distance(p.xz()) < distance + 4.
            {
                result.push((c, s));
            }
        }
    }
    result.sort_by(|a, b| a.1.distance_squared(p).total_cmp(&b.1.distance_squared(p)));
    result.truncate(MAX_HOLES);
    result
}
// Sites are at least 48m from cell edges, so this 44m clearing never
// crosses a cell boundary. No neighboring-site search or recursive terrain query.
pub(crate) fn terrain_clearance(p: Vec2) -> f32 {
    site((p / CELL).floor().as_ivec2()).map_or(1.0, |s| {
        crate::world::biome::smooth_range(18.0, 44.0, p.distance(s.xz()))
    })
}

#[derive(Default)]
struct HoleCache {
    key: Option<(IVec2, i32, u64)>,
    candidates: Vec<(IVec2, Vec3)>,
    view: Option<(Vec3, f32)>,
    result: [Vec4; MAX_HOLES],
}
thread_local! { static HOLES: std::cell::RefCell<HoleCache> = std::cell::RefCell::default(); }
pub(crate) fn hole_uniforms(p: Vec3, distance: f32) -> [Vec4; MAX_HOLES] {
    HOLES.with_borrow_mut(|cache| {
        let center = (p.xz() / CELL).floor().as_ivec2();
        let radius = ((distance + 44.) / CELL).ceil() as i32 + 1;
        let key = (center, radius, crate::world::biome::world_seed());
        if cache.key != Some(key) {
            cache.key = Some(key);
            cache.view = None;
            cache.candidates.clear();
            for x in -radius..=radius {
                for z in -radius..=radius {
                    let cell = center + IVec2::new(x, z);
                    if let Some(s) = site(cell) {
                        cache.candidates.push((cell, s));
                    }
                }
            }
        }
        if cache.view != Some((p, distance)) {
            cache.view = Some((p, distance));
            // Keep exact nearest-site truncation and ordering, including ties.
            let mut selected: Vec<_> = cache
                .candidates
                .iter()
                .copied()
                .filter(|(_, s)| s.xz().distance(p.xz()) < distance + 48.)
                .collect();
            selected.sort_by(|a, b| a.1.distance_squared(p).total_cmp(&b.1.distance_squared(p)));
            cache.result.fill(Vec4::ZERO);
            for (out, (cell, s)) in cache.result.iter_mut().zip(selected) {
                *out = Vec4::new(
                    s.x,
                    s.z,
                    HOLE_RADIUS,
                    if den_kind(cell) == 2 {
                        -HOLE_LENGTH
                    } else {
                        HOLE_LENGTH
                    },
                );
            }
        }
        cache.result
    })
}
pub(crate) fn entrance_overlap(p: Vec3, radius: f32) -> bool {
    let cell = (p.xz() / CELL).floor().as_ivec2();
    site(cell).is_some_and(|home| floors::entrance(den_kind(cell), (p - home).xz(), radius))
}
pub(crate) fn ceiling_at(p: Vec3) -> Option<f32> {
    let base = base_ceiling_at(p);
    let cell = (p.xz() / CELL).floor().as_ivec2();
    let Some(home) = site(cell) else {
        return base;
    };
    if p.xz().distance_squared(home.xz()) > 23. * 23. {
        return base;
    }
    let rock = collision::barriers(den_kind(cell))
        .ceiling(p - home, crate::player::movement::PLAYER_RADIUS);
    match (base, rock) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn base_ceiling_at(p: Vec3) -> Option<f32> {
    let cell = (p.xz() / CELL).floor().as_ivec2();
    if den_kind(cell) == 1
        && let Some(home) = site(cell)
    {
        let q = p - home;
        if q.x.abs() < 2.2 && (-4.0..0.95).contains(&q.z) && q.y < 0.2 {
            return Some(0.2);
        }
    }
    if p.y >= 0. {
        return None;
    }
    let s = site((p.xz() / CELL).floor().as_ivec2())?;
    let q = p - s;
    if q.z >= -4. {
        return None;
    }
    floors::mesh(den_kind(cell)).ceiling(q, crate::player::movement::PLAYER_RADIUS)
}
// Camera and actors share the exact same indexed surface triangles.
fn visible_tunnel_bounds(at: Vec2, kind: usize) -> Option<(f32, f32)> {
    let mesh = floors::mesh(kind);
    let floor = mesh.floor(at)?;
    let ceiling = mesh.heights(at, true).into_iter().min_by(f32::total_cmp)?;
    Some((floor, ceiling))
}

/// Only moving colliders intersecting a short camera boom invalidate its solution.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct CameraColliders {
    min: IVec2,
    max: IVec2,
    gates: [(u32, bool); 4],
}
pub(crate) fn camera_colliders(center: Vec3, reach: f32) -> Option<CameraColliders> {
    if !center.is_finite() || !(0. ..CELL * 0.49).contains(&reach) {
        return None;
    }
    let min = ((center.xz() - Vec2::splat(reach)) / CELL)
        .floor()
        .as_ivec2();
    let max = ((center.xz() + Vec2::splat(reach)) / CELL)
        .floor()
        .as_ivec2();
    let phases = gate_openness().read().unwrap();
    let open = open_gates().read().unwrap();
    let mut gates = [(0, false); 4];
    let mut index = 0;
    for x in min.x..=max.x {
        for z in min.y..=max.y {
            let cell = IVec2::new(x, z);
            gates[index] = (
                phases.get(&cell).copied().unwrap_or(0.).to_bits(),
                open.contains(&cell),
            );
            index += 1;
        }
    }
    Some(CameraColliders { min, max, gates })
}
pub(crate) fn camera_clear(p: Vec3, radius: f32) -> bool {
    let cell = (p.xz() / CELL).floor().as_ivec2();
    let Some(home) = site(cell) else {
        return false;
    };
    let q = p - home;
    let kind = den_kind(cell);
    let Some(floor) = surface_height(
        p + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
        radius,
    ) else {
        return false;
    };
    if p.y <= floor + radius
        || collision::barriers(kind).blocks_sized(q - Vec3::Y * radius, radius, 2. * radius)
    {
        return false;
    }
    // Camera clearance uses its sphere, not a standing character above it.
    let phase = *gate_openness().read().unwrap().get(&cell).unwrap_or(&0.);
    if kind == 1 {
        let size = if phase > 0.86 {
            Vec3::splat(1.9)
        } else {
            Vec3::new(1.9, 1.52, 1.95)
        };
        if ((q - stone_position(phase)) / (size + Vec3::splat(radius))).length_squared() < 1. {
            return false;
        }
    } else if kind == 2 {
        let pose = gate_pose(kind, phase);
        let local = pose.rotation.inverse() * (q - pose.translation);
        if local.z.abs() < 0.42 + radius
            && (local.x / (1.78 + radius)).powi(2) + (local.y / (1.42 + radius)).powi(2) < 1.
        {
            return false;
        }
    }
    if entrance_overlap(p, radius)
        && !floors::opening(cell, q.xz(), radius)
        && p.y - radius < GRATE_HEIGHT
        && p.y + radius > GRATE_HEIGHT
    {
        return false;
    }
    if p.y >= radius {
        return true;
    }
    crate::world::floor::footprint(q.xz(), radius).all(|at| {
        let height = if at.y > -4. {
            floors::mesh(kind).floor(at).map(|y| (y, f32::INFINITY))
        } else {
            visible_tunnel_bounds(at, kind)
        };
        height.is_some_and(|(floor, ceiling)| q.y > floor + radius && q.y < ceiling - radius)
    })
}
pub(crate) fn preview() -> Vec3 {
    let requested = std::env::var("HITHER_DEN_KIND")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    for x in -10..10 {
        for z in -10..10 {
            if let Some(p) = site(IVec2::new(x, z))
                .filter(|_| requested.is_none_or(|k| den_kind(IVec2::new(x, z)) == k))
            {
                return p;
            }
        }
    }
    Vec3::ZERO
}
fn party(h: u64) -> Vec<usize> {
    let count = match h & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => 3,
    };
    let mut rng = Rng(mix(h ^ 0x7061727479));
    (0..count).map(|_| (rng.unit() * 3.) as usize).collect()
}

pub struct OrcPlugin;
impl Plugin for OrcPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dens>()
            .init_resource::<Trampling>()
            .add_plugins(torch::TorchPlugin)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (load_models, stream.in_set(Layer::Orcs), encounters, animate)
                    .chain()
                    .after(crate::player::movement::move_camera),
            )
            .add_systems(
                PostUpdate,
                attach_carried_stone
                    .after(bevy::app::AnimationSystems)
                    .before(bevy::transform::TransformSystems::Propagate),
            );
    }
}
#[derive(Resource, Default)]
struct Dens {
    loaded: HashMap<IVec2, Entity>,
    alerted: HashSet<IVec2>,
}
#[derive(Component)]
struct Den {
    cell: IVec2,
    elapsed: Option<f32>,
    emitted: usize,
    open: f32,
}
#[derive(Component)]
struct Gate;
#[derive(Component)]
struct StonePalm {
    owner: Entity,
}

// Evaluate the live joint after animation, before transform propagation. The
// trajectory remains the physics/release source; the visible bearing face uses
// the actual palm even between baked samples and at arbitrary frame rates.
fn attach_carried_stone(
    palms: Query<(Entity, &StonePalm)>,
    orcs: Query<&Orc>,
    dens: Query<(&Den, &Children)>,
    mut transforms: ParamSet<(
        bevy::transform::helper::TransformHelper,
        Query<&mut Transform, With<Gate>>,
    )>,
) {
    static SOCKET: OnceLock<[f32; 3]> = OnceLock::new();
    let socket = Vec3::from_array(*SOCKET.get_or_init(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/orcs/stone-palm.json"
        )))
        .expect("baked palm socket")
    }));
    for (hand, palm) in &palms {
        let Ok(orc) = orcs.get(palm.owner) else {
            continue;
        };
        if !orc.stone_leader || orc.mode != 3 {
            continue;
        }
        let cell = (orc.home.xz() / CELL).floor().as_ivec2();
        let Some((den, children)) = dens.iter().find(|(den, _)| den.cell == cell) else {
            continue;
        };
        if den.open > 0.7 {
            continue;
        }
        let Ok(hand_world) = transforms.p0().compute_global_transform(hand) else {
            continue;
        };
        let lift = (den.open / 0.25).clamp(0., 1.);
        let angle = lift * lift * (3. - 2. * lift) * PI * 0.5;
        let center = hand_world.transform_point(socket) - orc.home
            + Vec3::new(1.95 * angle.cos(), -STONE_BEARING_Y * angle.sin(), 0.);
        for child in children {
            if let Ok(mut gate) = transforms.p1().get_mut(*child) {
                gate.translation = center;
            }
        }
    }
}
#[derive(Resource, Default)]
pub(crate) struct Trampling(pub [Vec4; 16]);
#[derive(Component)]
struct Orc {
    variant: usize,
    home: Vec3,
    age: f32,
    mode: usize,
    step_distance: f32,
    left: bool,
    stone_leader: bool,
    retreat: Option<f32>,
    navigation: navigation::Navigator,
    fall_velocity: f32,
    engaged: bool,
}
#[derive(Component)]
struct OrcAnimator {
    owner: Entity,
    current: usize,
    elapsed: f32,
    choice: u64,
    previous_mode: usize,
    had_target: bool,
}
struct Model {
    scene: Handle<WorldAsset>,
    graph: Handle<AnimationGraph>,
    clips: Vec<AnimationNodeIndex>,
}
#[derive(Resource)]
struct Art {
    gltfs: Vec<Handle<Gltf>>,
    models: Vec<Model>,
    dens: Vec<DenArt>,
    rock: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    shale: Handle<StandardMaterial>,
    torch: torch::TorchArt,
}
fn room_torch_position(kind: usize) -> Vec3 {
    let center = tunnel_center(-8.);
    // Stand clear of the timber upright at x=1.92; the bracket reaches back.
    Vec3::new(
        center.x + if kind == 2 { 1.65 } else { 1.25 },
        center.y + 1.65,
        center.z,
    )
}
pub(crate) fn torch_preview_position() -> Vec3 {
    let p = preview();
    p + room_torch_position(den_kind((p.xz() / CELL).floor().as_ivec2()))
}
struct DenArt {
    earth: Handle<Mesh>,
    metal: Option<Handle<Mesh>>,
    door: Option<Handle<Mesh>>,
}

mod art;
use art::*;

fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut flames: ResMut<Assets<torch::FlameMaterial>>,
) {
    // Build shared collision geometry during loading, before any approach.
    for kind in 0..3 {
        floors::mesh(kind);
        collision::barriers(kind);
    }
    open_gates().write().unwrap().clear();
    gate_openness().write().unwrap().clear();
    let dens = (0..3)
        .map(|kind| {
            let (earth, metal, door) = den_variant(kind);
            DenArt {
                earth: meshes.add(earth.mesh()),
                metal: (!metal.positions.is_empty()).then(|| meshes.add(metal.mesh())),
                door: (!door.positions.is_empty()).then(|| meshes.add(door.mesh())),
            }
        })
        .collect();
    let mut shale = den_material(false, &mut images);
    // Stone is not earth with a gray tint: suppress clod-scale dirt normals and
    // bake a low-contrast mineral grain into its own neutral texture.
    if let Some(image) = shale
        .base_color_texture
        .as_ref()
        .and_then(|h| images.get(h))
    {
        let mut image = image.clone();
        if let Some(data) = image.data.as_mut() {
            for pixel in data.chunks_exact_mut(4) {
                let grain = (pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / 765.;
                let shade = (85. + grain * 135.) as u8;
                pixel[0] = shade;
                pixel[1] = shade;
                pixel[2] = shade.saturating_sub(5);
            }
        }
        if image.texture_descriptor.mip_level_count > 1 {
            crate::rendering::mipmaps::generate(
                &mut image,
                crate::rendering::mipmaps::Filter::Color,
            );
        }
        shale.base_color_texture = Some(images.add(image));
    }
    // Keep the weathered relief: a perfectly smooth normal makes even a
    // fractured silhouette read as a molded plastic prop.
    let torch = torch::build(&mut meshes, &mut materials, &mut images, &mut flames);
    commands.insert_resource(Art {
        gltfs: (0..3)
            .map(|i| server.load(format!("orcs/orc-{i}.glb")))
            .collect(),
        models: Vec::new(),
        dens,
        rock: materials.add(den_material(false, &mut images)),
        iron: materials.add(den_material(true, &mut images)),
        shale: materials.add(shale),
        torch,
    });
}
fn load_models(
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut art: ResMut<Art>,
) {
    if !art.models.is_empty()
        || !art
            .gltfs
            .iter()
            .all(|h| server.is_loaded_with_dependencies(h))
    {
        return;
    }
    let models = art
        .gltfs
        .iter()
        .map(|h| {
            let g = gltfs.get(h).expect("loaded orc GLB");
            let (graph, clips) = AnimationGraph::from_clips(
                [
                    "Idle",
                    "Walk",
                    "Attack",
                    "Push",
                    "IdleWatch",
                    "IdleWeary",
                    "WalkHeavy",
                    "AttackBackhand",
                    "AttackOverhead",
                    "Alert",
                ]
                .map(|n| g.named_animations[n].clone()),
            );
            Model {
                scene: g.scenes[0].clone(),
                graph: graphs.add(graph),
                clips,
            }
        })
        .collect();
    art.models = models;
}
fn stream(
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    range: Res<crate::rendering::view_distance::Range>,
    art: Res<Art>,
    mut dens: ResMut<Dens>,
    orcs: Query<&Orc>,
) {
    let mut wanted = sites(rig.position, range.load);
    // Keep a returning party's home alive even outside the visual range. Orcs
    // disappear inside their den, not at an arbitrary render-distance boundary.
    for orc in &orcs {
        let cell = (orc.home.xz() / CELL).floor().as_ivec2();
        if !wanted.iter().any(|(c, _)| *c == cell) {
            wanted.push((cell, orc.home));
        }
    }
    dens.loaded.retain(|cell, entity| {
        if wanted.iter().any(|(c, _)| c == cell) {
            true
        } else {
            open_gates().write().unwrap().remove(cell);
            gate_openness().write().unwrap().remove(cell);
            commands.entity(*entity).despawn();
            false
        }
    });
    for (cell, p) in wanted {
        if p.xz().distance_squared(rig.position.xz()) < 96. * 96. {
            floor_regions::prepare(p);
        }
        let _installation = if !dens.loaded.contains_key(&cell) {
            coordinator.install(Layer::Orcs, 8, 0)
        } else {
            None
        };
        if !dens.loaded.contains_key(&cell) && _installation.is_none() {
            continue;
        }
        dens.loaded.entry(cell).or_insert_with(|| {
            let kind = den_kind(cell);
            let style = &art.dens[kind];
            commands
                .spawn((
                    Name::new(
                        [
                            "Orc den / barred burrow",
                            "Orc den / Stonejaw quarry",
                            "Orc den / Rootwarren burrow",
                        ][kind],
                    ),
                    Den {
                        cell,
                        elapsed: None,
                        emitted: 0,
                        open: 0.,
                    },
                    Transform::from_translation(p),
                    Visibility::default(),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Mesh3d(style.earth.clone()),
                        MeshMaterial3d(art.rock.clone()),
                        Transform::default(),
                    ));
                    if let Some(mesh) = &style.metal {
                        parent.spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(art.iron.clone()),
                            Transform::default(),
                        ));
                    }
                    if let Some(mesh) = &style.door {
                        parent.spawn((
                            Gate,
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(if kind == 1 {
                                art.shale.clone()
                            } else if kind == 0 {
                                art.iron.clone()
                            } else {
                                art.rock.clone()
                            }),
                            gate_pose(kind, 0.),
                        ));
                    }
                    torch::spawn(parent, &art.torch, room_torch_position(kind), p);
                    parent.spawn((
                        crate::rendering::lighting::LightEmitter {
                            color: if kind == 1 {
                                [0.30, 0.55, 0.65]
                            } else {
                                [0.85, 0.20, 0.035]
                            },
                            lumens: if kind == 0 { 45. } else { 85. },
                            range: 3.5,
                            shadows: crate::rendering::lighting::EmissionShadows::None,
                            ..default()
                        },
                        Transform::from_xyz(1.4, -4.3, -4.5),
                    ));
                })
                .id()
        });
    }
}
#[allow(clippy::too_many_arguments)]
fn encounters(
    mut commands: Commands,
    time: Res<Time>,
    game: Res<crate::app::GameState>,
    rig: Res<crate::player::camera::CameraRig>,
    art: Res<Art>,
    mut dens: ResMut<Dens>,
    mut holes: Query<(&mut Den, &Transform, &Children), Without<Gate>>,
    mut gates: Query<&mut Transform, With<Gate>>,
    orcs: Query<(&Orc, &Transform), Without<Gate>>,
) {
    if game.paused || art.models.len() != 3 {
        return;
    }
    for (mut den, t, children) in &mut holes {
        let player_near = !rig.is_spectating()
            && rig
                .position
                .distance(t.translation + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT)
                <= 10.;
        let active = orcs.iter().any(|(o, _)| o.home == t.translation);
        // An unloaded, completed den must not retain a permanent spent flag.
        if den.elapsed.is_none() && !active && !player_near {
            dens.alerted.remove(&den.cell);
        }
        if den.elapsed.is_none() && player_near && dens.alerted.insert(den.cell) {
            den.elapsed = Some(0.);
        }
        let Some(mut elapsed) = den.elapsed else {
            continue;
        };
        elapsed += time.delta_secs().min(0.05);
        den.elapsed = Some(elapsed);
        let stone = den_kind(den.cell) == 1;
        let party = party(mix(seed(den.cell) ^ 0x656e636f756e7465));
        let finish = 2. + EMERGENCE_SECONDS;
        let occupied = !rig.is_spectating()
            && rig.position.y - rig.landing_offset
                < crate::player::movement::PLAYER_EYE_HEIGHT - 0.15
            && rig.position.xz().distance(t.translation.xz()) < 22.;
        let returning = orcs.iter().any(|(o, p)| {
            o.home == t.translation
                && !can_pursue(p.translation, rig.position, rig.is_spectating(), o.engaged)
                && (o.retreat.is_some()
                    || p.translation.xz().distance(return_mouth(o.home).xz()) < 2.5)
        });
        // Residents alone do not reverse a chasing party's closing lid.
        // A player inside must always retain a way out, even if they entered
        // during closing or were recovered from an overlap beneath the lid.
        let using_entrance = occupied
            || orcs.iter().any(|(o, p)| {
                o.home == t.translation
                    && (o.age < EMERGENCE_SECONDS || needs_exit_gate(p.translation, o.home))
            });
        let open = if elapsed < finish {
            if stone {
                ((elapsed - 0.1) / STONE_PUSH_SECONDS).clamp(0., 1.)
            } else {
                (elapsed / 1.2).clamp(0., 1.)
            }
        } else if stone {
            // A thrown boulder cannot replay its flight backward to close.
            // Leave it aside while the party is active; reset only after all
            // residents are inside and the player has left the encounter.
            if !active && !player_near && !occupied {
                0.
            } else {
                1.
            }
        } else {
            returning_gate_phase(
                den.open,
                time.delta_secs(),
                returning || occupied,
                using_entrance,
            )
        };
        den.open = open;
        gate_openness().write().unwrap().insert(den.cell, open);
        {
            let mut state = open_gates().write().unwrap();
            if open > if stone { 0.97 } else { 0.72 } {
                state.insert(den.cell);
            } else {
                state.remove(&den.cell);
            }
        }
        for child in children {
            if let Ok(mut gate) = gates.get_mut(*child) {
                *gate = gate_pose(den_kind(den.cell), open);
            }
        }
        if encounter_can_reset(
            den.emitted,
            party.len(),
            active,
            player_near || occupied,
            open,
        ) && elapsed > finish
        {
            den.elapsed = None;
            den.emitted = 0;
            dens.alerted.remove(&den.cell);
            continue;
        }
        // Materialize the whole encounter in one command batch. Residents
        // immediately use ordinary navigation; only the lid carrier is scripted.
        while den.emitted < party.len() {
            let variant = party[den.emitted];
            let stone_leader = stone && den.emitted == 0;
            den.emitted += 1;
            commands
                .spawn((
                    Name::new(
                        [
                            "Orc / mace brute",
                            "Orc / ironcap axeman",
                            "Orc / crested raider",
                        ][variant],
                    ),
                    Orc {
                        variant,
                        home: t.translation,
                        age: if stone_leader { 0. } else { EMERGENCE_SECONDS },
                        mode: 1,
                        step_distance: 0.,
                        left: false,
                        stone_leader,
                        retreat: None,
                        navigation: navigation::Navigator::pursuit(),
                        fall_velocity: 0.,
                        engaged: true,
                    },
                    WorldAssetRoot(art.models[variant].scene.clone()),
                    Transform::from_translation(
                        t.translation
                            + if stone_leader {
                                stone_carrier(0.)
                            } else {
                                emergence_position(0.)
                            },
                    )
                    .with_scale(Vec3::new(1.18, 1.12, 1.12)),
                    Visibility::default(),
                ))
                .observe(ready);
        }
    }
}
fn ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    orcs: Query<&Orc>,
    art: Res<Art>,
    mut players: Query<&mut AnimationPlayer>,
    names: Query<&Name>,
) {
    let Ok(orc) = orcs.get(event.entity) else {
        return;
    };
    for e in children.iter_descendants(event.entity) {
        if orc.stone_leader && names.get(e).is_ok_and(|name| name.as_str() == "hand_l") {
            commands.entity(e).insert(StonePalm {
                owner: event.entity,
            });
        }
        if let Ok(mut player) = players.get_mut(e) {
            let model = &art.models[orc.variant];
            let mut transitions = AnimationTransitions::new();
            let carrying = orc.stone_leader && orc.age < STONE_PUSH_SECONDS;
            let initial = if carrying { 3 } else { 1 };
            let animation =
                transitions.play(&mut player, model.clips[initial], std::time::Duration::ZERO);
            animation.repeat();
            if carrying {
                let cell = (orc.home.xz() / CELL).floor().as_ivec2();
                let phase = *gate_openness().read().unwrap().get(&cell).unwrap_or(&0.);
                animation
                    .set_speed(0.)
                    .set_seek_time(phase * STONE_PUSH_SECONDS);
            } else {
                animation.seek_to((mix(event.entity.to_bits()) % 80) as f32 / 100.);
            }
            commands.entity(e).insert((
                AnimationGraphHandle(model.graph.clone()),
                transitions,
                OrcAnimator {
                    owner: event.entity,
                    current: initial,
                    elapsed: 0.,
                    choice: mix(event.entity.to_bits()),
                    previous_mode: initial,
                    had_target: false,
                },
            ));
        }
    }
}
fn emergence_position(age: f32) -> Vec3 {
    let z = -6. + 10.5 * (age / EMERGENCE_SECONDS).clamp(0., 1.);
    let y = floors::mesh(0)
        .rounded_support(Vec2::new(0., z), crate::player::movement::PLAYER_RADIUS)
        .unwrap_or(GRATE_HEIGHT);
    Vec3::new(0., y, z)
}
// Approach from the front before requesting passage into the underground
// layer; the entry radius gives the party room to form a moving queue.
fn return_mouth(home: Vec3) -> Vec3 {
    home + emergence_position(EMERGENCE_SECONDS)
}
fn returning_gate_phase(open: f32, dt: f32, returning: bool, using_entrance: bool) -> f32 {
    if !returning && using_entrance && open >= 1. {
        return open;
    }
    (open + dt.min(0.05) / 1.2 * if returning { 1. } else { -1. }).clamp(0., 1.)
}
fn needs_exit_gate(position: Vec3, home: Vec3) -> bool {
    position.y < GRATE_HEIGHT && position.xz().distance(home.xz()) < 22.
}
fn encounter_can_reset(
    emitted: usize,
    total: usize,
    active: bool,
    occupied: bool,
    open: f32,
) -> bool {
    emitted == total && !active && !occupied && open <= 0.001
}
#[cfg(test)]
fn intruder_in_den(home: Vec3, player: Vec3, spectator: bool) -> bool {
    !spectator
        && player.y < crate::player::movement::PLAYER_EYE_HEIGHT - 0.15
        && player.xz().distance(home.xz()) < 22.
}
fn is_attack_clip(index: usize) -> bool {
    matches!(index, 2 | 7 | 8)
}

// Bevy system parameters declare independent ECS access.
#[allow(clippy::too_many_arguments)]
fn animate(
    mut commands: Commands,
    time: Res<Time>,
    game: Res<crate::app::GameState>,
    rig: Res<crate::player::camera::CameraRig>,
    art: Res<Art>,
    mut orcs: Query<(Entity, &mut Orc, &mut Transform)>,
    mut players: Query<(
        &mut OrcAnimator,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
    mut tracks: ResMut<crate::world::snow::Tracks>,
    mut trampling: ResMut<Trampling>,
    mut navigation_world: Local<navigation::Navigation>,
) {
    let _queries = navigation::FrameQueries::begin();
    let dt = time.delta_secs().min(0.05);
    navigation_world.refresh();
    let positions: Vec<_> = orcs.iter().map(|(e, _, t)| (e, t.translation)).collect();
    navigation_world.retain_actors(&positions);
    let mut navigation_budget = 192;
    let mut planning_time = std::time::Duration::from_millis(1);
    if !game.paused {
        let start = navigation_world.cursor % positions.len().max(1);
        navigation_world.cursor = navigation_world.cursor.wrapping_add(1);
        for (e, _) in positions.iter().cycle().skip(start).take(positions.len()) {
            // Earlier movers have already reserved their new positions. Using
            // the frame-start snapshot lets two residents step into each other.
            let neighbors: Vec<_> = orcs.iter().map(|(id, _, t)| (id, t.translation)).collect();
            let (e, mut orc, mut t) = orcs.get_mut(*e).unwrap();
            let targeting = can_pursue(
                t.translation,
                rig.position,
                rig.is_spectating(),
                orc.engaged,
            );
            orc.engaged = targeting;
            if targeting && orc.retreat.take().is_some() {
                orc.navigation = navigation::Navigator::pursuit();
            }
            orc.age += dt;
            if orc.stone_leader && orc.age < STONE_PUSH_SECONDS {
                let cell = (orc.home.xz() / CELL).floor().as_ivec2();
                let open = *gate_openness().read().unwrap().get(&cell).unwrap_or(&0.);
                t.translation = orc.home + stone_carrier(open);
                t.rotation = Quat::IDENTITY;
                orc.mode = 3;
                continue;
            }
            // Entry, exit and pursuit share the same supported route follower.
            // A timed centerline script cannot detour around a rock or a crowd.
            orc.age = orc.age.max(EMERGENCE_SECONDS);
            // Gravity and overlap recovery use the same barriers as player motion.
            if !navigation_world.settle(e, &mut t.translation, &mut orc.fall_velocity, dt) {
                orc.mode = 0;
                continue;
            }
            let target = if !targeting {
                let mouth = return_mouth(orc.home);
                // Start entering before the bottleneck. Reserving fixed queue
                // slots can order a body already past the mouth behind one
                // outside it, making them face each other forever. Supported
                // navigation and crowd contacts handle the shared passage.
                if orc.retreat.is_some() || t.translation.xz().distance(mouth.xz()) < 2.5 {
                    orc.retreat = Some(0.);
                    let inside = orc.home + emergence_position(0.);
                    if t.translation.distance(inside) < 0.3 {
                        commands.entity(e).despawn();
                        continue;
                    }
                    inside
                } else {
                    mouth
                }
            } else {
                rig.position - Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT
            };
            let reachable = targeting && navigation::attack_reachable(t.translation, target);
            let goal = navigation::chase_goal(target);
            let next = if reachable {
                Some(target)
            } else if let Some(next) = navigation_world.floor_waypoint(e, t.translation, goal, dt) {
                orc.navigation = navigation::Navigator::pursuit();
                orc.navigation.status = crate::world::navigation::Status::Complete;
                Some(next)
            } else {
                navigation_world.graph.begin_slice(planning_time);
                let started = std::time::Instant::now();
                let next = orc.navigation.steer(
                    &mut navigation_world.graph,
                    &navigation::World,
                    crate::world::navigation::Agent::default(),
                    t.translation,
                    goal,
                    dt,
                    &mut navigation_budget,
                );
                planning_time = planning_time.saturating_sub(started.elapsed());
                next
            };
            let delta = (next.unwrap_or(t.translation) - t.translation).with_y(0.);
            let distance = delta.length();
            orc.mode = if reachable {
                2
            } else if next.is_some() {
                1
            } else {
                0
            };
            if distance > 0.01 {
                t.rotation = t.rotation.slerp(
                    Quat::from_rotation_y(delta.x.atan2(delta.z)),
                    (dt * 9.).min(1.),
                );
            }
            if orc.mode == 1 {
                let previous = t.translation;
                t.translation = navigation::step(previous, next.unwrap(), dt, &neighbors, e);
                let movement = t.translation - previous;
                if movement.xz().length_squared() < 0.000001 {
                    orc.mode = 0;
                }
                if movement.xz().length_squared() > 0.000001 {
                    t.rotation = t.rotation.slerp(
                        Quat::from_rotation_y(movement.x.atan2(movement.z)),
                        (dt * 9.).min(1.),
                    );
                }
                orc.step_distance += movement.xz().length();
                if orc.step_distance >= 0.65 {
                    orc.step_distance %= 0.65;
                    orc.left = !orc.left;
                    if t.translation.y < 0.08 {
                        tracks.stamp_orc(t.translation.xz(), footprint_yaw(movement), orc.left);
                    }
                }
            }
        }
    }
    if !game.paused {
        let mut bodies: Vec<_> = orcs
            .iter()
            .map(|(e, o, t)| {
                (
                    e,
                    t.translation,
                    !o.stone_leader || o.age >= STONE_PUSH_SECONDS,
                )
            })
            .collect();
        bodies.sort_by_key(|body| body.0);
        navigation::separate(
            &navigation::World,
            crate::world::navigation::Agent::default(),
            &mut bodies,
            dt,
        );
        for (e, position, movable) in bodies {
            if movable && let Ok((_, _, mut transform)) = orcs.get_mut(e) {
                transform.translation = position;
            }
        }
    }
    trampling.0.fill(Vec4::ZERO);
    let mut feet: Vec<_> = orcs
        .iter()
        .filter(|(_, o, t)| o.age >= EMERGENCE_SECONDS && t.translation.y < 0.15)
        .map(|(_, _, t)| t.translation)
        .collect();
    feet.sort_by(|a, b| {
        a.distance_squared(rig.position)
            .total_cmp(&b.distance_squared(rig.position))
    });
    for (out, p) in trampling.0.iter_mut().zip(feet) {
        *out = Vec4::new(p.x, p.z, 0.52, 1.);
    }
    for (mut animator, mut player, mut transitions) in &mut players {
        if game.paused {
            player.pause_all();
            continue;
        } else {
            player.resume_all();
        }
        let Ok((_, orc, transform)) = orcs.get(animator.owner) else {
            continue;
        };
        animator.elapsed += dt;
        let target = orc.age >= EMERGENCE_SECONDS
            && can_pursue(
                transform.translation,
                rig.position,
                rig.is_spectating(),
                orc.engaged,
            );
        let alert = target && !animator.had_target && orc.mode != 2;
        animator.had_target = target;
        let duration = match animator.current {
            0 => 3.6,
            4 => 4.8,
            5 => 4.2,
            9 => 1.4,
            _ => f32::INFINITY,
        };
        // Attack completion follows the actual one-shot clip, including frame
        // stalls, instead of a second timer that can drift from asset playback.
        let complete = if is_attack_clip(animator.current) {
            player
                .animation(art.models[orc.variant].clips[animator.current])
                .is_some_and(|active| active.is_finished())
        } else {
            animator.elapsed >= duration
        };
        let holding_alert = animator.current == 9 && !complete && target;
        if alert || (!holding_alert && (animator.previous_mode != orc.mode || complete)) {
            animator.choice = mix(animator.choice.wrapping_add(1));
            let index = if alert {
                9
            } else {
                match orc.mode {
                    0 => [0, 4, 5][animator.choice as usize % 3],
                    1 => {
                        if orc.variant == 1 {
                            6
                        } else {
                            1
                        }
                    }
                    2 => [2, 7, 8][animator.choice as usize % 3],
                    _ => 3,
                }
            };
            animator.current = index;
            animator.previous_mode = orc.mode;
            animator.elapsed = 0.;
            let animation = transitions.play(
                &mut player,
                art.models[orc.variant].clips[index],
                std::time::Duration::from_millis(if index == 3 {
                    // The carried rock follows this exact pose from frame one.
                    0
                } else if is_attack_clip(index) {
                    60
                } else {
                    160
                }),
            );
            animation.replay();
            animation
                .set_repeat(if is_attack_clip(index) {
                    bevy::animation::RepeatAnimation::Never
                } else {
                    bevy::animation::RepeatAnimation::Forever
                })
                .set_speed(match index {
                    1 => WALK_SPEED_MULTIPLIER,
                    6 => 1.125 * WALK_SPEED_MULTIPLIER,
                    _ => 1.,
                });
        }
        // One clock for the carried rock and the authored lift, even if the
        // character scene finishes loading after the encounter has started.
        if orc.mode == 3 {
            let cell = (orc.home.xz() / CELL).floor().as_ivec2();
            let phase = *gate_openness().read().unwrap().get(&cell).unwrap_or(&0.);
            if let Some(active) = player.animation_mut(art.models[orc.variant].clips[3]) {
                // Animation advancement runs after Update. Freeze playback so
                // it cannot put the palm one render frame ahead of the rock.
                active
                    .set_speed(0.)
                    .set_seek_time(phase * STONE_PUSH_SECONDS);
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn cached_holes_match_nearest_sites_across_window_changes() {
    for distance in [48., 260., 1024., 48.] {
        for p in [
            Vec3::ZERO,
            Vec3::new(591., 30., 1344.),
            Vec3::new(-500., 0., -600.),
            Vec3::ZERO,
        ] {
            let expected: Vec<_> = sites(p, distance + 44.)
                .into_iter()
                .map(|(cell, s)| {
                    Vec4::new(
                        s.x,
                        s.z,
                        HOLE_RADIUS,
                        if den_kind(cell) == 2 {
                            -HOLE_LENGTH
                        } else {
                            HOLE_LENGTH
                        },
                    )
                })
                .collect();
            let actual = hole_uniforms(p, distance);
            assert_eq!(&actual[..expected.len()], expected);
            assert!(actual[expected.len()..].iter().all(|v| *v == Vec4::ZERO));
            assert_eq!(actual, hole_uniforms(p, distance));
        }
    }
}

#[cfg(test)]
mod pursuit_tests;

#[cfg(test)]
mod pursuit_world_tests;

#[test]
fn site_cache_preserves_accepted_and_rejected_regions_after_replacement() {
    for pass in 0..2 {
        for x in -32..32 {
            for z in -16..16 {
                let cell = if pass == 0 {
                    IVec2::new(x, z)
                } else {
                    IVec2::new(-x, -z)
                };
                assert_eq!(site(cell), uncached_site(cell));
                assert_eq!(site(cell), uncached_site(cell));
            }
        }
    }
}

#[cfg(test)]
mod camera_state_tests {
    use super::*;
    #[test]
    fn camera_state_tracks_nearby_gate_motion_and_open_flags() {
        let cell = IVec2::new(17001, -19003);
        let center = (cell.as_vec2() * CELL + Vec2::new(0.5, 20.))
            .extend(0.)
            .xzy();
        let neighbor = cell - IVec2::X;
        let before = camera_colliders(center, 2.5).unwrap();
        gate_openness().write().unwrap().insert(neighbor, 0.37);
        let moving = camera_colliders(center, 2.5).unwrap();
        assert!(before != moving);
        let far = cell + IVec2::splat(10);
        gate_openness().write().unwrap().insert(far, 0.51);
        assert!(moving == camera_colliders(center, 2.5).unwrap());
        open_gates().write().unwrap().insert(neighbor);
        assert!(moving != camera_colliders(center, 2.5).unwrap());
        gate_openness().write().unwrap().remove(&neighbor);
        gate_openness().write().unwrap().remove(&far);
        open_gates().write().unwrap().remove(&neighbor);
        assert!(before == camera_colliders(center, 2.5).unwrap());
        assert!(camera_colliders(center, CELL).is_none());
    }
}
