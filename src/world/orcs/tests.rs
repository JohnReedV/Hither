use super::*;

#[test]
fn carried_stone_tracks_live_palm_and_detaches_on_release() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_systems(
            PostUpdate,
            attach_carried_stone.before(bevy::transform::TransformSystems::Propagate),
        );
    let home = Vec3::new(20., 3., 30.);
    let owner = app
        .world_mut()
        .spawn((
            Orc {
                variant: 0,
                home,
                age: 5.,
                mode: 3,
                step_distance: 0.,
                left: false,
                stone_leader: true,
                retreat: None,
                navigation: navigation::Navigator::pursuit(),
                fall_velocity: 0.,
                engaged: true,
            },
            Transform::from_translation(home).with_scale(Vec3::new(1.18, 1.12, 1.12)),
        ))
        .id();
    let hand = app
        .world_mut()
        .spawn((StonePalm { owner }, Transform::default()))
        .id();
    app.world_mut().entity_mut(owner).add_child(hand);
    let gate = app.world_mut().spawn((Gate, Transform::default())).id();
    let den = app
        .world_mut()
        .spawn((
            Den {
                cell: IVec2::ZERO,
                elapsed: Some(5.),
                emitted: 1,
                open: 0.69537,
            },
            Transform::from_translation(home),
        ))
        .id();
    app.world_mut().entity_mut(den).add_child(gate);
    let socket: [f32; 3] = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/orcs/stone-palm.json"
    )))
    .unwrap();
    // Deliberately change the joint every tick, away from any baked sample.
    for i in 0..12 {
        *app.world_mut().get_mut::<Transform>(hand).unwrap() =
            Transform::from_xyz(0.1 * i as f32, 2. + 0.017 * i as f32, 0.3)
                .with_rotation(Quat::from_rotation_z(0.031 * i as f32));
        app.update();
        let palm = app
            .world()
            .get::<GlobalTransform>(hand)
            .unwrap()
            .transform_point(Vec3::from_array(socket));
        let bearing = app
            .world()
            .get::<GlobalTransform>(gate)
            .unwrap()
            .translation()
            + Vec3::Y * STONE_BEARING_Y;
        assert!(
            palm.distance(bearing) < 0.00001,
            "live palm contact drifted"
        );
    }
    app.world_mut().get_mut::<Den>(den).unwrap().open = 0.7001;
    let released = Vec3::new(4., 5., 6.);
    app.world_mut()
        .get_mut::<Transform>(gate)
        .unwrap()
        .translation = released;
    app.update();
    assert_eq!(
        app.world().get::<Transform>(gate).unwrap().translation,
        released
    );
}

#[test]
fn camera_cannot_dip_below_the_visible_room_floor() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 151);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        for z in [-4.1, -5., -8., -11., -14.] {
            for x in [-1.3, 0., 1.3] {
                let at = tunnel_center(z).xz() + Vec2::X * x;
                if let Some((y, ceiling)) = visible_tunnel_bounds(at, kind) {
                    let below = home + Vec3::new(at.x, y - 0.05, at.y);
                    assert!(
                        !camera_clear(below, 0.16),
                        "camera under mesh: den {kind}, {at:?}"
                    );
                    let above = home + Vec3::new(at.x, ceiling + 0.05, at.y);
                    assert!(
                        !camera_clear(above, 0.16),
                        "camera above mesh: den {kind}, {at:?}"
                    );
                }
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn den_perimeters_block_bodies_and_cameras_even_at_the_room_cap() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 149);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        for side in [-1., 1.] {
            let z = -3.95;
            let width = if kind == 2 {
                burrow_width(z) - 0.12
            } else {
                2.08
            };
            let point = home
                + Vec3::new(
                    side * (width - crate::player::movement::PLAYER_RADIUS + 0.03),
                    -4.,
                    z,
                );
            assert!(
                underground_collision(point, crate::player::movement::PLAYER_RADIUS),
                "body escaped den {kind}"
            );
            assert!(
                !camera_clear(point, crate::player::movement::PLAYER_RADIUS),
                "camera escaped den {kind}"
            );
        }
        // Follow the visible ring cross-sections through the room's bend.
        for z in [-6., -8., -10., -12., -14.] {
            for side in [-1., 1.] {
                let center = tunnel_center(z);
                let point = home
                    + center
                    + Vec3::new(
                        side * (1.85 - crate::player::movement::PLAYER_RADIUS + 0.03),
                        1.25,
                        0.,
                    );
                assert!(
                    underground_collision(point, crate::player::movement::PLAYER_RADIUS),
                    "room wall bypass in den {kind} at {z}"
                );
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn airborne_player_cannot_move_under_den_banks() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 150);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        for z in [-13., -8., -4.2, -3., 0., 3.] {
            let x = if z < -4. { tunnel_center(z).x } else { 0. };
            let probe = home + Vec3::new(x, -2., z);
            let floor = crate::player::movement::world_support_height(
                probe,
                crate::player::movement::PLAYER_RADIUS,
            );
            for lift in [0.03, 0.3, 0.8] {
                let start = probe.with_y(floor + crate::player::movement::PLAYER_EYE_HEIGHT + lift);
                if crate::player::movement::character_collides(
                    start,
                    crate::player::movement::PLAYER_RADIUS,
                ) {
                    continue;
                }
                for direction in 0..16 {
                    let angle = direction as f32 * TAU / 16.;
                    let end = crate::player::movement::move_player_with_collisions(
                        start,
                        Vec3::new(angle.cos(), 0., angle.sin()) * 3.,
                    );
                    let support = crate::player::movement::world_support_height(
                        end,
                        crate::player::movement::PLAYER_RADIUS,
                    );
                    assert!(
                        end.y - crate::player::movement::PLAYER_EYE_HEIGHT >= support - 0.2011,
                        "buried feet: den {kind} z {z} lift {lift} direction {direction}"
                    );
                    let mut rig = crate::player::camera::CameraRig {
                        position: end,
                        jump_height: end.y - crate::player::movement::PLAYER_EYE_HEIGHT,
                        grounded: false,
                        ..default()
                    };
                    for _ in 0..150 {
                        crate::player::movement::update_jump(
                            &mut rig,
                            &ButtonInput::default(),
                            0.02,
                        );
                    }
                    assert!(
                        rig.grounded && rig.jump_height > -8.,
                        "unsettled den {kind} at {z} lift={lift} dir={direction} end={end:?} final={:?} feet={} vy={} support={}",
                        rig.position - home,
                        rig.jump_height,
                        rig.vertical_velocity,
                        crate::player::movement::world_support_height(
                            rig.position.with_y(
                                rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT
                            ),
                            crate::player::movement::PLAYER_RADIUS
                        )
                    );
                }
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn every_den_has_a_welded_ramp_to_room_floor() {
    let key = |p: Vec3| p.to_array().map(|v| (v * 100_000.).round() as i32);
    for kind in 0..3 {
        let (earth, _, _) = den_variant(kind);
        let mut edges = HashMap::new();
        for triangle in earth.indices.chunks_exact(3) {
            for i in 0..3 {
                let a = Vec3::from_array(earth.positions[triangle[i] as usize]);
                let b = Vec3::from_array(earth.positions[triangle[(i + 1) % 3] as usize]);
                if a.z != -4. || b.z != -4. {
                    continue;
                }
                let mut edge = [key(a), key(b)];
                edge.sort();
                *edges.entry(edge).or_insert(0) += 1;
            }
        }
        for column in 0..20 {
            let boundary = |i: usize| {
                let x = -2.2 + i as f32 * 0.22;
                let mut p = Vec3::new(x, ramp_height(x, -4.), -4.);
                if kind == 2 {
                    p.x *= burrow_width(p.z) / 2.2;
                }
                key(p)
            };
            let mut edge = [boundary(column), boundary(column + 1)];
            edge.sort();
            assert_eq!(
                edges.get(&edge),
                Some(&2),
                "open floor seam: den {kind}, column {column}"
            );
        }
    }
}

#[test]
fn targeting_is_limited_to_fifty_meters_and_excludes_spectator() {
    let eye = Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
    assert!(can_target(Vec3::ZERO, eye + Vec3::X * 50., false));
    assert!(!can_target(Vec3::ZERO, eye + Vec3::X * 50.01, false));
    assert!(!can_target(Vec3::ZERO, eye, true));
    assert!(can_target(Vec3::ZERO, eye + Vec3::X * 49.9, false));
}
#[test]
fn orc_toes_follow_actual_travel_in_all_directions() {
    for i in 0..32 {
        let angle = i as f32 * std::f32::consts::TAU / 32.;
        let movement = Vec3::new(angle.sin(), 0., angle.cos());
        let yaw = footprint_yaw(movement);
        let toes = Vec2::new(yaw.sin(), -yaw.cos());
        assert!(toes.dot(movement.xz()) > 0.9999);
    }
}
#[test]
fn return_route_approaches_the_front_and_reverses_inside_without_teleporting() {
    for i in 0..100 {
        let progress = i as f32 / 100. * EMERGENCE_SECONDS;
        let entry = emergence_position(EMERGENCE_SECONDS - progress);
        let resumed_exit =
            emergence_position((EMERGENCE_SECONDS - progress + 0.01).min(EMERGENCE_SECONDS));
        assert!(entry.distance(resumed_exit) < 0.025);
    }
    assert!(emergence_position(0.).y < -5.);
}
#[test]
fn party_probabilities_and_variants() {
    let mut counts = [0; 3];
    let mut variants = [0; 3];
    for h in 0..12000 {
        let p = party(h);
        counts[p.len() - 1] += 1;
        for v in p {
            variants[v] += 1;
        }
    }
    assert_eq!(counts, [3000, 6000, 3000]);
    for n in variants {
        assert!((7500..8500).contains(&n));
    }
}
#[test]
fn placement_acceptance_does_not_bias_encounter_size() {
    let mut counts = [0; 3];
    for x in -50..50 {
        for z in -50..50 {
            let cell = IVec2::new(x, z);
            if site(cell).is_some() {
                counts[party(mix(seed(cell) ^ 0x656e636f756e7465)).len() - 1] += 1;
            }
        }
    }
    let total = counts.iter().sum::<usize>() as f32;
    for (n, target) in counts.into_iter().zip([0.25, 0.5, 0.25]) {
        assert!((n as f32 / total - target).abs() < 0.04);
    }
}
#[test]
fn entrances_remain_blocked_and_alert_memory_survives_unloading() {
    let p = preview();
    assert!(entrance_overlap(
        p + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
        0.28
    ));
    assert!(!entrance_overlap(p + Vec3::X * 2.7, 0.28));
    assert_eq!(
        surface_height(
            p + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
            0.28
        ),
        Some(GRATE_HEIGHT)
    );
    assert_eq!(
        surface_height(
            p + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT + Vec3::X * 3.,
            0.28
        ),
        Some(0.)
    );
    let cell = (p.xz() / CELL).floor().as_ivec2();
    let mut dens = Dens::default();
    assert!(dens.alerted.insert(cell));
    dens.loaded.clear();
    assert!(!dens.alerted.insert(cell));
    let holes = hole_uniforms(p, 48.);
    assert_eq!(holes[0], Vec4::new(p.x, p.z, HOLE_RADIUS, HOLE_LENGTH));
}
#[test]
fn lair_rearms_only_after_the_whole_party_returns_and_player_leaves() {
    assert!(!encounter_can_reset(1, 3, false, false, 0.));
    assert!(!encounter_can_reset(3, 3, true, false, 0.));
    assert!(!encounter_can_reset(3, 3, false, true, 0.));
    assert!(!encounter_can_reset(3, 3, false, false, 0.5));
    assert!(encounter_can_reset(3, 3, false, false, 0.));
}
#[test]
fn underground_orcs_hold_an_open_gate_and_rear_is_sealed() {
    let home = Vec3::new(100., 0., 100.);
    assert!(needs_exit_gate(home + Vec3::new(0., -2., 1.), home));
    assert!(needs_exit_gate(home + Vec3::new(0., -6., -7.), home));
    assert!(!needs_exit_gate(
        home + Vec3::new(0., GRATE_HEIGHT, 5.),
        home
    ));
    assert!(tunnel_floor(Vec2::new(5., -14.)).is_some());
    assert!(tunnel_floor(Vec2::new(6., -15.)).is_none());
    let (earth, _, _) = den_geometry();
    assert!(
        earth
            .positions
            .iter()
            .zip(&earth.colors)
            .any(|(p, c)| { p[2] == -15. && c[0] == 0. && c[1] == 0. && c[2] == 0. })
    );
}
#[test]
fn iron_den_jump_interrupts_every_stage_of_emergence() {
    for age in [0.1_f32, 2., 4., 6., 7.3] {
        let kind = 0;
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 128);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|home| (cell, home))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<crate::app::GameState>()
            .init_resource::<crate::player::camera::CameraRig>()
            .init_resource::<crate::world::snow::Tracks>()
            .init_resource::<Trampling>()
            .init_resource::<Dens>()
            .insert_resource(Art {
                gltfs: vec![],
                models: (0..3)
                    .map(|_| Model {
                        scene: default(),
                        graph: default(),
                        clips: vec![],
                    })
                    .collect(),
                dens: vec![],
                rock: default(),
                iron: default(),
                shale: default(),
                torch: default(),
            })
            .add_systems(Update, (encounters, animate).chain());
        let den = app
            .world_mut()
            .spawn((
                Den {
                    cell,
                    elapsed: Some(30.),
                    emitted: 3,
                    open: 1.,
                },
                Transform::from_translation(home),
            ))
            .id();
        let gate = app.world_mut().spawn((Gate, Transform::default())).id();
        app.world_mut().entity_mut(den).add_child(gate);
        let residents: Vec<_> = (0..3)
            .map(|variant| {
                let age = (age - variant as f32 * 1.7).max(0.01);
                app.world_mut()
                    .spawn((
                        Orc {
                            variant,
                            home,
                            age,
                            mode: 1,
                            step_distance: 0.,
                            left: false,
                            stone_leader: false,
                            retreat: None,
                            navigation: navigation::Navigator::pursuit(),
                            fall_velocity: 0.,
                            engaged: false,
                        },
                        Transform::from_translation(home + emergence_position(age)),
                    ))
                    .id()
            })
            .collect();
        let initial: Vec<_> = residents
            .iter()
            .map(|e| app.world().get::<Transform>(*e).unwrap().translation)
            .collect();
        // Jump through the opening while the staggered party is still
        // following its scripted exit. Run real player gravity during the drop.
        {
            let mut rig = app
                .world_mut()
                .resource_mut::<crate::player::camera::CameraRig>();
            rig.position =
                home + Vec3::new(0., crate::player::movement::PLAYER_EYE_HEIGHT + 0.5, 1.);
            rig.jump_height = 0.5;
            rig.grounded = false;
        }
        for _ in 0..60 {
            crate::player::movement::update_jump(
                &mut app
                    .world_mut()
                    .resource_mut::<crate::player::camera::CameraRig>(),
                &ButtonInput::default(),
                1. / 30.,
            );
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(1. / 30.));
            app.update();
        }
        for (e, initial) in residents.iter().zip(initial) {
            let actor = app.world().get::<Orc>(*e).unwrap();
            let p = app.world().get::<Transform>(*e).unwrap().translation;
            assert!(
                actor.age >= EMERGENCE_SECONDS,
                "exit did not cancel at age={age}"
            );
            if initial.distance(
                app.world()
                    .resource::<crate::player::camera::CameraRig>()
                    .position
                    - Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
            ) > 3.
            {
                assert!(
                    p.distance(initial) > 0.25,
                    "chase did not resume promptly at age={age}: {p:?} status={:?}",
                    actor.navigation.status
                );
            }
        }
        let inside = home + Vec3::new(1., tunnel_floor(Vec2::new(1., -10.)).unwrap(), -10.);
        for goal in [inside, home + Vec3::Z * 9.] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = goal + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
            for _ in 0..1800 {
                app.update();
                if residents.iter().all(|e| {
                    app.world()
                        .get::<Transform>(*e)
                        .unwrap()
                        .translation
                        .distance(goal)
                        < 3.3
                }) {
                    break;
                }
            }
            assert!(
                residents.iter().all(|e| app
                    .world()
                    .get::<Transform>(*e)
                    .unwrap()
                    .translation
                    .distance(goal)
                    < 3.3),
                "jump at age={age}, goal={:?}, residents={:?}",
                goal - home,
                residents
                    .iter()
                    .map(|e| app.world().get::<Transform>(*e).unwrap().translation - home)
                    .collect::<Vec<_>>()
            );
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn targetless_party_enters_and_closes_each_den_from_the_lip() {
    for (kind, dt, start_z) in (0..3)
        .flat_map(|kind| [1. / 30., 1. / 144.].map(move |dt| (kind, dt)))
        .flat_map(|(kind, dt)| [4.74, 9.].map(move |z| (kind, dt, z)))
    {
        return_party_case(kind, dt, start_z);
    }
}

fn return_party_case(kind: usize, dt: f32, start_z: f32) {
    let (cell, home) = (100..500)
        .find_map(|x| {
            let cell = IVec2::new(x, 131);
            site(cell)
                .filter(|_| den_kind(cell) == kind)
                .map(|home| (cell, home))
        })
        .unwrap();
    open_gates().write().unwrap().insert(cell);
    gate_openness().write().unwrap().insert(cell, 1.);
    let mut app = App::new();
    app.insert_resource(Time::<()>::default())
        .init_resource::<crate::app::GameState>()
        .init_resource::<crate::player::camera::CameraRig>()
        .init_resource::<crate::world::snow::Tracks>()
        .init_resource::<Trampling>()
        .init_resource::<Dens>()
        .insert_resource(Art {
            gltfs: vec![],
            models: (0..3)
                .map(|_| Model {
                    scene: default(),
                    graph: default(),
                    clips: vec![],
                })
                .collect(),
            dens: vec![],
            rock: default(),
            iron: default(),
            shale: default(),
            torch: default(),
        })
        .add_systems(Update, (encounters, animate).chain());
    let den = app
        .world_mut()
        .spawn((
            Den {
                cell,
                elapsed: Some(30.),
                emitted: party(mix(seed(cell) ^ 0x656e636f756e7465)).len(),
                open: if kind == 1 { 1. } else { 0. },
            },
            Transform::from_translation(home),
        ))
        .id();
    let gate = app.world_mut().spawn((Gate, Transform::default())).id();
    app.world_mut().entity_mut(den).add_child(gate);
    let residents: Vec<_> = (0..3)
        .map(|variant| {
            let age = EMERGENCE_SECONDS + 1.;
            app.world_mut()
                .spawn((
                    Orc {
                        variant,
                        home,
                        age,
                        mode: 1,
                        step_distance: 0.,
                        left: false,
                        stone_leader: false,
                        retreat: None,
                        navigation: navigation::Navigator::pursuit(),
                        fall_velocity: 0.,
                        engaged: false,
                    },
                    Transform::from_translation(
                        home + Vec3::new((variant as f32 - 1.) * 0.65, GRATE_HEIGHT, start_z),
                    ),
                ))
                .id()
        })
        .collect();
    app.world_mut()
        .resource_mut::<crate::player::camera::CameraRig>()
        .position = home + Vec3::X * (PURSUIT_DISTANCE + 20.);
    for _ in 0..(30. / dt) as usize {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        app.update();
        if residents
            .iter()
            .all(|e| app.world().get::<Orc>(*e).is_none())
            && app.world().get::<Den>(den).unwrap().elapsed.is_none()
        {
            break;
        }
    }
    assert!(
        residents
            .iter()
            .all(|e| app.world().get::<Orc>(*e).is_none()),
        "return stalled: kind={kind} dt={dt} start_z={start_z} gate={} remaining={:?}",
        app.world().get::<Den>(den).unwrap().open,
        residents
            .iter()
            .filter_map(|e| app.world().get::<Transform>(*e).map(|t| (
                t.translation - home,
                app.world()
                    .get::<Orc>(*e)
                    .map(|o| (o.retreat, o.navigation.status))
            )))
            .collect::<Vec<_>>()
    );
    let state = app.world().get::<Den>(den).unwrap();
    assert!(
        state.elapsed.is_none() && state.open == 0.,
        "den did not close/rearm: kind={kind}"
    );
    open_gates().write().unwrap().remove(&cell);
    gate_openness().write().unwrap().remove(&cell);
}

#[test]
fn rootwarren_return_queue_at_high_frame_rate() {
    return_party_case(2, 1. / 144., 4.74);
}

#[test]
fn live_encounter_chases_intruder_before_emergence_and_back_out() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 85);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|home| (cell, home))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<crate::app::GameState>()
            .init_resource::<crate::player::camera::CameraRig>()
            .init_resource::<crate::world::snow::Tracks>()
            .init_resource::<Trampling>()
            .init_resource::<Dens>()
            .insert_resource(Art {
                gltfs: vec![],
                models: (0..3)
                    .map(|_| Model {
                        scene: default(),
                        graph: default(),
                        clips: vec![],
                    })
                    .collect(),
                dens: vec![],
                rock: default(),
                iron: default(),
                shale: default(),
                torch: default(),
            })
            .add_systems(Update, (encounters, animate).chain());
        let den = app
            .world_mut()
            .spawn((
                Den {
                    cell,
                    elapsed: Some(30.),
                    emitted: 3,
                    open: 1.,
                },
                Transform::from_translation(home),
            ))
            .id();
        let gate = app.world_mut().spawn((Gate, Transform::default())).id();
        app.world_mut().entity_mut(den).add_child(gate);
        let residents: Vec<_> = (0..3)
            .map(|variant| {
                let age = (variant + 1) as f32 / 60.;
                app.world_mut()
                    .spawn((
                        Orc {
                            variant,
                            home,
                            age,
                            mode: 1,
                            step_distance: 0.,
                            left: false,
                            stone_leader: false,
                            retreat: None,
                            navigation: navigation::Navigator::pursuit(),
                            fall_velocity: 0.,
                            engaged: false,
                        },
                        Transform::from_translation(home + emergence_position(age)),
                    ))
                    .id()
            })
            .collect();
        let inside = home + Vec3::new(1., tunnel_floor(Vec2::new(1., -10.)).unwrap(), -10.);
        let outside = home + Vec3::new(0., 0., 9.);
        for goal in [inside, outside] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = goal + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
            let mut reached = false;
            for _ in 0..1800 {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(1. / 30.));
                app.update();
                reached = residents.iter().all(|e| {
                    app.world()
                        .get::<Transform>(*e)
                        .unwrap()
                        .translation
                        .distance(goal)
                        < 3.3
                });
                if reached {
                    break;
                }
            }
            assert!(
                reached,
                "kind {kind}, target {:?}, residents {:?}",
                goal - home,
                residents
                    .iter()
                    .map(|e| app.world().get::<Transform>(*e).unwrap().translation - home)
                    .collect::<Vec<_>>()
            );
        }
        if kind != 1 {
            // A player below ground must be able to leave even during a chase.
            app.world_mut().get_mut::<Den>(den).unwrap().open = 0.;
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = inside + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
            for _ in 0..10 {
                app.update();
            }
            assert!(app.world().get::<Den>(den).unwrap().open > 0.);
            // Losing the target far from the mouth is not a return request.
            app.world_mut().get_mut::<Den>(den).unwrap().open = 0.;
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = home + Vec3::X * (PURSUIT_DISTANCE + 20.);
            for e in &residents {
                app.world_mut()
                    .get_mut::<Transform>(*e)
                    .unwrap()
                    .translation = home + Vec3::Z * 15.;
            }
            app.update();
            assert_eq!(app.world().get::<Den>(den).unwrap().open, 0.);
            // A targetless resident arriving at the mouth opens it to enter.
            app.world_mut()
                .get_mut::<Transform>(residents[0])
                .unwrap()
                .translation = home + Vec3::Z * 4.6;
            app.update();
            assert!(app.world().get::<Den>(den).unwrap().open > 0.);
            // Reacquiring a target cancels reopening immediately.
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position =
                home + Vec3::Z * 8. + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
            let before = app.world().get::<Den>(den).unwrap().open;
            app.update();
            assert!(app.world().get::<Den>(den).unwrap().open < before);
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn underground_intruders_interrupt_the_exit_path() {
    let home = Vec3::new(100., 0., 100.);
    let player =
        home + emergence_position(3.) + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
    assert!(intruder_in_den(home, player, false));
    assert!(!intruder_in_den(home, player, true));
    assert!(!intruder_in_den(
        home,
        home + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
        false
    ));
    assert!(!intruder_in_den(home, player + Vec3::X * 30., false));
}
#[test]
fn bundled_orcs_have_valid_skins_and_animation_contract() {
    for i in 0..3 {
        crate::player::avatar::validate_file(&format!(
            "{}/assets/orcs/orc-{i}.glb",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let bytes = std::fs::read(format!(
            "{}/assets/orcs/orc-{i}.glb",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
        let eyes = doc["materials"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "Orc brown eyes")
            .expect("orc-specific brown eyes");
        assert_eq!(eyes["alphaMode"], "MASK");
        let texture = eyes["pbrMetallicRoughness"]["baseColorTexture"]["index"]
            .as_u64()
            .unwrap() as usize;
        let image = doc["textures"][texture]["source"].as_u64().unwrap() as usize;
        assert_eq!(doc["images"][image]["name"], "brown_eye");
        for (name, seconds) in [
            ("Attack", 0.8),
            ("AttackBackhand", 0.9),
            ("AttackOverhead", 1.0),
        ] {
            let clip = doc["animations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["name"] == name)
                .unwrap();
            for sampler in clip["samplers"].as_array().unwrap() {
                let input = sampler["input"].as_u64().unwrap() as usize;
                let end = doc["accessors"][input]["max"][0].as_f64().unwrap();
                assert!(
                    (end - seconds).abs() < 0.02,
                    "{name} must include the complete retimed recovery"
                );
            }
        }
        let outfit = [
            "Bruiser hide jerkin",
            "Ironcap forged breastplate",
            "Raider canvas vest",
        ][i];
        assert!(
            doc["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["name"] == outfit)
        );
        let equipment = [
            "Bruiser equipment /",
            "Ironcap equipment /",
            "Raider equipment /",
        ][i];
        let pieces: Vec<_> = doc["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| {
                n["name"]
                    .as_str()
                    .is_some_and(|name| name.starts_with(equipment))
            })
            .collect();
        assert!(
            (3..=6).contains(&pieces.len()),
            "equipment must remain material-batched"
        );
        assert!(
            pieces.iter().all(|n| n["skin"].is_number()),
            "outfit must follow the skeleton"
        );
        for name in [
            "IdleWatch",
            "IdleWeary",
            "WalkHeavy",
            "AttackBackhand",
            "AttackOverhead",
            "Alert",
            "Push",
        ] {
            assert!(
                doc["animations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["name"] == name),
                "missing {name}"
            );
        }
        let controls: Vec<_> = doc["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, n)| n["name"].as_str().is_some_and(|n| n.starts_with("face_")))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(controls.len(), 9);
        for animation in doc["animations"].as_array().unwrap() {
            if animation["name"] == "Push" {
                let input = animation["samplers"][0]["input"].as_u64().unwrap() as usize;
                let duration = doc["accessors"][input]["max"][0].as_f64().unwrap();
                assert!(
                    (duration - STONE_PUSH_SECONDS as f64).abs() < 0.02,
                    "lift animation and rock trajectory must have the same duration"
                );
            }
            assert!(animation["channels"].as_array().unwrap().iter().any(|c| {
                c["target"]["node"]
                    .as_u64()
                    .is_some_and(|i| controls.contains(&(i as usize)))
            }));
        }
    }
}
#[test]
fn lair_styles_are_stable_and_all_available() {
    let mut counts = [0; 3];
    for x in -30..30 {
        for z in -30..30 {
            let cell = IVec2::new(x, z);
            if site(cell).is_some() {
                let kind = den_kind(cell);
                assert_eq!(kind, den_kind(cell));
                counts[kind] += 1;
            }
        }
    }
    assert!(counts.iter().all(|n| *n > 150));
}
#[test]
fn followers_use_boulder_clearance_not_throw_completion() {
    let at_mouth = Vec3::new(0., ramp_height(0., 2.6), 2.6);
    assert!(boulder_blocks(at_mouth, 0.28, 0.));
    assert!(
        !boulder_blocks(at_mouth, 0.28, 0.60),
        "mouth must clear while rock is still carried"
    );
    let center = stone_position(0.5);
    assert!(boulder_blocks(center - Vec3::Y, 0.28, 0.5));
}
#[test]
fn stone_bearing_face_supports_the_palm_without_a_gap() {
    let (_, _, rock) = den_variant(1);
    let faces: Vec<_> = rock
        .indices
        .chunks_exact(3)
        .filter_map(|ids| {
            let p = ids
                .iter()
                .map(|i| Vec3::from_array(rock.positions[*i as usize]))
                .collect::<Vec<_>>();
            p.iter()
                .all(|v| (v.y - STONE_BEARING_Y).abs() < 0.0001)
                .then(|| [p[0].xz(), p[1].xz(), p[2].xz()])
        })
        .collect();
    for point in [
        Vec2::ZERO,
        Vec2::X * 0.07,
        -Vec2::X * 0.07,
        Vec2::Y * 0.07,
        -Vec2::Y * 0.07,
    ] {
        assert!(
            faces.iter().any(|p| {
                let sides = [0, 1, 2].map(|i| (p[(i + 1) % 3] - p[i]).perp_dot(point - p[i]));
                sides.iter().all(|v| *v >= -0.00001) || sides.iter().all(|v| *v <= 0.00001)
            }),
            "palm contact {point:?} must lie on the flat underside"
        );
    }
    if let Ok(path) = std::env::var("HITHER_STONE_PREVIEW_MESH") {
        std::fs::write(
            path,
            serde_json::to_vec(&serde_json::json!({
                "positions": rock.positions, "indices": rock.indices
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn stone_is_lifted_carried_and_thrown_clear() {
    let closed = gate_pose(1, 0.);
    let lifted = gate_pose(1, 0.25);
    let carried = gate_pose(1, 0.65);
    let flying = gate_pose(1, 0.84);
    let landed = gate_pose(1, 1.);
    assert!(lifted.translation.y > closed.translation.y + 0.15);
    assert!(carried.translation.z > 5.);
    assert!(flying.translation.x > carried.translation.x);
    assert!(landed.translation.x > 2.);
    assert!((carried.translation - stone_contact(0.65)).length() < 0.001);
    let before = (stone_position(0.7) - stone_position(0.7 - 0.0001)) / 0.0001;
    let after = (stone_position(0.7001) - stone_position(0.7)) / 0.0001;
    assert!(
        (before - after).length() < 0.15,
        "release must preserve palm velocity"
    );
}
#[test]
fn lair_variants_have_distinct_finite_geometry_and_walkable_stone_lid() {
    let mut sizes = Vec::new();
    for kind in 0..3 {
        let (earth, metal, door) = den_variant(kind);
        sizes.push(earth.positions.len());
        for g in [&earth, &metal, &door] {
            assert!(kind == 2 || !g.positions.is_empty());
            assert!(g.positions.iter().flatten().all(|v| v.is_finite()));
        }
        if kind == 1 {
            assert!(door.positions.iter().any(|p| p[1] > 1.3));
            assert!(door.positions.iter().any(|p| p[1] < -0.5));
            let closed = gate_pose(kind, 0.);
            let opened = gate_pose(kind, 1.);
            assert!(opened.translation.x > closed.translation.x + 2.);
            assert!(gate_pose(1, 0.6).translation.y > 3.);
            // Quarry hardware must not secretly retain the original grille.
            assert!(
                !metal
                    .positions
                    .iter()
                    .any(|p| p[1] > -0.1 && p[0].abs() < 1.5)
            );
        }
        if kind == 2 {
            assert!(metal.positions.is_empty() && !door.positions.is_empty());
            assert!(earth.positions.iter().all(|p| p[1] < 0.7));
            let feet = Vec3::new(0., ramp_height(0., 1.), 1.);
            assert!(root_plug_blocks(feet, 0.28, 0.));
            assert!(!root_plug_blocks(feet, 0.28, 1.));
        }
    }
    assert!(sizes[0] != sizes[1] && sizes[1] != sizes[2] && sizes[0] != sizes[2]);
}
#[test]
fn den_sites_are_deterministic_sparse_and_in_all_lowland_biomes() {
    let mut biomes = [0; 4];
    let mut count = 0;
    let mut eligible = 0;
    for x in -60..60 {
        for z in -60..60 {
            let c = IVec2::new(x, z);
            let candidate = candidate_position(c, seed(c));
            eligible += usize::from(
                candidate.abs().max_element() >= 20.
                    && crate::world::biome::mountain_amount(candidate) <= 0.15,
            );
            assert_eq!(site(c), site(c));
            if let Some(p) = site(c) {
                count += 1;
                assert!(crate::world::biome::mountain_amount(p.xz()) <= 0.15);
                let snow = crate::world::biome::snow_amount(p.xz()) > 0.5;
                let forest = crate::world::biome::forest_amount(p.xz()) > 0.5;
                biomes[usize::from(snow) * 2 + usize::from(forest)] += 1;
            }
        }
    }
    let density = count as f32 / eligible as f32;
    assert!(
        (0.22..0.28).contains(&density),
        "lowland den density: {density}"
    );
    assert!(biomes.iter().all(|n| *n > 100));
}
#[test]
fn den_meshes_have_finite_geometry() {
    for g in [den_geometry().0, den_geometry().1, den_geometry().2] {
        assert!(!g.positions.is_empty());
        assert!(g.positions.iter().flatten().all(|x| x.is_finite()));
    }
}

#[test]
fn passage_has_standing_clearance_depth_and_continuous_exit() {
    let earth = den_geometry().0;
    assert!(earth.positions.iter().any(|p| p[1] < -6. && p[2] < -14.));
    let mut previous = emergence_position(0.);
    assert!(
        (previous.y
            - floors::mesh(0)
                .rounded_support(previous.xz(), crate::player::movement::PLAYER_RADIUS)
                .unwrap())
        .abs()
            < 0.001
    );
    assert!(previous.y < -5.5);
    for i in 1..=750 {
        let p = emergence_position(i as f32 * 0.01);
        assert!(p.distance(previous) < 0.16);
        assert!(p.z >= previous.z);
        previous = p;
    }
    assert_eq!(previous.y, GRATE_HEIGHT);
    assert!(previous.z > HOLE_LENGTH);
}

#[test]
fn player_can_cross_grate_without_entering_den() {
    let p = preview();
    let start = p + Vec3::new(
        -0.65,
        crate::player::movement::PLAYER_EYE_HEIGHT + GRATE_HEIGHT,
        0.,
    );
    let mut position = start;
    for _ in 0..13 {
        position = crate::player::movement::move_player_with_collisions(position, Vec3::X * 0.1);
        let floor = crate::player::movement::world_support_height(
            position,
            crate::player::movement::PLAYER_RADIUS,
        );
        position.y = floor + crate::player::movement::PLAYER_EYE_HEIGHT;
        if (position.x - p.x).abs() < 2. {
            assert_eq!(floor, GRATE_HEIGHT);
        }
    }
    assert!(position.x > p.x + 0.6);
}

#[test]
fn moving_den_gates_do_not_embed_or_drop_the_player() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 157);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        for x in [-2., -1., 0., 1., 2.] {
            for z in [-3., 0., 1., 2.6, 4.] {
                let mut rig = crate::player::camera::CameraRig {
                    position: home
                        + Vec3::new(x, 3. + crate::player::movement::PLAYER_EYE_HEIGHT, z),
                    jump_height: 3.,
                    grounded: false,
                    ..default()
                };
                for frame in 0..240 {
                    let phase = (frame as f32 / 150.).min(1.);
                    gate_openness().write().unwrap().insert(cell, phase);
                    if phase > 0.72 {
                        open_gates().write().unwrap().insert(cell);
                    } else {
                        open_gates().write().unwrap().remove(&cell);
                    }
                    crate::player::movement::update_jump(&mut rig, &ButtonInput::default(), 0.05);
                    let physics = rig
                        .position
                        .with_y(rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT);
                    assert!(
                        rig.jump_height > -8. && !crate::player::movement::player_collides(physics),
                        "moving gate trapped player: kind={kind} phase={phase} start=({x},{z}) at={:?}",
                        physics - home
                    );
                }
                assert!(rig.grounded, "player never landed: kind={kind} x={x} z={z}");
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn player_can_follow_banks_out_of_all_dens() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 155);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        for side in [-1., 1.] {
            for dt in [1. / 144., 1. / 60., 1. / 20.] {
                let probe = home + Vec3::new(0., -4., -5.5);
                let floor = crate::player::movement::world_support_height(
                    probe,
                    crate::player::movement::PLAYER_RADIUS,
                );
                let mut rig = crate::player::camera::CameraRig {
                    position: probe.with_y(floor + crate::player::movement::PLAYER_EYE_HEIGHT),
                    jump_height: floor,
                    grounded: true,
                    ..default()
                };
                for direction in [Vec3::X * side, Vec3::Z] {
                    let seconds = if direction.z == 0. { 2. } else { 10. };
                    for _ in 0..(seconds / dt) as usize {
                        let physics = rig
                            .position
                            .with_y(rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT);
                        rig.position = crate::player::movement::move_player_with_collisions(
                            physics,
                            direction * 3. * dt,
                        );
                        rig.jump_height =
                            rig.position.y - crate::player::movement::PLAYER_EYE_HEIGHT;
                        crate::player::movement::update_jump(&mut rig, &ButtonInput::default(), dt);
                        assert!(rig.jump_height > -8.);
                    }
                }
                assert!(
                    rig.position.z > home.z + 5.,
                    "bank exit blocked kind={kind} side={side} dt={dt} at={:?}",
                    rig.position - home
                );
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn player_can_walk_out_of_every_den_without_jumping() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 155);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        for dt in [1. / 144., 1. / 60., 1. / 20.] {
            for x in [-1.2, -0.8, -0.4, 0., 0.4, 0.8, 1.2] {
                let probe = home + Vec3::new(x, -4., -7.);
                let floor = crate::player::movement::world_support_height(
                    probe,
                    crate::player::movement::PLAYER_RADIUS,
                );
                let mut rig = crate::player::camera::CameraRig {
                    position: probe.with_y(floor + crate::player::movement::PLAYER_EYE_HEIGHT),
                    jump_height: floor,
                    grounded: true,
                    ..default()
                };
                for _ in 0..(8. / dt) as usize {
                    let physics = rig
                        .position
                        .with_y(rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT);
                    rig.position = crate::player::movement::move_player_with_collisions(
                        physics,
                        Vec3::Z * 3. * dt,
                    );
                    rig.jump_height = rig.position.y - crate::player::movement::PLAYER_EYE_HEIGHT;
                    crate::player::movement::update_jump(&mut rig, &ButtonInput::default(), dt);
                    assert!(
                        (rig.position.x - (home.x + x)).abs() < 0.15,
                        "exit forced into another lane: kind={kind} x={x} at={:?}",
                        rig.position - home
                    );
                    assert!(
                        !crate::player::movement::player_collides(
                            rig.position.with_y(
                                rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT
                            )
                        ),
                        "overlap on exit: kind={kind} dt={dt} at={:?}",
                        rig.position - home
                    );
                }
                assert!(
                    rig.position.z > home.z + 5.,
                    "blocked exit: kind={kind} dt={dt} x={x} at={:?}",
                    rig.position - home
                );
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn embedded_player_recovers_from_den_banks_and_below_floors() {
    for kind in 0..3 {
        let (cell, home) = (100..500)
            .find_map(|x| {
                let cell = IVec2::new(x, 156);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        open_gates().write().unwrap().insert(cell);
        gate_openness().write().unwrap().insert(cell, 1.);
        for z in [-13., -8., -4.1, -3., 0., 3.] {
            let center = if z < -4. { tunnel_center(z).x } else { 0. };
            let width = if z < -6. {
                1.85
            } else {
                ramp_half_width(kind, z).min(1.85)
            };
            for side in [-1., 0., 1.] {
                let at = home
                    + Vec3::new(
                        center + side * (width - crate::player::movement::PLAYER_RADIUS + 0.08),
                        -4.,
                        z,
                    );
                let floor = crate::player::movement::world_support_height(
                    at,
                    crate::player::movement::PLAYER_RADIUS,
                );
                for penetration in [0., 0.4, 4.] {
                    let mut rig = crate::player::camera::CameraRig {
                        position: at.with_y(
                            floor - penetration + crate::player::movement::PLAYER_EYE_HEIGHT,
                        ),
                        jump_height: floor - penetration,
                        grounded: false,
                        ..default()
                    };
                    for _ in 0..100 {
                        crate::player::movement::update_jump(
                            &mut rig,
                            &ButtonInput::default(),
                            0.05,
                        );
                    }
                    let end = rig
                        .position
                        .with_y(rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT);
                    assert!(
                        rig.grounded && !crate::player::movement::player_collides(end),
                        "failed recovery: kind={kind} z={z} side={side} penetration={penetration} at={:?}",
                        end - home
                    );
                    assert!(
                        end.xz().distance(at.xz()) < 1.2,
                        "recovery left the local bank: kind={kind} start={:?} end={:?}",
                        at - home,
                        end - home
                    );
                    assert!(
                        // A correction may leave the open front rim, but
                        // must never jump through the covered tunnel roof.
                        rig.jump_height < 0.
                            || (z >= -4. && (end - home).z >= -4. && !entrance_overlap(end, 0.)),
                        "recovery chose overhead ground kind={kind} z={z} side={side} penetration={penetration} initial_floor={floor} end={:?}",
                        end - home
                    );
                }
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn player_den_floors_hold_during_falls_and_ramp_traversal() {
    for kind in 0..3 {
        let (cell, home) = (100..160)
            .find_map(|x| {
                let cell = IVec2::new(x, 117);
                site(cell)
                    .filter(|_| den_kind(cell) == kind)
                    .map(|p| (cell, p))
            })
            .unwrap();
        for phase in [0., 0.35, 0.7, 1.] {
            if phase > 0.72 {
                open_gates().write().unwrap().insert(cell);
            } else {
                open_gates().write().unwrap().remove(&cell);
            }
            gate_openness().write().unwrap().insert(cell, phase);
            // Drop onto every part of the entrance and the covered passage,
            // including the edges where floor selection changes.
            for ix in -16..=32 {
                for iz in -76..=24 {
                    let at = home + Vec3::new(ix as f32 * 0.2, 0., iz as f32 * 0.2);
                    for height in [3., -0.16, -3., -5.8] {
                        let eye =
                            at + Vec3::Y * (height + crate::player::movement::PLAYER_EYE_HEIGHT);
                        if crate::player::movement::character_collides(
                            eye,
                            crate::player::movement::PLAYER_RADIUS,
                        ) {
                            continue;
                        }
                        let support = crate::player::movement::world_support_height(
                            eye,
                            crate::player::movement::PLAYER_RADIUS,
                        );
                        if height < support - 0.2 {
                            continue;
                        }
                        let mut rig = crate::player::camera::CameraRig {
                            position: eye,
                            jump_height: height,
                            grounded: false,
                            ..default()
                        };
                        let mut stable_frames = 0;
                        for _ in 0..100 {
                            crate::player::movement::update_jump(
                                &mut rig,
                                &ButtonInput::default(),
                                0.05,
                            );
                            stable_frames = if rig.grounded { stable_frames + 1 } else { 0 };
                            // The gate is static for this drop. Prove stable
                            // support, then advance to the next location.
                            if stable_frames == 3 {
                                break;
                            }
                        }
                        assert!(
                            rig.grounded && rig.jump_height > -8.,
                            "kind={kind} phase={phase} x={} z={} start={height} final={}",
                            ix as f32 * 0.2,
                            iz as f32 * 0.2,
                            rig.jump_height
                        );
                        let settled = rig
                            .position
                            .with_y(rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT);
                        assert!(
                            !crate::player::movement::player_collides(settled),
                            "trapped after landing: kind={kind} phase={phase} start={:?} end={:?}",
                            eye - home,
                            settled - home
                        );
                        assert!(
                            (0..16).any(|direction| {
                                let angle = direction as f32 * TAU / 16.;
                                let end = crate::player::movement::move_player_with_collisions(
                                    settled,
                                    Vec3::new(angle.cos(), 0., angle.sin()) * 0.15,
                                );
                                end.xz().distance(settled.xz()) > 0.05
                            }),
                            "cannot move after landing: kind={kind} phase={phase} at={:?}",
                            settled - home
                        );
                    }
                }
            }
        }
        // Exercise the real horizontal movement + gravity order while
        // jumping and crossing the entrance and ramp seams in both directions.
        for dt in [1. / 144., 1. / 60., 1. / 20.] {
            for x in [-0.25, 0., 0.25] {
                for direction in [-1., 1.] {
                    let z = if direction < 0. { 5. } else { -7. };
                    let eye = home
                        + Vec3::new(
                            x,
                            if z > 0. {
                                crate::player::movement::PLAYER_EYE_HEIGHT
                            } else {
                                -6. + crate::player::movement::PLAYER_EYE_HEIGHT
                            },
                            z,
                        );
                    let floor = crate::player::movement::world_support_height(
                        eye,
                        crate::player::movement::PLAYER_RADIUS,
                    );
                    let mut rig = crate::player::camera::CameraRig {
                        position: eye.with_y(floor + crate::player::movement::PLAYER_EYE_HEIGHT),
                        jump_height: floor,
                        grounded: true,
                        ..default()
                    };
                    for frame in 0..(8. / dt) as usize {
                        let physics = rig
                            .position
                            .with_y(rig.jump_height + crate::player::movement::PLAYER_EYE_HEIGHT);
                        rig.position = crate::player::movement::move_player_with_collisions(
                            physics,
                            Vec3::Z * direction * 3. * dt,
                        );
                        rig.jump_height =
                            rig.position.y - crate::player::movement::PLAYER_EYE_HEIGHT;
                        let mut keys = ButtonInput::default();
                        if frame % (1. / dt) as usize == 0 {
                            keys.press(KeyCode::Space);
                        }
                        crate::player::movement::update_jump(&mut rig, &keys, dt);
                        assert!(
                            rig.jump_height > -8.,
                            "moving fall-through: kind={kind} dt={dt} x={x} direction={direction} position={:?}",
                            rig.position - home
                        );
                    }
                }
            }
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}

#[test]
fn open_hatch_removes_support_and_player_lands_below_ground() {
    let (cell, p) = (30..60)
        .find_map(|x| {
            site(IVec2::new(x, 30))
                .filter(|_| den_kind(IVec2::new(x, 30)) == 0)
                .map(|p| (IVec2::new(x, 30), p))
        })
        .unwrap();
    let mut rig = crate::player::camera::CameraRig {
        position: p + Vec3::Y * (crate::player::movement::PLAYER_EYE_HEIGHT + GRATE_HEIGHT),
        jump_height: GRATE_HEIGHT,
        grounded: true,
        ..default()
    };
    assert_eq!(surface_height(rig.position, 0.28), Some(GRATE_HEIGHT));
    open_gates().write().unwrap().insert(cell);
    assert!(surface_height(rig.position, 0.28).unwrap() < -2.);
    assert!(surface_height(rig.position + Vec3::X * 1.3, 0.28).is_some_and(|y| y < 0.));
    for _ in 0..300 {
        crate::player::movement::update_jump(&mut rig, &ButtonInput::default(), 0.01);
    }
    assert!(rig.grounded);
    assert!(
        (rig.jump_height
            - floors::mesh(den_kind(cell))
                .rounded_support(Vec2::ZERO, 0.28)
                .unwrap())
        .abs()
            < 0.001
    );
    open_gates().write().unwrap().remove(&cell);
    // Closing a hatch above an underground player must not teleport them.
    assert!(surface_height(rig.position, 0.28).unwrap() < -2.);
    assert!(underground_collision(p + Vec3::new(2.2, -1., 0.), 0.28));
}

#[test]
fn whole_party_spawns_in_one_encounter_update() {
    for kind in 0..3 {
        let (cell, home) = (194..204)
            .flat_map(|z| (100..500).map(move |x| IVec2::new(x, z)))
            .find_map(|cell| {
                site(cell)
                    .filter(|_| {
                        den_kind(cell) == kind
                            && party(mix(seed(cell) ^ 0x656e636f756e7465)).len() == 3
                    })
                    .map(|home| (cell, home))
            })
            .unwrap_or_else(|| panic!("missing three-resident fixture for den {kind}"));
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<crate::app::GameState>()
            .insert_resource(crate::player::camera::CameraRig {
                position: home + Vec3::new(0., crate::player::movement::PLAYER_EYE_HEIGHT, 6.),
                ..default()
            })
            .init_resource::<Dens>()
            .insert_resource(Art {
                gltfs: vec![],
                models: (0..3)
                    .map(|_| Model {
                        scene: default(),
                        graph: default(),
                        clips: vec![],
                    })
                    .collect(),
                dens: vec![],
                rock: default(),
                iron: default(),
                shale: default(),
                torch: default(),
            })
            .add_systems(Update, encounters);
        let den = app
            .world_mut()
            .spawn((
                Den {
                    cell,
                    elapsed: None,
                    emitted: 0,
                    open: 0.,
                },
                Transform::from_translation(home),
            ))
            .id();
        let gate = app.world_mut().spawn((Gate, Transform::default())).id();
        app.world_mut().entity_mut(den).add_child(gate);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1. / 60.));
        app.update();
        let expected = party(mix(seed(cell) ^ 0x656e636f756e7465)).len();
        let count = app.world_mut().query::<&Orc>().iter(app.world()).count();
        assert_eq!(
            count, expected,
            "den kind {kind} did not spawn the entire party"
        );
        app.update();
        assert_eq!(
            app.world_mut().query::<&Orc>().iter(app.world()).count(),
            expected
        );
        app.init_resource::<crate::world::snow::Tracks>()
            .init_resource::<Trampling>()
            .add_systems(Update, animate.after(encounters));
        let inside = home + Vec3::new(1., tunnel_floor(Vec2::new(1., -10.)).unwrap(), -10.);
        for goal in [home + Vec3::Z * 9., inside, home + Vec3::Z * 9.] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = goal + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT;
            let mut reached = false;
            for _ in 0..2700 {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(1. / 30.));
                app.update();
                let mut q = app.world_mut().query_filtered::<&Transform, With<Orc>>();
                reached = q
                    .iter(app.world())
                    .all(|t| t.translation.distance(goal) < 3.3);
                if reached {
                    break;
                }
            }
            let mut q = app.world_mut().query_filtered::<&Transform, With<Orc>>();
            assert!(
                reached,
                "new encounter kind={kind} target={:?} actors={:?}",
                goal - home,
                q.iter(app.world())
                    .map(|t| t.translation - home)
                    .collect::<Vec<_>>()
            );
        }
        open_gates().write().unwrap().remove(&cell);
        gate_openness().write().unwrap().remove(&cell);
    }
}
