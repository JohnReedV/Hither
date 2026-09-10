use super::*;
struct SteppedCorner;
impl Geometry for SteppedCorner {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        if p.y.abs() <= 0.04 && p.x <= 0.04 {
            vec![0.]
        } else if p.x.abs() <= 0.04 && p.y > 0.04 {
            vec![0.18]
        } else {
            vec![]
        }
    }
    fn body_clear(&self, p: Vec3, a: Agent) -> bool {
        !self.floors(p.xz(), a).is_empty()
    }
}
#[test]
fn close_waypoint_is_kept_until_the_next_elevation_segment_is_reachable() {
    let agent = Agent::default();
    let world = SteppedCorner;
    let from = Vec3::new(-0.1, 0., 0.);
    let corner = Vec3::ZERO;
    let goal = Vec3::new(0., 0.18, 1.);
    assert!(clear(&world, agent, from, corner));
    assert!(clear(&world, agent, corner, goal));
    assert!(!clear(&world, agent, from, goal));
    for dt in [1. / 30., 1. / 60., 1. / 144., 1. / 240.] {
        let mut graph = Graph::default();
        let mut nav = Navigator {
            path: VecDeque::from([corner, goal]),
            goal: Some(goal),
            agent: Some(agent),
            retry: 1.,
            status: Status::Complete,
            ..Default::default()
        };
        let mut p = from;
        assert_eq!(
            nav.steer(&mut graph, &world, agent, p, goal, dt, &mut 192),
            Some(corner)
        );
        for _ in 0..1000 {
            let Some(next) = nav.steer(&mut graph, &world, agent, p, goal, dt, &mut 192) else {
                break;
            };
            let delta = next.xz() - p.xz();
            let to = p.xz() + delta.normalize_or_zero() * delta.length().min(1.55 * dt);
            p = walk(&world, agent, p, to)
                .unwrap_or_else(|| panic!("retained corner {p:?} -> {next:?}, dt {dt}"));
        }
        assert!(p.distance(goal) < 0.15, "stopped at {p:?} with dt {dt}");
    }
}

#[test]
fn completed_search_cannot_send_a_moving_actor_back_to_its_old_start() {
    // Finish a route just as its slice expires. The actor has already followed
    // an older corridor two metres beyond the position where this search began.
    struct SlowFinish(std::cell::Cell<usize>);
    impl Geometry for SlowFinish {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0.]
        }
        fn body_clear(&self, p: Vec3, _: Agent) -> bool {
            if p.x == 4. {
                self.0.set(self.0.get() + 1);
                if self.0.get() == 2 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
            true
        }
    }
    let agent = Agent::default();
    let goal = Vec3::X * 4.;
    let nodes: Vec<_> = (0..=4)
        .map(|x| Node::new(IVec2::new(x * 4, 0), 0.))
        .collect();
    let mut graph = Graph::default();
    let search = Search {
        start: None,
        open: BinaryHeap::from([Open {
            node: nodes[4],
            cost: 4.,
            score: 4.,
        }]),
        pending: None,
        costs: HashMap::from([(nodes[4], 4.)]),
        parents: (1..=4).map(|i| (nodes[i], nodes[i - 1])).collect(),
        goal,
        revision: 0,
        agent,
    };
    let mut navigator = Navigator {
        path: VecDeque::from([goal]),
        search: Some(search),
        goal: Some(goal),
        agent: Some(agent),
        retry: 1.,
        ..Default::default()
    };
    let world = SlowFinish(Default::default());
    graph.begin_slice(std::time::Duration::from_millis(10));
    let next = navigator.steer(
        &mut graph,
        &world,
        agent,
        Vec3::X * 2.,
        goal,
        0.016,
        &mut 192,
    );
    assert!(
        next.is_none_or(|p| p.x >= 2.),
        "stale route reversed pursuit: {next:?}"
    );
    // Once time is available, attach ahead even if the target also moved.
    graph.deadline = None;
    let next = navigator.steer(
        &mut graph,
        &world,
        agent,
        Vec3::X * 2.1,
        goal + Vec3::X,
        0.016,
        &mut 192,
    );
    assert!(
        next.is_some_and(|p| p.x > 2.1),
        "could not join ahead: {next:?}"
    );
}

#[test]
fn two_point_oscillation_is_not_counted_as_forward_progress() {
    struct Flat;
    impl Geometry for Flat {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0.]
        }
        fn body_clear(&self, _: Vec3, _: Agent) -> bool {
            true
        }
    }
    let agent = Agent::default();
    let goal = Vec3::X * 30.;
    let mut navigator = Navigator {
        path: VecDeque::from([goal]),
        goal: Some(goal),
        agent: Some(agent),
        status: Status::Complete,
        ..Default::default()
    };
    let mut graph = Graph::default();
    for frame in 0..90 {
        let p = Vec3::X * if frame % 2 == 0 { 1. } else { 2. };
        navigator.steer(&mut graph, &Flat, agent, p, goal, 1. / 60., &mut 0);
    }
    assert!(
        navigator.search.is_some(),
        "back-and-forth motion never triggered replanning"
    );
}

#[test]
fn long_edge_yields_without_caching_an_unfinished_sweep_as_blocked() {
    struct SlowGround(std::cell::Cell<usize>);
    impl Geometry for SlowGround {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0.]
        }
        fn body_clear(&self, _: Vec3, _: Agent) -> bool {
            self.0.set(self.0.get() + 1);
            std::thread::sleep(std::time::Duration::from_micros(100));
            true
        }
    }
    let world = SlowGround(Default::default());
    let mut graph = Graph::default();
    let agent = Agent::default();
    let a = Node::new(IVec2::ZERO, 0.);
    let b = Node::new(IVec2::new(64, 0), 0.);
    graph.begin_slice(std::time::Duration::from_millis(1));
    assert_eq!(graph.edge_probe(&world, agent, a, b, true), None);
    assert!(
        world.0.get() < 30,
        "a complete long sweep ran after its deadline"
    );
    assert!(
        graph.edges.is_empty(),
        "unfinished query was cached as a collision"
    );
    assert_eq!(graph.probes.len(), 1);
    graph.deadline = None;
    assert_eq!(graph.edge_probe(&world, agent, a, b, true), Some(true));
    assert!(graph.probes.is_empty());
}

#[test]
fn initial_attachments_yield_and_resume_without_repeating_completed_columns() {
    struct Counted(std::cell::Cell<usize>);
    impl Geometry for Counted {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            self.0.set(self.0.get() + 1);
            vec![0.]
        }
        fn body_clear(&self, _: Vec3, _: Agent) -> bool {
            true
        }
    }
    let world = Counted(std::cell::Cell::new(0));
    let agent = Agent::default();
    let mut graph = Graph::default();
    let mut search = Search::new(&mut graph, &world, agent, Vec3::ZERO, Vec3::X * 3.);
    let initial = world.0.get();
    graph.begin_slice(std::time::Duration::ZERO);
    assert_eq!(
        search.advance(&mut graph, &world, agent, &mut 192).0,
        Status::Searching
    );
    assert_eq!(world.0.get(), initial);
    assert_eq!(search.start.as_ref().unwrap().column, 0);
    graph.deadline = None;
    // A one-unit allowance discovers one column, then preserves its candidate.
    search.advance(&mut graph, &world, agent, &mut 1);
    assert_eq!(search.start.as_ref().unwrap().column, 1);
    assert_eq!(search.start.as_ref().unwrap().nodes.len(), 1);
    for _ in 0..100 {
        let (status, path) = search.advance(&mut graph, &world, agent, &mut 13);
        if status == Status::Complete {
            assert_eq!(path.back(), Some(&(Vec3::X * 3.)));
            return;
        }
        assert_eq!(status, Status::Searching);
    }
    panic!("resumed initialization lost its frontier");
}

#[test]
fn layered_resident_starts_on_its_floor_and_still_routes_around_a_barrier() {
    struct Floors;
    impl Geometry for Floors {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0., 3.]
        }
        fn body_clear(&self, p: Vec3, a: Agent) -> bool {
            !((1.5 - a.radius..2.5 + a.radius).contains(&p.x) && p.z.abs() < 0.3 + a.radius)
        }
    }
    let mut graph = Graph::default();
    let mut navigator = Navigator::layered_pursuit();
    let agent = Agent {
        radius: 0.12,
        height: 0.44,
        step: 0.09,
        slope: 0.8,
    };
    let goal = Vec3::X * 4.;
    let mut p = Vec3::ZERO;
    assert_eq!(
        navigator.steer(&mut graph, &Floors, agent, p, goal, 1. / 30., &mut 180),
        Some(Vec3::X)
    );
    for _ in 0..1000 {
        if let Some(next) = navigator.steer(&mut graph, &Floors, agent, p, goal, 1. / 30., &mut 180)
        {
            let delta = next.xz() - p.xz();
            if let Some(to) = slide(
                &Floors,
                agent,
                p,
                p.xz() + delta.normalize_or_zero() * delta.length().min(0.18 / 30.),
                |_| true,
            ) {
                p = to;
            }
        }
        assert_eq!(p.y, 0.);
        if p.distance(goal) < 0.15 {
            return;
        }
    }
    panic!(
        "resident stalled instead of detouring: {p}; path={:?}; status={:?}; prefix={} search={} ready={:?}",
        navigator.path,
        navigator.status,
        navigator.prefix,
        navigator.search.is_some(),
        navigator.ready
    );
}
