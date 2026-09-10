use super::*;
fn example() -> Den {
    example_for_seed(721)
}
fn example_for_seed(seed: u64) -> Den {
    for r in 0..60_i32 {
        for x in -r..=r {
            for z in -r..=r {
                if x.abs() != r && z.abs() != r {
                    continue;
                }
                if let Some(d) = generate(IVec2::new(x, z), seed) {
                    return d;
                }
            }
        }
    }
    panic!("seed {seed} must have a suitable great mountain");
}
#[test]
fn rarity_matches_oak_and_layout_is_reproducible() {
    let d = example();
    let again = generate(d.cell, 721).unwrap();
    assert_eq!(d.center, again.center);
    assert_eq!(d.residents(), again.residents());
    assert_eq!(d.tunnels, again.tunnels);
    assert!(super::super::orchard::oak::region_enabled(
        d.cell,
        721 ^ 0x676f626c696e
    ));
    assert!((2..=5).contains(&d.houses.len()));
    assert!((1..=3).contains(&d.tunnels.len()));
    assert_eq!(d.residents().len(), d.houses.len() * 4);
    assert!(biome::mountain_for_seed(d.center.xz(), 721) >= 0.85);
    assert!(biome::terrain_for_seed(d.center.xz(), 721) - d.center.y >= 62.);
    assert!(generate(IVec2::ZERO, 721).is_none());
    println!("preview den {:?} at {}", d.cell, d.center);
}
#[test]
fn tunnels_reach_high_surface_and_have_walkable_gradients() {
    let d = example();
    for path in &d.tunnels {
        assert!(d.air(path[0]) < 0.);
        let exit = d.center + *path.last().unwrap();
        assert!(exit.y > biome::terrain_for_seed(exit.xz(), 721));
        assert!(exit.y > 120.);
        for w in path.windows(2) {
            assert!((w[1].y - w[0].y).abs() / w[1].xz().distance(w[0].xz()) < 0.65);
            for i in 0..10 {
                assert!(d.air(w[0].lerp(w[1], i as f32 / 10.)) <= -1.10);
            }
        }
    }
}
#[test]
fn residents_and_bridges_have_actual_mesh_support_and_clearance() {
    let d = example();
    let built = d.built();
    assert!(built.meshes[0].positions.len() > 1000);
    for home in d.residents() {
        let p = home - d.center;
        let hit = built.floors.hit(p + Vec3::Y * 0.05, -Vec3::Y, 0.12);
        assert!(hit.is_some(), "missing house floor at {p}");
        for y in [0.15, 0.30, 0.45] {
            assert!(
                !built.collision.overlaps_sphere(p + Vec3::Y * y, 0.12),
                "resident intersects furnishings at {p}"
            );
        }
    }
    // Test the same rendered boards the player and navigation query. At a
    // joint either adjacent plank may supply support, with no fall-through.
    for span in &d.spans {
        for i in 0..=50 {
            let p = span.point(i as f32 / 50.);
            assert!(
                built.floors.hit(p + Vec3::Y * 0.2, -Vec3::Y, 0.5).is_some(),
                "bridge gap at {p}"
            );
        }
    }
    assert!(
        built
            .meshes
            .iter()
            .flat_map(|g| &g.positions)
            .flatten()
            .all(|v| v.is_finite())
    );
}

#[test]
fn all_house_doors_and_passages_fit_a_player() {
    let d = example();
    let built = d.built();
    let clear = |p: Vec3| {
        let floor = d
            .floor_levels(d.center.xz() + p.xz(), 0.28)
            .into_iter()
            .filter(|y| *y <= d.center.y + p.y + 0.21)
            .max_by(f32::total_cmp)
            .expect("route needs a floor");
        let p = p.with_y(floor - d.center.y + 0.01);
        for y in [0.31, 0.55, 0.8, 1.05, 1.27] {
            assert!(
                !built.collision.overlaps_sphere(p + Vec3::Y * y, 0.28),
                "blocked player route at {p}, height {y}, nearest face {:?}",
                built
                    .collision
                    .triangles
                    .iter()
                    .min_by(|a, b| a
                        .distance_squared(p + Vec3::Y * y)
                        .total_cmp(&b.distance_squared(p + Vec3::Y * y)))
                    .map(|t| t.0)
            );
        }
    };
    for span in &d.spans {
        for i in 0..=60 {
            clear(span.point(i as f32 / 60.));
        }
    }
    for h in d.houses.iter().chain([&d.shrine, &d.food_hall]) {
        for i in 0..=30 {
            clear(
                h.at + Quat::from_rotation_y(h.yaw)
                    * Vec3::Z
                    * (i as f32 / 30.)
                    * (h.depth * 0.5 + 0.35),
            );
        }
    }
    for path in &d.tunnels {
        for w in path.windows(2) {
            for i in 0..=4 {
                clear(w[0].lerp(w[1], i as f32 / 4.) - Vec3::Y * 1.45);
            }
        }
    }
}

#[test]
fn civic_buildings_have_twelve_readable_relics_and_covered_floors() {
    let d = example();
    let built = d.built();
    assert_eq!(built.relics.len(), 12 + d.tunnels.len() * 4);
    let mut entries: Vec<_> = built
        .relics
        .iter()
        .filter(|r| r.entry < 12)
        .map(|r| r.entry)
        .collect();
    entries.sort_unstable();
    assert_eq!(entries, (0..12).collect::<Vec<_>>());
    for h in [&d.shrine, &d.food_hall] {
        let rotation = Quat::from_rotation_y(h.yaw);
        for x in [-h.width * 0.5, 0., h.width * 0.5] {
            for z in [-h.depth * 0.5, 0., h.depth * 0.5] {
                let p = h.at + rotation * Vec3::new(x, 0.01, z);
                assert!(d.air(p) < 0., "civic floor intersects cave at {p}");
                assert!(
                    built
                        .floors
                        .hit(p + Vec3::Y * 0.05, -Vec3::Y, 0.12)
                        .is_some()
                );
            }
        }
    }
}

#[test]
fn excavated_passages_have_closed_roofs_before_the_surface_mouth() {
    let den = example();
    let built = den.built();
    for path in &den.tunnels {
        for w in path.windows(2).skip(3).take(80) {
            let side = Vec3::Y.cross((w[1] - w[0]).with_y(0.).normalize());
            for direction in [
                Vec3::Y,
                (Vec3::Y + side * 0.6).normalize(),
                (Vec3::Y - side * 0.6).normalize(),
            ] {
                assert!(
                    built.collision.hit(w[0], direction, 12.).is_some(),
                    "unintended skylight before the tunnel mouth at {:?}",
                    w[0]
                );
            }
        }
    }
}

#[test]
fn tunnel_bridge_turn_has_supported_corners() {
    for seed in [721, 42, 1701] {
        let d = example_for_seed(seed);
        let built = d.built();
        for path in &d.tunnels {
            let at = path[1] - Vec3::Y * 1.45;
            // Sweep the whole turn, not only each route's centerline: these corners
            // were previously open between differently oriented deck/floor edges.
            for i in 0..32 {
                let angle = i as f32 * TAU / 32.;
                let p = at + Vec3::new(angle.cos(), 0., angle.sin()) * 1.55;
                assert!(
                    built.floors.hit(p + Vec3::Y * 1.5, -Vec3::Y, 2.0).is_some(),
                    "unsupported tunnel landing corner at {p}"
                );
            }
        }
    }
}

#[test]
fn indexed_air_matches_the_full_segment_scan() {
    let den = example();
    for path in &den.tunnels {
        for &p in path {
            for x in -4..=4 {
                for y in -4..=4 {
                    for z in -4..=4 {
                        let q = p + Vec3::new(x as f32, y as f32, z as f32);
                        assert_eq!(
                            den.air(q).to_bits(),
                            den.air_reference(q).to_bits(),
                            "{q:?}"
                        );
                    }
                }
            }
        }
    }
    for x in -12..=12 {
        for y in -8..=8 {
            for z in -12..=12 {
                let p = Vec3::new(x as f32, y as f32, z as f32) * 4.;
                assert_eq!(
                    den.air(p).to_bits(),
                    den.air_reference(p).to_bits(),
                    "{p:?}"
                );
            }
        }
    }
}

#[test]
fn indexed_floors_preserve_support_in_chambers_and_tunnels() {
    let d = example();
    let built = d.built();
    let points = d
        .tunnels
        .iter()
        .flat_map(|path| {
            path.windows(2)
                .flat_map(|w| (0..5).map(move |i| w[0].lerp(w[1], i as f32 / 5.0).xz()))
        })
        .chain((-15..=15).flat_map(|x| (-15..=15).map(move |z| Vec2::new(x as f32, z as f32))));
    for p in points {
        let origin = Vec3::new(p.x, 120.0, p.y);
        let actual = built.floors.downward_hits(origin, -30.0);
        let mut expected = Vec::new();
        let mut y = origin.y;
        for _ in 0..24 {
            let Some(t) = built.floors.hit(origin.with_y(y), -Vec3::Y, y + 30.0) else {
                break;
            };
            y -= t;
            expected.push(y);
            y -= 0.025;
        }
        assert_eq!(
            actual.len(),
            expected.len(),
            "floor count at {p:?}: {actual:?} vs {expected:?}"
        );
        for (a, b) in actual.iter().zip(&expected) {
            assert!((a - b).abs() < 0.0001, "floor at {p:?}: {a} vs {b}");
        }
    }
}

#[test]
#[ignore = "Opt-in CPU floor-query microbenchmark; run without builds"]
fn profile_den_floor_queries() {
    let den = example_for_seed(42);
    let built = den.built();
    let points: Vec<_> = den
        .tunnels
        .iter()
        .flat_map(|p| p.iter().copied())
        .chain(den.residents().into_iter().map(|p| p - den.center))
        .map(|p| p.with_y(120.0))
        .collect();
    let run = |indexed: bool| {
        let start = std::time::Instant::now();
        let mut count = 0usize;
        for _ in 0..20 {
            for &origin in &points {
                if indexed {
                    count += std::hint::black_box(built.floors.downward_hits(origin, -30.0)).len();
                } else {
                    let mut y = origin.y;
                    for _ in 0..24 {
                        let Some(t) = built.floors.hit(origin.with_y(y), -Vec3::Y, y + 30.0) else {
                            break;
                        };
                        y -= t;
                        count += 1;
                        y -= 0.025;
                    }
                }
            }
        }
        (
            start.elapsed().as_secs_f64() * 1000.0,
            std::hint::black_box(count),
        )
    };
    let (before, a) = run(false);
    let (after, b) = run(true);
    assert_eq!(a, b);
    println!(
        "FLOOR_QUERIES count={} old_ms={before:.3} indexed_ms={after:.3} speedup={:.3}x",
        points.len() * 20,
        before / after
    );
}
