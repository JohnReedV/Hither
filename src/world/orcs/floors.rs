//! Den support adapter. Rendering, player physics and navigation consume the
//! same final triangles, including variant warps and the ramp/tunnel seam.
use super::*;
use crate::world::floor::MeshFloors;

pub(super) fn mesh(kind: usize) -> &'static MeshFloors {
    static MESHES: OnceLock<[MeshFloors; 3]> = OnceLock::new();
    &MESHES.get_or_init(|| {
        std::array::from_fn(|kind| {
            let mut surfaces = Vec::new();
            let (earth, _, _) = den_variant_geometry(kind, &mut Vec::new(), &mut surfaces);
            let mut mesh = MeshFloors::default();
            for (range, ceiling) in surfaces {
                for face in earth.indices[range].chunks_exact(3) {
                    mesh.insert(
                        std::array::from_fn(|i| {
                            Vec3::from_array(earth.positions[face[i] as usize])
                        }),
                        ceiling,
                    );
                }
            }
            mesh
        })
    })[kind]
}

/// All structural support layers at a world XZ location. There is no guessed
/// eye height or surface/underground altitude switch in this enumeration.
pub(super) fn levels(at: Vec2, radius: f32) -> Option<Vec<f32>> {
    let cell = (at / CELL).floor().as_ivec2();
    let home = site(cell)?;
    let q = at - home.xz();
    if q.length_squared() > 23. * 23. {
        return None;
    }
    let kind = den_kind(cell);
    let mut levels: Vec<_> = mesh(kind).rounded_support(q, radius).into_iter().collect();
    let eye = Vec3::new(at.x, crate::player::movement::PLAYER_EYE_HEIGHT, at.y);
    let entrance = entrance_overlap(eye, radius);
    let opening = opening(cell, q, radius);
    if !entrance {
        levels.push(crate::world::orchard::oak::support_height(eye, radius));
    } else if !opening {
        levels.push(GRATE_HEIGHT);
    }
    if kind == 1 {
        for z in [-3.8, -2.6, -1.4, -0.2] {
            let d = (q.x / 2.65).powi(2) + ((q.y - z) / 1.25).powi(2);
            if d < 1. {
                levels.push(0.85 + 0.65 * (1. - d).sqrt());
            }
        }
    }
    let phase = *gate_openness().read().unwrap().get(&cell).unwrap_or(&0.);
    if let Some(top) = moving_gate_top(Vec3::new(q.x, 0., q.y), radius, kind, phase) {
        levels.push(if !entrance {
            top.max(0.)
        } else if !opening {
            top.max(GRATE_HEIGHT)
        } else {
            top
        });
    }
    levels.sort_by(f32::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    Some(levels)
}

pub(super) fn entrance(kind: usize, at: Vec2, radius: f32) -> bool {
    let rounding = if kind == 2 { 0.4 } else { 0.6 };
    let q = at.abs() - Vec2::new(HOLE_RADIUS, HOLE_LENGTH) + Vec2::splat(rounding);
    q.max(Vec2::ZERO).length() + q.max_element().min(0.) < rounding + radius
}
fn aperture(kind: usize, at: Vec2, radius: f32) -> bool {
    let width = ramp_half_width(kind, at.y);
    at.x.abs() < width - radius && (-4. ..=4.).contains(&at.y)
}
pub(super) fn opening(cell: IVec2, at: Vec2, radius: f32) -> bool {
    gate_is_open(cell) && aperture(den_kind(cell), at, radius)
}
/// Static volume contract used by actors and floor-region certification. Door
/// motion is an additional live obstacle, not an alternate floor/wall recipe.
pub(super) fn body_blocks(feet: Vec3, radius: f32, kind: usize, open: bool) -> bool {
    if collision::barriers(kind).blocks(feet, radius) {
        return true;
    }
    if entrance(kind, feet.xz(), radius)
        && !(open && aperture(kind, feet.xz(), radius))
        && feet.y < GRATE_HEIGHT - 0.001
        && feet.y + collision::BODY_HEIGHT > GRATE_HEIGHT
    {
        return true;
    }
    feet.y < -0.15 && !contains_body(feet, radius, kind)
}

/// A footprint must remain over the continuous floor, including at the rounded
/// tunnel wall. A small rise is allowed for ordinary steps; a missing floor is
/// solid outside the authored underground volume, not a fall-through escape.
pub(super) fn contains_body(feet: Vec3, radius: f32, kind: usize) -> bool {
    if feet.y < -0.15 {
        if (-4. ..=4.).contains(&feet.z) && feet.x.abs() >= ramp_half_width(kind, feet.z) - radius {
            return false;
        }
        if kind == 2
            && (-6. ..-4.).contains(&feet.z)
            && feet.x.abs() > burrow_width(feet.z) - 0.12 - radius
        {
            return false;
        }
    }
    let Some(floor) = mesh(kind).rounded_support(feet.xz(), radius) else {
        return false;
    };
    feet.y >= floor - 0.201
        && crate::world::floor::footprint(feet.xz(), radius)
            .all(|at| (at.y > 4. && feet.y >= -0.201) || mesh(kind).floor(at).is_some())
        && mesh(kind)
            .ceiling(feet.with_y(floor), radius)
            .is_none_or(|top| feet.y + collision::BODY_HEIGHT <= top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::navigation::{Agent, Geometry as _};

    #[test]
    fn camera_sphere_has_clear_views_beneath_each_tunnel_ceiling() {
        let mut checked = 0;
        for kind in 0..3 {
            let home = (100..500)
                .find_map(|x| {
                    let cell = IVec2::new(x, 186);
                    site(cell).filter(|_| den_kind(cell) == kind)
                })
                .unwrap();
            for z in [-6., -8., -10., -12.] {
                let at = Vec2::new(tunnel_center(z).x, z);
                let (floor, ceiling) = visible_tunnel_bounds(at, kind).unwrap();
                let camera = home + Vec3::new(at.x, ceiling - 0.45, at.y);
                if collision::barriers(kind).blocks_sized(
                    camera - home - Vec3::Y * 0.16,
                    0.16,
                    0.32,
                ) {
                    continue;
                }
                assert!(camera.y - home.y > floor + 0.16);
                assert!(
                    camera_clear(camera, 0.16),
                    "camera incorrectly treated as standing body: kind={kind} z={z}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 9);
    }
    #[test]
    fn solid_earth_above_tunnel_is_blocked_and_embedded_bodies_recover() {
        for kind in 0..3 {
            let home = (100..500)
                .find_map(|x| {
                    let cell = IVec2::new(x, 185);
                    site(cell).filter(|_| den_kind(cell) == kind)
                })
                .unwrap();
            for z in [-6., -9., -12.] {
                let at = Vec2::new(tunnel_center(z).x, z);
                let eye =
                    home + Vec3::new(at.x, crate::player::movement::PLAYER_EYE_HEIGHT - 0.3, at.y);
                assert!(
                    crate::player::movement::character_collides(
                        eye,
                        crate::player::movement::PLAYER_RADIUS
                    ),
                    "solid overburden kind={kind} z={z}"
                );
                let recovered =
                    recover_player_position(eye, crate::player::movement::PLAYER_RADIUS)
                        .expect("embedded body must recover into open volume");
                assert!(!crate::player::movement::character_collides(
                    recovered,
                    crate::player::movement::PLAYER_RADIUS
                ));
                assert!(recovered.y - crate::player::movement::PLAYER_EYE_HEIGHT < -1.);
            }
        }
    }
    #[test]
    fn every_rendered_floor_triangle_has_matching_physical_support() {
        let mut checked = 0;
        for kind in 0..3 {
            let mut surfaces = Vec::new();
            let (earth, _, _) = den_variant_geometry(kind, &mut Vec::new(), &mut surfaces);
            for (range, ceiling) in surfaces {
                for face in earth.indices[range].chunks_exact(3) {
                    let [a, b, c] = std::array::from_fn(|i| {
                        Vec3::from_array(earth.positions[face[i] as usize])
                    });
                    // Independent interpolation of rendered vertices, including
                    // off-grid interiors and shared edges after all deformation.
                    for (u, v) in [(0.2, 0.3), (0.6, 0.1), (0.5, 0.5), (0., 0.)] {
                        let expected = a + (b - a) * u + (c - a) * v;
                        assert!(
                            mesh(kind)
                                .heights(expected.xz(), ceiling)
                                .iter()
                                .any(|y| (*y - expected.y).abs() < 0.002),
                            "missing rendered surface: kind={kind} ceiling={ceiling} at={expected:?} vertices={a:?},{b:?},{c:?} hits={:?}",
                            mesh(kind).heights(expected.xz(), ceiling)
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 40_000);
    }

    #[test]
    fn players_and_navigation_share_visible_floors_across_every_den() {
        let mut checked = 0;
        for kind in 0..3 {
            let (cell, home) = (100..500)
                .find_map(|x| {
                    let cell = IVec2::new(x, 184);
                    site(cell)
                        .filter(|_| den_kind(cell) == kind)
                        .map(|home| (cell, home))
                })
                .unwrap();
            open_gates().write().unwrap().insert(cell);
            gate_openness().write().unwrap().insert(cell, 1.);
            for zi in 0..190 {
                let z = 3.9 - zi as f32 * 0.095;
                for xi in -15..=15 {
                    let x = tunnel_center(z).x + xi as f32 * 0.1;
                    // Query the same representable world position as actors.
                    let local = (home.xz() + Vec2::new(x, z)) - home.xz();
                    let (x, z) = (local.x, local.y);
                    let Some(y) = mesh(kind).floor(Vec2::new(x, z)) else {
                        continue;
                    };
                    let physical = mesh(kind)
                        .rounded_support(Vec2::new(x, z), crate::player::movement::PLAYER_RADIUS)
                        .unwrap();
                    assert!(physical >= y - 0.001);
                    let feet = home + Vec3::new(x, physical, z);
                    if !navigation::World.body_clear(feet, Agent::default()) {
                        continue;
                    }
                    let player = surface_height(
                        feet + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
                        crate::player::movement::PLAYER_RADIUS,
                    )
                    .unwrap();
                    assert!(
                        (player - physical).abs() < 0.004,
                        "player/render mismatch kind={kind} at={feet:?} floor={player} expected={y}"
                    );
                    assert!(
                        navigation::World
                            .floors(feet.xz(), Agent::default())
                            .iter()
                            .any(|level| (*level - physical).abs() < 0.004)
                    );
                    // Falling from inside the passage must hit this same floor,
                    // rather than the world surface above the tunnel.
                    let mut falling = feet + Vec3::Y * 0.1;
                    if !navigation::World.body_clear(falling, Agent::default()) {
                        continue;
                    }
                    let mut velocity = -50.;
                    let grounded = crate::world::navigation::settle(
                        &navigation::World,
                        Agent::default(),
                        &mut falling,
                        &mut velocity,
                        0.05,
                    );
                    assert!(
                        grounded && (falling.y - physical).abs() < 0.004,
                        "fall missed mesh: kind={kind} feet={feet:?} landed={falling:?}"
                    );
                    checked += 1;
                }
            }
            open_gates().write().unwrap().remove(&cell);
            gate_openness().write().unwrap().remove(&cell);
        }
        assert!(checked > 4000, "only tested {checked} positions");
    }
}
