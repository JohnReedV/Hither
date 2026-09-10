use super::*;
struct Mountain;
impl Geometry for Mountain {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        vec![p.x * 0.75]
    }
    fn body_clear(&self, _: Vec3, _: Agent) -> bool {
        true
    }
}
#[test]
fn invalid_old_destination_does_not_pin_a_changed_queue_slot() {
    struct ObstructedMountain;
    impl Geometry for ObstructedMountain {
        fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
            vec![p.x * 0.75]
        }
        fn body_clear(&self, p: Vec3, _: Agent) -> bool {
            (p.x - 30.).abs() > 0.5 || p.z.abs() > 1.
        }
    }
    for old in [Vec3::new(30., 22.5, 0.), Vec3::new(25., 20., 0.)] {
        let mut graph = Graph::default();
        let mut nav = Navigator::default();
        let agent = Agent::default();
        nav.steer(
            &mut graph,
            &ObstructedMountain,
            agent,
            Vec3::ZERO,
            old,
            0.016,
            &mut 1,
        );
        assert_eq!(nav.search.as_ref().unwrap().goal, old);
        let replacement = Vec3::new(20., 15., 0.);
        // Beyond shortcut range: replacing this goal requires a new search.
        nav.steer(
            &mut graph,
            &ObstructedMountain,
            agent,
            Vec3::ZERO,
            replacement,
            0.016,
            &mut 0,
        );
        assert_eq!(nav.search.as_ref().unwrap().goal, replacement);
        for _ in 0..100 {
            nav.steer(
                &mut graph,
                &ObstructedMountain,
                agent,
                Vec3::ZERO,
                replacement,
                0.016,
                &mut 192,
            );
            if nav.status == Status::Complete {
                break;
            }
        }
        assert_eq!(nav.status, Status::Complete);
    }
}
#[test]
fn moving_destination_does_not_restart_in_progress_search() {
    let agent = Agent::default();
    let mut graph = Graph::default();
    let mut nav = Navigator::default();
    let original = Vec3::new(30., 22.5, 0.);
    nav.steer(
        &mut graph,
        &Mountain,
        agent,
        Vec3::ZERO,
        original,
        0.016,
        &mut 1,
    );
    let count = nav.search.as_ref().unwrap().costs.len();
    nav.steer(
        &mut graph,
        &Mountain,
        agent,
        Vec3::ZERO,
        original * 2.,
        0.016,
        &mut 0,
    );
    assert_eq!(nav.search.as_ref().unwrap().goal, original);
    assert_eq!(nav.search.as_ref().unwrap().costs.len(), count);
    for _ in 0..100 {
        nav.steer(
            &mut graph,
            &Mountain,
            agent,
            Vec3::ZERO,
            original * 2.,
            0.016,
            &mut 192,
        );
        if nav.status == Status::Complete {
            return;
        }
    }
    panic!("moving goal prevented search completion");
}
#[test]
fn kilometer_mountain_route_uses_coarse_connections() {
    let mut graph = Graph::default();
    let agent = Agent::default();
    let goal = Vec3::new(1024., 768., 0.);
    let mut search = Search::new(&mut graph, &Mountain, agent, Vec3::ZERO, goal);
    for _ in 0..1000 {
        let (status, path) = search.advance(&mut graph, &Mountain, agent, &mut 192);
        if status != Status::Searching {
            assert_eq!(status, Status::Complete);
            assert_eq!(path.back(), Some(&goal));
            assert!(
                path.len() < 200,
                "coarse connections should avoid thousands of fine waypoints"
            );
            return;
        }
    }
    panic!("mountain route did not finish");
}
