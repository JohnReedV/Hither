//! Aperiodic, world-space geology shared by the visible shell and excavation.
use bevy::prelude::*;
fn lattice(p: IVec3) -> f32 {
    let mut h = (p.x as u32).wrapping_mul(0x8da6b343)
        ^ (p.y as u32).wrapping_mul(0xd8163841)
        ^ (p.z as u32).wrapping_mul(0xcb1ab31f);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb352d);
    h ^= h >> 15;
    (h & 0xffffff) as f32 / 0xffffff as f32
}
pub(super) fn noise(p: Vec3) -> f32 {
    let cell = p.floor().as_ivec3();
    let f = p - p.floor();
    let u = f * f * (Vec3::splat(3.) - 2. * f);
    let mut value = 0.;
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let w = Vec3::new(
                    if x == 0 { 1. - u.x } else { u.x },
                    if y == 0 { 1. - u.y } else { u.y },
                    if z == 0 { 1. - u.z } else { u.z },
                );
                value += lattice(cell + IVec3::new(x, y, z)) * w.x * w.y * w.z;
            }
        }
    }
    value
}
pub(super) fn relief(p: Vec3, seed: u64) -> f32 {
    let p = p + Vec3::splat((seed % 8192) as f32);
    let warp = Vec3::new(
        noise(p * 0.10),
        noise(p * 0.10 + Vec3::splat(31.)),
        noise(p * 0.10 + Vec3::splat(67.)),
    ) * 2.;
    let p = p + warp;
    (noise(p * 0.19) - 0.5) * 2.6
        + (noise(p * 0.53) - 0.5) * 0.95
        + ((noise(p * 0.91) - 0.5).abs() - 0.25) * 0.5
}
pub(super) fn tint(p: Vec3) -> Vec3 {
    let broad = noise(p * 0.17);
    let damp = noise(p * Vec3::new(0.36, 0.09, 0.36) + Vec3::splat(51.));
    let iron = noise(p * 0.24 + Vec3::splat(11.));
    let pale = noise(p * 0.38 + Vec3::splat(87.));
    let mut c = Vec3::new(0.72, 0.71, 0.67).lerp(Vec3::new(1.30, 1.22, 1.06), broad);
    c = c.lerp(
        Vec3::new(0.70, 0.43, 0.24),
        ((iron - 0.55) * 2.8).clamp(0., 0.65),
    );
    c = c.lerp(
        Vec3::new(1.45, 1.40, 1.26),
        ((pale - 0.64) * 3.).clamp(0., 0.7),
    );
    c * (1. - ((damp - 0.48) * 1.0).clamp(0., 0.42))
}
// Tileable value noise for stone micropores; larger color variation is world-space.
fn noise2(p: Vec2, period: i32) -> f32 {
    noise_rect(p, IVec2::splat(period))
}
fn noise_rect(p: Vec2, period: IVec2) -> f32 {
    let cell = p.floor().as_ivec2();
    let f = p - p.floor();
    let u = f * f * (Vec2::splat(3.) - 2. * f);
    let sample = |x: i32, y: i32| {
        lattice(IVec3::new(
            (cell.x + x).rem_euclid(period.x),
            (cell.y + y).rem_euclid(period.y),
            971,
        ))
    };
    let a = sample(0, 0) * (1. - u.x) + sample(1, 0) * u.x;
    let b = sample(0, 1) * (1. - u.x) + sample(1, 1) * u.x;
    a * (1. - u.y) + b * u.y
}
pub(super) fn texture(p: Vec2) -> f32 {
    let uv = p / 256.;
    // Granular mineral pores at several scales, without a tiled polygon grid.
    [
        (4, 0.22),
        (8, 0.22),
        (16, 0.19),
        (32, 0.16),
        (64, 0.11),
        (128, 0.10),
    ]
    .into_iter()
    .map(|(period, weight)| noise2(uv * period as f32, period) * weight)
    .sum()
}

// Splintered longitudinal fibers, broad staining and knots; seamless at tile edges.
pub(super) fn wood_texture(p: Vec2) -> f32 {
    let uv = p / 256.;
    let warp = noise2(uv * 4., 4) * 0.10;
    let fiber = noise_rect(
        Vec2::new(uv.x * 64. + warp * 64., uv.y * 4.),
        IVec2::new(64, 4),
    );
    let fine = noise_rect(Vec2::new(uv.x * 128., uv.y * 8.), IVec2::new(128, 8));
    let stain = noise2(uv * 8., 8);
    let knot = noise2(uv * 4., 4);
    0.16 + fiber * 0.38 + fine * 0.16 + stain * 0.20 - ((knot - 0.65) * 3.).clamp(0., 0.22)
}
// Shared with the terrain portal shader: unequal hacked planes, not a pipe.
pub(super) fn burrow_profile(world: Vec3, seed: u64) -> Vec4 {
    let p = world + Vec3::splat((seed % 4096) as f32);
    Vec4::new(
        1.30 + noise(p * 0.12) * 0.95,
        1.30 + noise(p * 0.16 + Vec3::splat(31.)) * 0.90,
        1.40 + noise(p * 0.27 + Vec3::splat(87.)) * 0.80,
        (noise(p * 2.1) - 0.5) * 0.24 + (noise(p * 0.8) - 0.5) * 0.24,
    )
}
pub(super) fn burrow_field(v: Vec3, forward: Vec3, profile: Vec4) -> f32 {
    let x = v.dot(Vec3::Y.cross(forward));
    let sides = (-x - profile.x + v.y * 0.14).max(x - profile.y - v.y * 0.08);
    let roof = v.y + (x * 0.43).max(-x * 0.66) - profile.z;
    sides
        .max(roof)
        .max(-v.y - 1.68)
        .max(v.dot(forward).abs() - 2.25)
        + profile.w
}
