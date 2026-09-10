use super::*;

pub(super) fn add_shape(g: &mut Geometry, mesh: Mesh, t: Transform, color: Vec3) {
    let mut shape = crate::rendering::geometry::primitive(mesh);
    shape.colors.fill(color.extend(1.).to_array());
    crate::rendering::geometry::append(g, &shape, t);
}
pub(super) fn beam(g: &mut Geometry, a: Vec3, b: Vec3, r: f32, color: Vec3) {
    add_shape(
        g,
        Cylinder::new(r, a.distance(b))
            .mesh()
            .resolution(12)
            .build(),
        Transform::from_translation((a + b) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize())),
        color,
    );
}
pub(super) fn earth_quad(g: &mut Geometry, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: Vec3) {
    let normal = (b - a).cross(c - a).abs();
    let uv = |p: Vec3| {
        if normal.y >= normal.x && normal.y >= normal.z {
            p.xz() * 1.4
        } else if normal.x > normal.z {
            Vec2::new(p.z, p.y) * 1.4
        } else {
            p.xy() * 1.4
        }
    };
    let a = g.vertex(a, uv(a), color);
    let b = g.vertex(b, uv(b), color);
    let c = g.vertex(c, uv(c), color);
    let d = g.vertex(d, uv(d), color);
    g.triangle(a, b, c);
    g.triangle(a, c, d);
}
// The lower half uses the ramp's exact 20-column boundary. Sharing the
// tessellation as well as the height avoids cracks between curved surfaces.
pub(super) fn tunnel_ring(p: Vec3, index: usize) -> Vec3 {
    let rough = ((-4. - p.z) / 1.2).clamp(0., 1.);
    let (t, x) = if index <= 16 {
        let t = index as f32 * PI / 16.;
        (t, 2.2 * t.cos())
    } else {
        let x = -2.2 + (index - 16) as f32 * 0.22;
        (TAU - (x / 2.2).clamp(-1., 1.).acos(), x)
    };
    if p.z == -4. && (index == 0 || index >= 16) {
        return Vec3::new(x, ramp_height(x, -4.), -4.);
    }
    let mut point = p + Vec3::new(
        x + rough
            * t.cos()
            * (0.16 * (p.z * 2.3 + t * 5.).sin() + 0.10 * (p.z * 4.7 - t * 9.).sin()),
        1.35 + t.sin().max(0.) * (2.05 + rough * 0.32 * (p.z * 1.7 + t * 6.).sin())
            + t.sin().min(0.) * (1.35 + rough * 0.08 * (p.z * 2.1).sin()),
        0.,
    );
    if index >= 16 {
        point.y = (p.y + ramp_height(x, -4.) + 5.8) * (1. - rough) + point.y * rough;
    }
    point
}

pub(super) fn add_obstacle(
    g: &mut Geometry,
    obstacles: &mut Vec<std::ops::Range<usize>>,
    mesh: Mesh,
    t: Transform,
    color: Vec3,
) {
    let first = g.positions.len();
    add_shape(g, mesh, t, color);
    obstacles.push(first..g.positions.len());
}
pub(super) fn den_geometry_for(
    natural: bool,
    obstacles: &mut Vec<std::ops::Range<usize>>,
    surfaces: &mut Vec<(std::ops::Range<usize>, bool)>,
) -> (Geometry, Geometry, Geometry) {
    let (mut earth, mut grate, mut hatch) = (
        Geometry::default(),
        Geometry::default(),
        Geometry::default(),
    );
    let mut rng = Rng(781321);
    // Eight-metre descending ramp, nearly six metres below grade at its back.
    // Separate walls and an arched continuation leave real room for full-size actors.
    let floor = |t: f32, x: f32| Vec3::new(x, ramp_height(x, 4. - 8. * t), 4. - 8. * t);
    let lip = |t: f32, side: f32| {
        Vec3::new(
            side * (2.2 + 0.15 * (t * 19. + side).sin().powi(2)),
            0.045 + 0.10 * (t * 13. + side).sin().powi(2),
            4. - 8. * t,
        )
    };
    for j in 0..40 {
        let t = j as f32 / 40.;
        let u = (j + 1) as f32 / 40.;
        let tint = Vec3::new(0.72, 0.66, 0.55) * (1. - t * 0.65);
        for i in 0..20 {
            let x = -2.2 + i as f32 * 0.22;
            let y = -2.2 + (i + 1) as f32 * 0.22;
            let start = earth.indices.len();
            earth_quad(
                &mut earth,
                floor(t, x),
                floor(t, y),
                floor(u, y),
                floor(u, x),
                tint,
            );
            surfaces.push((start..earth.indices.len(), false));
        }
        for side in [-1., 1.] {
            for k in 0..10 {
                let v = k as f32 / 10.;
                let w = (k + 1) as f32 / 10.;
                let wall = |t: f32, v: f32| {
                    let bottom = floor(t, side * 2.2);
                    Vec3::new(
                        side * (2.2
                            + 0.15 * (t * 19. + side).sin().powi(2) * v
                            + (0.65 * (t * 14. + v * 9.).sin() + 0.20 * (t * 39. - v * 21.).cos())
                                * v
                                * (1. - v)),
                        bottom.y * (1. - v)
                            + lip(t, side).y * v
                            + 0.10 * (t * 23. + v * 17.).sin() * v * (1. - v),
                        bottom.z,
                    )
                };
                earth_quad(
                    &mut earth,
                    wall(t, v),
                    wall(u, v),
                    wall(u, w),
                    wall(t, w),
                    tint,
                );
            }
            earth_quad(
                &mut earth,
                lip(t, side) - Vec3::X * side * 0.03,
                lip(u, side) - Vec3::X * side * 0.03,
                Vec3::new(side * (2.45 + 0.08 * (u * 61.).sin()), -0.025, 4. - 8. * u),
                Vec3::new(side * (2.45 + 0.08 * (t * 61.).sin()), -0.025, 4. - 8. * t),
                tint,
            );
        }
    }
    let centers = [
        Vec3::new(0., -5.8, -4.),
        Vec3::new(0., -6., -7.),
        Vec3::new(1., -6.1, -10.),
        Vec3::new(3., -6.2, -13.),
        Vec3::new(6., -6.2, -15.),
    ];
    for segment in 0..4 {
        for j in 0..8 {
            let a = tunnel_center(centers[segment].lerp(centers[segment + 1], j as f32 / 8.).z);
            let b = tunnel_center(
                centers[segment]
                    .lerp(centers[segment + 1], (j + 1) as f32 / 8.)
                    .z,
            );
            let shade_at = |p: Vec3| {
                let fade = ((p.z + 14.) / 10.).clamp(0., 1.);
                let shade = 0.22 * fade * fade;
                Vec3::new(shade, shade * 0.87, shade * 0.70)
            };
            for k in 0..36 {
                let start = earth.indices.len();
                earth_quad(
                    &mut earth,
                    tunnel_ring(a, k),
                    tunnel_ring(b, k),
                    tunnel_ring(b, (k + 1) % 36),
                    tunnel_ring(a, (k + 1) % 36),
                    shade_at(a),
                );
                surfaces.push((start..earth.indices.len(), k < 16));
                let n = earth.colors.len();
                earth.colors[n - 3] = shade_at(b).extend(1.).to_array();
                earth.colors[n - 2] = shade_at(b).extend(1.).to_array();
            }
        }
    }
    // Seal the formerly open final ring beyond the fully black bend.
    earth_quad(
        &mut earth,
        Vec3::new(3.4, -6.6, -15.),
        Vec3::new(8.6, -6.6, -15.),
        Vec3::new(8.6, -2.4, -15.),
        Vec3::new(3.4, -2.4, -15.),
        Vec3::ZERO,
    );
    // Earth above the rear arch closes the surface, not the passage.
    for i in 0..16 {
        let a = i as f32 * PI / 16.;
        let b = (i + 1) as f32 * PI / 16.;
        for row in 0..12 {
            let v = row as f32 / 12.;
            let w = (row + 1) as f32 / 12.;
            let bank = |a: f32, v: f32| {
                Vec3::new(
                    2.2 * a.cos(),
                    (-5.8 + 3.4 * a.sin()) * (1. - v) + 0.15 * v,
                    -4. + (0.35 * (a * 7. + v * 11.).sin() + 0.16 * (a * 19. - v * 23.).cos())
                        * v
                        * (1. - v),
                )
            };
            earth_quad(
                &mut earth,
                bank(a, v),
                bank(b, v),
                bank(b, w),
                bank(a, w),
                Vec3::new(0.22, 0.18, 0.13),
            );
        }
    }
    // Overlapping earth shoulders seal the independently excavated side banks.
    for side in [-1., 1.] {
        earth_quad(
            &mut earth,
            Vec3::new(side * 1.95, -5.9, -4.01),
            Vec3::new(side * 2.7, -5.9, -4.01),
            Vec3::new(side * 2.7, 0.2, -4.01),
            Vec3::new(side * 1.95, 0.2, -4.01),
            Vec3::new(0.22, 0.18, 0.13),
        );
    }
    earth.finish_normals();
    // Large embedded stones and smaller scree, never identical masonry blocks.
    for i in 0..110 {
        let z = rng.range(-7., 4.);
        let side = if i % 2 == 0 { -1. } else { 1. };
        let y = if z >= -4. { (4. - z) / 8. * -5.8 } else { -5.9 };
        let on_rim = i < 30;
        let p = if on_rim {
            Vec3::new(
                side * rng.range(2.12, 2.42),
                rng.range(-0.10, 0.08),
                z.max(-4.),
            )
        } else {
            Vec3::new(side * rng.range(1.3, 2.23), y + rng.range(0., 0.12), z)
        };
        let size = if on_rim {
            rng.range(0.10, 0.34)
        } else {
            rng.range(0.04, 0.22)
        };
        add_obstacle(
            &mut earth,
            obstacles,
            Sphere::new(1.).mesh().ico(1).unwrap(),
            Transform::from_translation(p)
                .with_scale(Vec3::new(size, size * rng.range(0.6, 1.5), size * 1.4))
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.unit() * PI,
                    rng.unit() * PI,
                    rng.unit() * PI,
                )),
            Vec3::new(0.65, 0.67, 0.68) * if on_rim { 1. } else { 0.38 },
        );
    }
    // Hanging torn roots branch out from the cut banks.
    for i in 0..28 {
        let side = if i % 2 == 0 { -1. } else { 1. };
        let z = rng.range(-3.8, 3.2);
        let a = Vec3::new(side * 2.23, 0.02, z);
        let b = a + Vec3::new(-side * 0.18, -rng.range(0.22, 0.65), 0.12);
        let c = b + Vec3::new(-side * 0.12, -0.22, -0.16);
        beam(&mut earth, a, b, 0.03, Vec3::new(0.34, 0.22, 0.10));
        beam(&mut earth, b, c, 0.017, Vec3::new(0.25, 0.17, 0.09));
        beam(
            &mut earth,
            b,
            b + Vec3::new(side * 0.09, -0.24, 0.21),
            0.008,
            Vec3::new(0.22, 0.14, 0.07),
        );
    }
    // Rough timber shoring, iron straps and scavenged supplies down the ramp.
    for z in [-2.5, -5.5, -8.].into_iter().filter(|_| !natural) {
        let y = if z >= -4. { (4. - z) / 8. * -5.8 } else { -6. };
        for side in [-1., 1.] {
            let p = Vec3::new(side * 1.92, y, z);
            let first = earth.positions.len();
            beam(
                &mut earth,
                p,
                p + Vec3::Y * 2.9,
                0.13,
                Vec3::new(0.22, 0.14, 0.07),
            );
            obstacles.push(first..earth.positions.len());
            for h in [0.4, 2.3] {
                add_shape(
                    &mut grate,
                    Cuboid::new(0.3, 0.10, 0.30).mesh().build(),
                    Transform::from_translation(p + Vec3::Y * h),
                    Vec3::splat(0.18),
                );
            }
        }
        beam(
            &mut earth,
            Vec3::new(-2.02, y + 2.9, z),
            Vec3::new(2.02, y + 3.02, z),
            0.15,
            Vec3::new(0.22, 0.15, 0.07),
        );
    }
    for i in 0..7 {
        let p = Vec3::new(
            if i % 2 == 0 { -1.5 } else { 1.5 },
            -5.55,
            -4.5 - i as f32 * 0.45,
        );
        add_obstacle(
            &mut earth,
            obstacles,
            Cuboid::new(0.45, 0.5, 0.42).mesh().build(),
            Transform::from_translation(p).with_rotation(Quat::from_rotation_y(i as f32)),
            Vec3::new(0.20, 0.15, 0.085),
        );
        beam(
            &mut grate,
            p + Vec3::new(-0.18, 0.26, 0.),
            p + Vec3::new(0.18, 0.26, 0.),
            0.015,
            Vec3::splat(0.16),
        );
    }
    for _ in 0..24 {
        let p = Vec3::new(rng.range(-1.7, 1.7), -5.74, rng.range(-6.3, -4.3));
        beam(
            &mut earth,
            p,
            p + Vec3::new(rng.range(-0.2, 0.2), 0.015, rng.range(-0.15, 0.15)),
            0.023,
            Vec3::new(0.43, 0.40, 0.30),
        );
    }
    // The entire ramp lid opens. A fixed grille above either walking lane
    // would trap an ascending full-height body before it reached the lip.
    let rust = Vec3::splat(0.6);
    for i in -11_i32..=11 {
        let x = i as f32 * 0.2;
        beam(
            &mut hatch,
            Vec3::new(x + 2.2, 0., -4.),
            Vec3::new(x + 2.2, 0., 4.),
            0.028,
            rust,
        );
    }
    for z in [-4., -2., 0., 2., 4.] {
        beam(
            &mut hatch,
            Vec3::new(0., 0., z),
            Vec3::new(4.4, 0., z),
            0.04,
            rust,
        );
    }
    for x in [0., 4.4] {
        beam(
            &mut hatch,
            Vec3::new(x, 0., -4.),
            Vec3::new(x, 0., 4.),
            0.045,
            rust,
        );
    }
    for z in [-3., 0., 3.] {
        beam(
            &mut grate,
            Vec3::new(-2.2, 0.1, z - 0.13),
            Vec3::new(-2.2, 0.1, z + 0.13),
            0.07,
            rust,
        );
        add_shape(
            &mut hatch,
            Cuboid::new(0.16, 0.14, 0.20).mesh().build(),
            Transform::from_xyz(4.32, -0.11, z),
            rust * 0.6,
        );
    }
    for i in 0..28 {
        add_shape(
            &mut grate,
            Torus::new(0.025, 0.044)
                .mesh()
                .major_resolution(10)
                .minor_resolution(6)
                .build(),
            Transform::from_xyz(1.75, -1.2 - i as f32 * 0.10, -2.6).with_rotation(
                Quat::from_rotation_x(if i % 2 == 0 { PI * 0.5 } else { 0. }),
            ),
            rust * 0.28,
        );
    }
    (earth, grate, hatch)
}
#[cfg(test)]
pub(super) fn den_geometry() -> (Geometry, Geometry, Geometry) {
    den_variant(0)
}
// All three retain the same excavated, navigable envelope. Dressing stays at
// the banks, leaving the central ramp and its collision surface unobstructed.
pub(super) fn den_variant(kind: usize) -> (Geometry, Geometry, Geometry) {
    den_variant_geometry(kind, &mut Vec::new(), &mut Vec::new())
}
pub(super) fn den_variant_geometry(
    kind: usize,
    obstacles: &mut Vec<std::ops::Range<usize>>,
    surfaces: &mut Vec<(std::ops::Range<usize>, bool)>,
) -> (Geometry, Geometry, Geometry) {
    let (mut earth, mut metal, mut door) = den_geometry_for(kind == 2, obstacles, surfaces);

    let mut rng = Rng(98171 + kind as u64 * 771);
    if kind == 1 {
        // Stonejaw: cool shale banks and a single thick, cracked stone lid.
        for c in &mut earth.colors {
            c[0] *= 0.68;
            c[1] *= 0.83;
            c[2] *= 1.08;
        }
        // A volumetric, irregular boulder, not a thin rectangular hatch.
        door = Geometry::default();
        metal = Geometry::default(); // No fixed grille across the quarry mouth.
        let grey = Vec3::new(0.46, 0.48, 0.43);
        add_shape(
            &mut door,
            Sphere::new(1.).mesh().ico(3).unwrap(),
            Transform::from_scale(Vec3::new(1.75, 1.45, 1.85)),
            grey,
        );
        for p in &mut door.positions {
            // Broad fracture planes create a quarried rock silhouette rather
            // than a scaled ball. Small chips weather those planar breaks.
            p[0] *= 1. + 0.09 * (p[2] * 2.1 + p[1]).sin();
            p[2] *= 1. + 0.07 * (p[0] * 2.7 - p[1]).cos();
            let chip = 0.08 * (p[0] * 4.1 + p[2] * 2.7).sin() * (p[2] * 3.3 - p[1] * 2.2).cos();
            // A small planar bearing face rests on the open carrying palm.
            p[1] = if p[1] < -1.30 {
                STONE_BEARING_Y
            } else {
                (p[1] + chip).max(STONE_BEARING_Y)
            };
        }
        for (p, c) in door.positions.iter().zip(&mut door.colors) {
            let strata = (p[1] * 13. + p[0] * 0.9 + (p[2] * 1.7).sin()).sin();
            let fissure = (p[0] * 2.7 + p[2] * 1.1 + (p[2] * 2.).sin() * 0.5)
                .sin()
                .abs();
            let shade = (0.8 + strata * 0.14) * if fissure < 0.12 { 0.52 } else { 1. };
            c[0] *= shade;
            c[1] *= shade;
            c[2] *= shade;
        }
        door.finish_normals();
        // Fixed rock over the rear excavation leaves a compact front mouth.
        for z in [-3.8, -2.6, -1.4, -0.2] {
            add_obstacle(
                &mut earth,
                obstacles,
                Sphere::new(1.).mesh().ico(2).unwrap(),
                Transform::from_xyz(0., 0.85, z).with_scale(Vec3::new(2.65, 0.65, 1.25)),
                grey,
            );
        }
        for side in [-1., 1.] {
            for i in 0..18 {
                let z = 3.6 - i as f32 * 0.55;
                let y = if z > -4. {
                    ramp_height(side * 1.9, z)
                } else {
                    -5.9
                };
                add_obstacle(
                    &mut earth,
                    obstacles,
                    Sphere::new(1.).mesh().ico(1).unwrap(),
                    Transform::from_xyz(side * 2.0, y + 0.45, z)
                        .with_scale(Vec3::new(0.31, rng.range(0.45, 0.85), 0.46))
                        .with_rotation(Quat::from_rotation_z(side * 0.2)),
                    grey * rng.range(0.7, 1.2),
                );
                if i % 3 == 0 {
                    beam(
                        &mut metal,
                        Vec3::new(side * 1.81, y + 0.5, z),
                        Vec3::new(side * 1.8, y + 1.05, z - 0.25),
                        0.022,
                        Vec3::new(0.27, 0.34, 0.30),
                    );
                }
            }
            // Discarded picks and ore shelves exposed when the stone moves.
            for i in 0..4 {
                let z = -4.5 - i as f32 * 0.7;
                beam(
                    &mut earth,
                    Vec3::new(side * 1.5, -5.8, z),
                    Vec3::new(side * 1.65, -4.65, z),
                    0.04,
                    Vec3::new(0.22, 0.13, 0.06),
                );
                beam(
                    &mut metal,
                    Vec3::new(side * 1.4, -4.65, z),
                    Vec3::new(side * 1.85, -4.62, z),
                    0.035,
                    Vec3::splat(0.4),
                );
                add_obstacle(
                    &mut earth,
                    obstacles,
                    Sphere::new(1.).mesh().ico(1).unwrap(),
                    Transform::from_xyz(side * 1.5, -5.8, z + 0.25)
                        .with_scale(Vec3::new(0.23, 0.19, 0.24)),
                    grey * 0.6,
                );
            }
        }
    } else if kind == 2 {
        // Rootwarren is an uncovered animal-like excavation, not architecture.
        metal = Geometry::default();
        door = Geometry::default();
        // A stitched rawhide core trapped in a basket of twisted roots. This
        // is a buried removable cork, with a deep draw-rope and side pocket,
        // rather than a second metal hatch or an above-ground hut.
        for i in 0..9 {
            let a = i as f32 * 2.39996;
            let r = (i as f32 / 9.).sqrt();
            add_shape(
                &mut door,
                Sphere::new(1.).mesh().ico(2).unwrap(),
                Transform::from_xyz(a.cos() * r * 1.05, a.sin() * r * 0.85, -0.05)
                    .with_scale(Vec3::new(0.75, 0.67, 0.20))
                    .with_rotation(Quat::from_rotation_z(a)),
                Vec3::new(0.30, 0.16, 0.065) * rng.range(0.75, 1.25),
            );
        }
        for ring in 0..7 {
            let r = 0.25 + ring as f32 * 0.125;
            for j in 0..40 {
                let point = |j: usize| {
                    let a = j as f32 * TAU / 40.;
                    Vec3::new(
                        1.75 * r * a.cos(),
                        1.40 * r * a.sin(),
                        0.22 + 0.06 * (a * 7. + ring as f32).sin(),
                    )
                };
                beam(
                    &mut door,
                    point(j),
                    point(j + 1),
                    0.028 + 0.008 * (ring % 2) as f32,
                    Vec3::new(0.22, 0.115, 0.045),
                );
            }
        }
        for i in 0..13 {
            let a = i as f32 * TAU / 13.;
            let end = Vec3::new(1.72 * a.cos(), 1.37 * a.sin(), 0.17);
            beam(
                &mut door,
                Vec3::new(0., 0., 0.30),
                end,
                0.035,
                Vec3::new(0.36, 0.22, 0.09),
            );
            // Bone pegs lash the hide to the wicker, with unequal worn tips.
            let p = end * 0.80;
            beam(
                &mut door,
                p,
                p + Vec3::new(0.035, 0.11, 0.13),
                0.023,
                Vec3::new(0.65, 0.54, 0.34),
            );
        }
        for side in [-1., 1.] {
            beam(
                &mut door,
                Vec3::new(side * 0.6, 0., -0.1),
                Vec3::new(side * 0.6, -0.25, -1.9),
                0.027,
                Vec3::new(0.28, 0.20, 0.11),
            );
        }
        for side in [-1., 1.] {
            for i in 0..22 {
                let z = 3.8 - i as f32 * 0.49;
                let floor = if z > -4. {
                    ramp_height(side * 1.85, z)
                } else {
                    -5.9
                };
                // Roots emerge from eroded soil and branch down the cut bank.
                let top = Vec3::new(side * rng.range(2.05, 2.25), -0.08, z);
                let bottom = Vec3::new(side * rng.range(1.85, 2.05), floor + 0.35, z - 0.17);
                let mid = top.lerp(bottom, 0.5) + Vec3::Z * rng.range(-0.3, 0.3);
                let wood = Vec3::new(0.20, 0.105, 0.045);
                beam(&mut earth, top, mid, rng.range(0.025, 0.065), wood);
                beam(&mut earth, mid, bottom, 0.018, wood);
                for j in 0..3 {
                    let p = mid.lerp(bottom, j as f32 / 3.);
                    beam(
                        &mut earth,
                        p,
                        p + Vec3::new(-side * 0.12, -0.3, 0.22),
                        0.009,
                        wood,
                    );
                }
                // Small ochre shelf fungi, bank cavities and half-buried bones.
                if i % 3 == 0 {
                    let p = Vec3::new(side * 1.96, floor + rng.range(0.3, 0.75), z);
                    add_obstacle(
                        &mut earth,
                        obstacles,
                        Sphere::new(1.).mesh().ico(2).unwrap(),
                        Transform::from_translation(p).with_scale(Vec3::new(0.22, 0.055, 0.17)),
                        Vec3::new(0.57, 0.28, 0.09),
                    );
                }
                if i % 4 == 0 {
                    let p = Vec3::new(side * 1.65, floor + 0.08, z);
                    beam(
                        &mut earth,
                        p,
                        p + Vec3::new(-side * 0.16, 0.08, 0.32),
                        0.025,
                        Vec3::new(0.53, 0.45, 0.29),
                    );
                }
            }
        }
        // Spoil remains at the lip in irregular low heaps; nothing roofs over
        // the opening. All tall dressing is below ground.
        for i in 0..16 {
            let side = if i % 2 == 0 { -1. } else { 1. };
            add_obstacle(
                &mut earth,
                obstacles,
                Sphere::new(1.).mesh().ico(2).unwrap(),
                Transform::from_xyz(side * rng.range(2.2, 2.45), -0.16, rng.range(-3.9, 3.8))
                    .with_scale(Vec3::new(
                        rng.range(0.22, 0.45),
                        rng.range(0.18, 0.32),
                        rng.range(0.3, 0.65),
                    )),
                Vec3::new(0.37, 0.24, 0.12),
            );
        }
    }
    if kind == 2 {
        for p in &mut earth.positions {
            if (-6.0..=4.0).contains(&p[2]) {
                p[0] *= burrow_width(p[2]) / 2.2;
            }
        }
        earth.finish_normals();
    }
    // Reserve a continuous 3.3 m exit corridor plus the stone lifter's stance.
    // Move the authored rocks into the bank, so their visible positions and
    // barriers agree instead of making a collider exception for the script.
    for range in obstacles.iter().cloned() {
        let (low, high) = earth.positions[range.clone()].iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(low, high), p| {
                (
                    low.min(Vec3::from_array(*p)),
                    high.max(Vec3::from_array(*p)),
                )
            },
        );
        if low.x < 0. && high.x > 0. {
            continue;
        } // Overhead roof, not a bank prop.
        let side = (low.x + high.x).signum();
        let clear = if kind == 1 && side < 0. && low.z < 2.95 && high.z > 2.25 {
            2.04
        } else {
            if low.z < 4.5 && high.z > -7.5 {
                1.65
            } else {
                0.50
            }
        };
        let inner = if side < 0. { -high.x } else { low.x };
        if inner < clear {
            for p in &mut earth.positions[range] {
                p[0] += side * (clear - inner);
            }
        }
    }
    (earth, metal, door)
}
pub(super) fn soil_texel(x: u32, y: u32) -> (Vec3, f32) {
    let hash = |x: u32, y: u32| {
        (mix(u64::from(x.wrapping_mul(7411) ^ y.wrapping_mul(9127))) & 65535) as f32 / 65535.
    };
    let noise = |scale: u32| {
        let a = x / scale;
        let b = y / scale;
        let u = (x % scale) as f32 / scale as f32;
        let v = (y % scale) as f32 / scale as f32;
        let u = u * u * (3. - 2. * u);
        let v = v * v * (3. - 2. * v);
        (hash(a, b) * (1. - u) + hash(a + 1, b) * u) * (1. - v)
            + (hash(a, b + 1) * (1. - u) + hash(a + 1, b + 1) * u) * v
    };
    let clods = noise(43) * 0.45 + noise(13) * 0.32 + noise(4) * 0.23;
    let grain = hash(x, y);
    let mut height = clods * 0.35 + grain * 0.08;
    let mut color = Vec3::new(0.14, 0.105, 0.065).lerp(Vec3::new(0.40, 0.31, 0.19), clods)
        * (0.83 + grain * 0.3);
    let cx = x as i32 / 32;
    let cy = y as i32 / 32;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let a = (cx + dx) as u32;
            let b = (cy + dy) as u32;
            let center = Vec2::new(
                (cx + dx) as f32 * 32. + hash(a, b) * 32.,
                (cy + dy) as f32 * 32. + hash(b, a) * 32.,
            );
            let size = 2. + hash(a.wrapping_add(7), b) * 7.;
            let q = (Vec2::new(x as f32, y as f32) - center) / Vec2::new(size, size * 0.7);
            if q.length_squared() < 1. {
                let dome = (1. - q.length_squared()).sqrt();
                height += dome * 0.35;
                color = Vec3::new(0.36, 0.34, 0.29) * (0.7 + dome * 0.4 + grain * 0.12);
            }
        }
    }
    (color, height)
}
pub(super) fn den_material(iron: bool, images: &mut Assets<Image>) -> StandardMaterial {
    let mut color = Vec::new();
    let mut normals = Vec::new();
    let mut rough = Vec::new();
    let grain =
        |x: u64, y: u64| (mix(x.wrapping_mul(7411) ^ y.wrapping_mul(9127)) & 65535) as f32 / 65535.;
    let size = if iron { 128 } else { 512 };
    for y in 0..size {
        for x in 0..size {
            let n = grain(x, y);
            let u = (x % 12) as f32 / 12.;
            let v = (y % 12) as f32 / 12.;
            let u = u * u * (3. - 2. * u);
            let v = v * v * (3. - 2. * v);
            let a = grain(x / 12, y / 12) * (1. - u) + grain(x / 12 + 1, y / 12) * u;
            let b = grain(x / 12, y / 12 + 1) * (1. - u) + grain(x / 12 + 1, y / 12 + 1) * u;
            let patch = a * (1. - v) + b * v;
            let rgb = if iron {
                Vec3::new(0.14, 0.16, 0.17).lerp(Vec3::new(0.51, 0.20, 0.065), patch)
                    * (0.65 + n * 0.45)
            } else {
                soil_texel(x as u32, y as u32).0
            };
            color.extend([rgb.x, rgb.y, rgb.z, 1.].map(|v| (v.clamp(0., 1.) * 255.) as u8));
            let normal = if iron {
                Vec3::new(
                    (n - grain((x + 1) % size, y)) * 0.6,
                    (n - grain(x, (y + 1) % size)) * 0.6,
                    1.,
                )
            } else {
                let h = soil_texel(x as u32, y as u32).1;
                Vec3::new(
                    (h - soil_texel((x + 1) as u32, y as u32).1) * 3.,
                    (h - soil_texel(x as u32, (y + 1) as u32).1) * 3.,
                    1.,
                )
            }
            .normalize()
                * 0.5
                + Vec3::splat(0.5);
            normals.extend([normal.x, normal.y, normal.z, 1.].map(|v| (v * 255.) as u8));
            rough.extend([
                255,
                230,
                if iron { ((1. - patch) * 160.) as u8 } else { 0 },
                255,
            ]);
        }
    }
    use crate::rendering::mipmaps::{self, Filter};
    let mut add = |data, filter| {
        let mut image = Image::new(
            Extent3d {
                width: size as u32,
                height: size as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            if matches!(filter, Filter::Color) {
                TextureFormat::Rgba8UnormSrgb
            } else {
                TextureFormat::Rgba8Unorm
            },
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
        if !(std::env::var_os("HITHER_PROFILE_FOREST").is_some()
            && std::env::var_os("HITHER_PROFILE_NO_ORC_MIPS").is_some())
        {
            mipmaps::generate(&mut image, filter);
        }
        images.add(image)
    };
    StandardMaterial {
        base_color_texture: Some(add(color, Filter::Color)),
        normal_map_texture: Some(add(normals, Filter::Normal)),
        metallic_roughness_texture: Some(add(rough, Filter::Linear)),
        metallic: if iron { 1. } else { 0. },
        reflectance: if iron { 0.5 } else { 0.0 },
        perceptual_roughness: 1.,
        cull_mode: None,
        double_sided: true,
        ..default()
    }
}
