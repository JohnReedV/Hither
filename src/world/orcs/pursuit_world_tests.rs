//! World-geometry and live ECS checks exercise the production AI system.
use super::*;
use crate::world::navigation::{self as nav, Agent, Geometry as _};

#[test]
fn engagement_survives_distance_and_elevation_but_spectating_cancels_it() {
    for height in [-12., 0., 0.1, 2., 20., 80.] {
        for distance in [49., 50., 50.01, 64., 96., 160., 192.] {
            let target = Vec3::new(distance, height, 0.);
            assert!(can_pursue(Vec3::ZERO, target, false, true));
            assert_eq!(
                can_pursue(Vec3::ZERO, target, false, false),
                distance <= 50.
            );
            assert!(!can_pursue(Vec3::ZERO, target, true, true));
        }
    }
    assert!(!can_pursue(Vec3::ZERO, Vec3::X * 193., false, true));
}

#[test]
fn engagement_does_not_flicker_when_player_runs_along_acquisition_boundary() {
    let mut engaged = false;
    for distance in [49., 50.1, 49.9, 60., 80., 160., 190., 51.] {
        engaged = can_pursue(Vec3::ZERO, Vec3::new(distance, 3., 0.), false, engaged);
        assert!(engaged);
    }
    engaged = can_pursue(Vec3::ZERO, Vec3::X * 200., false, engaged);
    assert!(!engaged);
    assert!(!can_pursue(Vec3::ZERO, Vec3::X * 80., false, engaged));
    assert!(can_pursue(Vec3::ZERO, Vec3::X * 45., false, engaged));
}

fn hillside(distance: f32, uphill: bool) -> (Vec3, Vec3) {
    type Fixtures = std::sync::Mutex<HashMap<(u32, bool), (Vec3, Vec3)>>;
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    let key = (distance.to_bits(), uphill);
    if let Some(points) = FIXTURES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .get(&key)
    {
        return *points;
    }
    let agent = Agent::default();
    for x in -16..16 {
        for z in -16..16 {
            let at = Vec2::new(x as f32 * 64. + 29., z as f32 * 64. + 17.);
            let gradient = crate::world::terrain::gradient(at);
            if !(0.04..0.55).contains(&gradient.length()) {
                continue;
            }
            let p = Vec3::new(at.x, crate::world::terrain::height(at), at.y);
            if !navigation::World.body_clear(p, agent) {
                continue;
            }
            for angle in [0_f32, 0.5, -0.5, 1., -1.] {
                let direction = Vec2::from_angle(angle).rotate(gradient.normalize())
                    * if uphill { 1. } else { -1. };
                let end = at + direction * distance;
                let q = Vec3::new(end.x, crate::world::terrain::height(end), end.y);
                if (q.y - p.y) * if uphill { 1. } else { -1. } < 0.12 {
                    continue;
                }
                if nav::walk(&navigation::World, agent, p, end)
                    .is_some_and(|v| v.distance(q) < 0.005)
                {
                    FIXTURES.get().unwrap().lock().unwrap().insert(key, (p, q));
                    return (p, q);
                }
            }
        }
    }
    panic!("no certified hillside fixture for {distance}m uphill={uphill}");
}

fn live_app(start: Vec3, goal: Vec3, count: usize) -> (App, Vec<Entity>) {
    let mut app = App::new();
    app.insert_resource(Time::<()>::default())
        .init_resource::<crate::app::GameState>()
        .insert_resource(crate::player::camera::CameraRig {
            position: goal + Vec3::Y * crate::player::movement::PLAYER_EYE_HEIGHT,
            ..default()
        })
        .init_resource::<crate::world::snow::Tracks>()
        .init_resource::<Trampling>()
        .insert_resource(Art {
            gltfs: vec![],
            models: vec![],
            dens: vec![],
            rock: default(),
            iron: default(),
            shale: default(),
            torch: default(),
        })
        .add_systems(Update, animate);
    let direction = (goal.xz() - start.xz()).normalize();
    let entities = (0..count)
        .map(|i| {
            let at = start.xz() + direction * (i as f32 * 0.7);
            let position = Vec3::new(at.x, crate::world::terrain::height(at), at.y);

            app.world_mut()
                .spawn((
                    Orc {
                        variant: 0,
                        home: start,
                        age: EMERGENCE_SECONDS,
                        mode: 1,
                        step_distance: 0.,
                        left: false,
                        stone_leader: false,
                        retreat: None,
                        navigation: navigation::Navigator::pursuit(),
                        fall_velocity: 0.,
                        engaged: true,
                    },
                    Transform::from_translation(position),
                ))
                .id()
        })
        .collect();
    (app, entities)
}

fn live_hillside(distance: f32, fps: f32, uphill: bool) {
    let (start, goal) = hillside(distance, uphill);
    let (mut app, entities) = live_app(start, goal, 1);
    let entity = entities[0];
    let dt = 1. / fps;
    let mut anchor = start;
    let mut stalled = 0.;
    for frame in 0..((distance / 1.2 + 8.) * fps) as usize {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        app.update();
        let p = app.world().get::<Transform>(entity).unwrap().translation;
        let orc = app.world().get::<Orc>(entity).unwrap();
        assert!(orc.engaged && orc.retreat.is_none(), "lost player at {p:?}");
        assert!(
            navigation::World.body_clear(p, Agent::default()),
            "embedded on hill {p:?}"
        );
        if navigation::attack_reachable(p, goal) {
            return;
        }
        if anchor.distance(p) > 0.04 {
            anchor = p;
            stalled = 0.;
        } else {
            stalled += dt;
        }
        assert!(
            stalled < 0.75,
            "live pursuit stalled {stalled}s: from={start:?} at={p:?} goal={goal:?} status={:?} frame={frame}",
            orc.navigation.status
        );
    }
    panic!("live hillside pursuit did not arrive {start:?} -> {goal:?}");
}
#[test]
fn live_downhill_pursuit_settles_and_enters_attack_range() {
    live_hillside(12., 60., false);
}

fn outrunning_party(fps: f32) {
    let (start, end) = hillside(96., true);
    let direction = (end.xz() - start.xz()).normalize();
    let (mut app, entities) = live_app(start, end, 3);
    let mut anchors: Vec<_> = entities
        .iter()
        .map(|e| app.world().get::<Transform>(*e).unwrap().translation)
        .collect();
    let mut stalled = [0_f32; 3];
    let mut timings = Vec::new();
    for frame in 0..(20. * fps) as usize {
        let t = frame as f32 / fps;
        let at = start.xz() + direction * (10. + 4. * t);
        let jump = (t * 3.).sin().max(0.) * 0.7;
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position = Vec3::new(
            at.x,
            crate::world::terrain::height(at) + jump + crate::player::movement::PLAYER_EYE_HEIGHT,
            at.y,
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(1. / fps));
        let started = std::time::Instant::now();
        app.update();
        timings.push(started.elapsed().as_secs_f64() * 1000.);
        for (i, entity) in entities.iter().enumerate() {
            let p = app.world().get::<Transform>(*entity).unwrap().translation;
            let orc = app.world().get::<Orc>(*entity).unwrap();
            assert!(orc.engaged && orc.retreat.is_none());
            if p.distance(anchors[i]) > 0.04 {
                anchors[i] = p;
                stalled[i] = 0.;
            } else {
                stalled[i] += 1. / fps;
            }
            assert!(
                stalled[i] < 0.75,
                "resident {i} stopped while player ran/jumped uphill: {p:?} fps={fps}"
            );
        }
    }
    timings.sort_by(f64::total_cmp);
    eprintln!(
        "live party CPU ms: median={:.3} p95={:.3} max={:.3}",
        timings[timings.len() / 2],
        timings[timings.len() * 95 / 100],
        timings[timings.len() - 1]
    );
    for entity in entities {
        let p = app.world().get::<Transform>(entity).unwrap().translation;
        assert!(
            (p.xz() - start.xz()).dot(direction) > 20.,
            "party made insufficient progress: {p:?}"
        );
    }
}
#[test]
fn full_party_keeps_chasing_a_faster_jumping_player_uphill_30fps() {
    outrunning_party(30.);
}
