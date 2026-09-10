use super::*;

#[test]
fn randomized_trees_are_reproducible_and_support_five_attached_apples() {
    let mut fingerprints = std::collections::HashSet::new();
    for seed in 0..32 {
        let tree = generate_tree(seed);
        assert!(tree.fruit_spurs.len() > 100);
        assert!(tree.wood.indices.len() + tree.leaves.indices.len() < 1_200_000);
        for geometry in [&tree.wood, &tree.leaves] {
            assert!(geometry.positions.iter().flatten().all(|x| x.is_finite()));
            assert!(
                geometry
                    .normals
                    .iter()
                    .all(|n| (Vec3::from(*n).length() - 1.0).abs() < 0.001)
            );
        }
        fingerprints.insert((
            tree.wood.positions.len(),
            tree.leaves.positions.len(),
            tree.fruit_spurs[0][0].x.to_bits(),
        ));
        if seed == 0 {
            let repeat = generate_tree(seed);
            assert_eq!(tree.wood.positions, repeat.wood.positions);
            assert_eq!(tree.leaves.positions, repeat.leaves.positions);
            assert_eq!(tree.fruit_spurs, repeat.fruit_spurs);
        }
        let mut rng = Rng(seed + 517);
        // Multiple harvest/refill rounds for every distinct tree shape.
        for _ in 0..10 {
            let mut apples = Vec::<Vec3>::new();
            for _ in 0..MAX_APPLES {
                let p = (0..256)
                    .find_map(|_| {
                        let choice = (rng.unit() * u32::MAX as f32) as u32;
                        let p = tree.hanging_position(choice, rng.unit(), rng.unit());
                        (tree.fruit_has_clearance(p)
                            && apples.iter().all(|q| {
                                p.distance(*q) >= crate::world::orchard::fruit::MIN_APPLE_SPACING
                            }))
                        .then_some(p)
                    })
                    .unwrap_or_else(|| panic!("No room for five apples with seed {seed}"));
                let anchor = tree
                    .fruit_anchor(p)
                    .expect("Generated wood directly above fruit");
                assert!(anchor.y - p.y > APPLE_RADIUS);
                assert!(anchor.xz().distance(p.xz()) < 0.0001);
                // Rays back up the stalk must actually meet rendered wood.
                assert!(tree.rays.hit(p, Vec3::Y, 0.4).is_some());
                apples.push(p);
            }
        }
    }
    assert_eq!(fingerprints.len(), 32);
}

#[test]
fn botanical_geometry_is_finite_and_detailed() {
    for geometry in [&tree().wood, &tree().leaves, apple()] {
        assert!(geometry.positions.len() > 1000);
        assert!(geometry.positions.iter().flatten().all(|x| x.is_finite()));
        assert!(
            geometry
                .normals
                .iter()
                .all(|n| (Vec3::from(*n).length() - 1.0).abs() < 0.001)
        );
        assert!(
            geometry
                .indices
                .iter()
                .all(|i| (*i as usize) < geometry.positions.len())
        );
    }
    assert!(tree().fruit_spurs.len() > 50);
}
#[test]
fn apple_picking_uses_dimpled_surface() {
    let center = Vec3::new(2.0, 1.25, 1.0);
    let inverse = apple_rotation(center).inverse();
    let origin = inverse * Vec3::Z * 2.0;
    let direction = inverse * (-Vec3::Z);
    assert!(apple_rays().hit(origin, direction, 70.0).is_some());
    for i in 0..100 {
        let center = Vec3::new(i as f32 * 0.11, 1.25, 1.0);
        assert!(apple_hit(center + Vec3::Z * 2.0, -Vec3::Z, center).is_some());
    }
    assert!(apple_hit(Vec3::Z, -Vec3::Z, Vec3::ZERO).is_some());
    // A ray through the old sphere's top now misses the inset stem dimple.
    assert!(apple_hit(Vec3::new(0.0, 0.138, 1.0), -Vec3::Z, Vec3::ZERO).is_none());
    assert!(apple_hit(Vec3::new(0.16, 0.0, 1.0), -Vec3::Z, Vec3::ZERO).is_none());
}
#[test]
fn tree_raycast_hits_wood_but_not_empty_space() {
    assert!(tree_occludes(Vec3::new(0.0, 1.0, 2.0), -Vec3::Z, 4.0));
    assert!(!tree_occludes(Vec3::new(2.0, 1.0, 2.0), -Vec3::Z, 4.0));
    // Axis-aligned rays on a BVH slab must not produce NaNs or false hits.
    assert!(!tree_occludes(Vec3::new(0.0, 5.0, 2.0), -Vec3::Z, 4.0));
}

#[test]
fn apples_hang_below_rendered_branches_with_spacing() {
    let mut state = OrchardState {
        random_state: 76251,
        ..default()
    };
    let mut distinct = std::collections::HashSet::new();
    for _ in 0..100 {
        state.apples.clear();
        for _ in 0..MAX_APPLES {
            let p = state
                .random_apple_position()
                .expect("Room for five hanging apples");
            let anchor = fruit_anchor(p).expect("Real branch above fruit");
            assert!(anchor.y > p.y + APPLE_RADIUS);
            assert!(anchor.xz().distance(p.xz()) < 0.0001);
            assert!(fruit_has_clearance(p));
            assert!(
                state
                    .apples
                    .iter()
                    .all(|q| p.distance(*q) >= crate::world::orchard::fruit::MIN_APPLE_SPACING)
            );
            distinct.insert([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
            state.apples.push(p);
        }
    }
    assert!(
        distinct.len() > 490,
        "Positions must be continuous, not fixed fruit sockets"
    );
}

#[test]
fn timed_regrowth_updates_visible_mesh_slots_after_harvest() {
    let mut world = World::new();
    let mut state = OrchardState::default();
    state.apples.clear();
    state.random_state = 3751;
    world.insert_resource(state);
    world.insert_resource(crate::app::GameState::default());
    world.insert_resource(Time::<()>::default());
    world.insert_resource(Assets::<Mesh>::default());
    world.insert_resource(OrchardMaterials {
        wood: Handle::default(),
        leaf: Handle::default(),
    });
    for slot in 0..MAX_APPLES {
        world.spawn((
            AppleVisual {
                slot,
                position: None,
            },
            Transform::default(),
            Visibility::Hidden,
        ));
    }
    let mut schedule = Schedule::default();
    schedule.add_systems((crate::world::orchard::fruit::grow_apples, sync_apples).chain());
    // Fast-forward gameplay time, without changing the production chance.
    for _ in 0..4000 {
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        schedule.run(&mut world);
    }
    let visible_count = |world: &mut World| {
        world
            .query::<(&AppleVisual, &Visibility)>()
            .iter(world)
            .filter(|(a, v)| a.position.is_some() && **v != Visibility::Hidden)
            .count()
    };
    assert_eq!(world.resource::<OrchardState>().apples.len(), MAX_APPLES);
    assert_eq!(visible_count(&mut world), MAX_APPLES);
    world.resource_mut::<OrchardState>().apples.clear();
    world
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::ZERO);
    schedule.run(&mut world);
    assert_eq!(visible_count(&mut world), 0);
    for _ in 0..4000 {
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        schedule.run(&mut world);
        assert!(world.resource::<OrchardState>().apples.len() <= MAX_APPLES);
    }
    assert_eq!(visible_count(&mut world), MAX_APPLES);
    // Pausing stops both the roll and growth timer, even with a large delta.
    world.resource_mut::<OrchardState>().apples.clear();
    world.resource_mut::<crate::app::GameState>().paused = true;
    let before = world.resource::<OrchardState>().random_state;
    world
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1000));
    schedule.run(&mut world);
    assert_eq!(world.resource::<OrchardState>().random_state, before);
    assert!(world.resource::<OrchardState>().apples.is_empty());
}

#[test]
fn growth_rolls_are_per_second_not_per_frame() {
    fn simulate(milliseconds: u64, frames: usize) -> (Vec<Vec3>, u64) {
        let mut world = World::new();
        world.insert_resource(OrchardState {
            apples: Vec::new(),
            random_state: 8592,
            ..default()
        });
        world.insert_resource(crate::app::GameState::default());
        world.insert_resource(Time::<()>::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(crate::world::orchard::fruit::grow_apples);
        for _ in 0..frames {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(milliseconds));
            schedule.run(&mut world);
        }
        let state = world.resource::<OrchardState>();
        (state.apples.clone(), state.random_state)
    }
    assert_eq!(simulate(1000, 600), simulate(100, 6000));
    assert_eq!(simulate(1000, 600), simulate(5000, 120));
}

#[test]
fn spawn_starts_without_apples() {
    assert!(OrchardState::default().apples.is_empty());
}
