use super::*;
struct Hill {
    slope: f32,
}
impl Geometry for Hill {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        vec![p.x * self.slope]
    }
    fn body_clear(&self, _: Vec3, _: Agent) -> bool {
        true
    }
}
#[test]
fn slope_limit_is_identical_for_search_edges_and_small_motion_steps() {
    let agent = Agent::default();
    for distance in [0.01, 0.025, 0.08, 0.5] {
        assert!(walk(&Hill { slope: 0.75 }, agent, Vec3::ZERO, Vec2::X * distance).is_some());
        assert!(walk(&Hill { slope: 2. }, agent, Vec3::ZERO, Vec2::X * distance).is_none());
    }
}
#[test]
fn covered_lower_floor_does_not_hide_a_reachable_step() {
    struct Layers;
    impl Geometry for Layers {
        fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
            if p.x >= 0. { vec![0., 0.15] } else { vec![0.] }
        }
        fn body_clear(&self, p: Vec3, _: Agent) -> bool {
            p.x < 0. || p.y >= 0.149
        }
    }
    for increment in [0.01, 0.025, 0.08, 0.4] {
        let mut p = Vec3::new(-0.2, 0., 0.);
        while p.x < 0.2 {
            p = walk(
                &Layers,
                Agent::default(),
                p,
                Vec2::new((p.x + increment).min(0.2), 0.),
            )
            .expect("choose clear upper step instead of buried floor");
        }
        assert_eq!(p.y, 0.15);
    }
}
struct Stair;
impl Geometry for Stair {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        vec![if p.x >= 0. { 0.18 } else { 0. }]
    }
    fn body_clear(&self, _: Vec3, _: Agent) -> bool {
        true
    }
}
#[test]
fn ordinary_stair_is_walkable_at_small_and_large_timesteps() {
    let agent = Agent::default();
    for start in [-0.01, -0.025, -0.08, -0.5] {
        assert!(
            walk(
                &Stair,
                agent,
                Vec3::new(start, 0., 0.),
                Vec2::new(-start, 0.)
            )
            .is_some()
        );
    }
}
struct RoughGround {
    height: f32,
    hill: f32,
    ceiling: f32,
}
impl Geometry for RoughGround {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        // Closely spaced triangular ridges with steep sides, like small roots.
        let ridge = 1. - ((p.x / 0.12).rem_euclid(2.) - 1.).abs();
        vec![p.x * self.hill + ridge * self.height]
    }
    fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
        p.y + agent.height <= self.ceiling
    }
}
#[test]
fn small_roughness_is_walkable_without_an_attack_or_replan() {
    let agent = Agent::default();
    for height in [0.04, 0.10, 0.18] {
        let world = RoughGround {
            height,
            hill: 0.,
            ceiling: 10.,
        };
        let from = Vec3::ZERO;
        let goal = Vec3::new(2.4, 0., 0.);
        assert!(
            clear(&world, agent, from, goal),
            "graph edge, height={height}"
        );
        for dt in [1. / 144., 1. / 60., 1. / 20.] {
            let mut p = from;
            let mut velocity = 0.;
            for _ in 0..1000 {
                assert!(settle(&world, agent, &mut p, &mut velocity, dt));
                let x = (p.x + 1.55 * dt).min(goal.x);
                p = walk(&world, agent, p, Vec2::new(x, 0.))
                    .unwrap_or_else(|| panic!("stalled at {p:?}, height={height}, dt={dt}"));
                if x == goal.x {
                    break;
                }
            }
            assert!(p.distance(goal) < 0.001);
            // Also recover when the actor starts directly on a ridge crest.
            let crest = Vec3::new(0.12, height, 0.);
            assert!(walk(&world, agent, crest, Vec2::new(0.12 - dt, 0.)).is_some());
        }
    }
}
#[test]
fn roughness_allowance_preserves_slope_and_body_limits() {
    let agent = Agent::default();
    for world in [
        RoughGround {
            height: 0.06,
            hill: 2.,
            ceiling: 10.,
        },
        RoughGround {
            height: 0.4,
            hill: 0.,
            ceiling: 10.,
        },
        RoughGround {
            height: 0.18,
            hill: 0.,
            ceiling: agent.height + 0.05,
        },
    ] {
        assert!(walk(&world, agent, Vec3::ZERO, Vec2::new(0.24, 0.)).is_none());
    }
}
#[test]
fn edge_cache_reuses_geometry_and_invalidates_with_world_changes() {
    struct Counted {
        calls: std::cell::Cell<usize>,
        blocked: std::cell::Cell<bool>,
    }
    impl Geometry for Counted {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            self.calls.set(self.calls.get() + 1);
            vec![0.]
        }
        fn body_clear(&self, _: Vec3, agent: Agent) -> bool {
            !self.blocked.get() && agent.radius < 1.
        }
    }
    let world = Counted {
        calls: std::cell::Cell::new(0),
        blocked: std::cell::Cell::new(false),
    };
    let mut graph = Graph::default();
    let a = Node::new(IVec2::ZERO, 0.);
    let b = Node::new(IVec2::X, 0.);
    let agent = Agent::default();
    assert!(graph.edge(&world, agent, a, b, false));
    let calls = world.calls.get();
    assert!(graph.edge(&world, agent, a, b, false));
    assert_eq!(calls, world.calls.get());
    assert!(!graph.edge(
        &world,
        Agent {
            radius: 1.1,
            ..agent
        },
        a,
        b,
        false
    ));
    world.blocked.set(true);
    graph.invalidate(Vec2::splat(-1.), Vec2::splat(1.));
    assert!(!graph.edge(&world, agent, a, b, false));
    world.blocked.set(false);
    graph.reset();
    assert!(graph.edge(&world, agent, a, b, false));
}

#[test]
fn optional_smoothing_is_amortized_but_geometry_changes_replan_immediately() {
    struct Counted {
        calls: std::cell::Cell<usize>,
        barrier: std::cell::Cell<bool>,
    }
    impl Geometry for Counted {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            self.calls.set(self.calls.get() + 1);
            vec![0.]
        }
        fn body_clear(&self, p: Vec3, _: Agent) -> bool {
            self.calls.set(self.calls.get() + 1);
            !self.barrier.get() || !(0.5..1.5).contains(&p.x) || p.z >= 0.5
        }
    }
    let world = Counted {
        calls: 0.into(),
        barrier: true.into(),
    };
    let agent = Agent::default();
    let mut graph = Graph::default();
    let goal = Vec3::new(2., 0., 0.);
    let corner = Vec3::new(0., 0., 1.);
    let mut nav = Navigator {
        path: VecDeque::from([corner, Vec3::new(2., 0., 1.), goal]),
        goal: Some(goal),
        agent: Some(agent),
        retry: 1.5,
        ..default()
    };
    assert_eq!(
        nav.steer(&mut graph, &world, agent, Vec3::ZERO, goal, 0.016, &mut 192),
        Some(corner)
    );
    let first = world.calls.replace(0);
    assert_eq!(
        nav.steer(&mut graph, &world, agent, Vec3::ZERO, goal, 0.016, &mut 192),
        Some(corner)
    );
    assert!(
        world.calls.get() * 10 < first,
        "unchanged route repeated expensive traces"
    );
    world.calls.set(0);
    nav.steer(&mut graph, &world, agent, Vec3::ZERO, goal, 0.15, &mut 192);
    assert!(
        world.calls.get() >= first,
        "periodic shortcut retry was lost"
    );
    // Reaching a corner immediately consumes it even inside the retry window.
    assert_eq!(
        nav.steer(&mut graph, &world, agent, corner, goal, 0.016, &mut 192),
        Some(Vec3::new(2., 0., 1.))
    );
    world.barrier.set(false);
    graph.invalidate(Vec2::ZERO, Vec2::splat(3.));
    assert_eq!(
        nav.steer(&mut graph, &world, agent, Vec3::ZERO, goal, 0.016, &mut 192),
        Some(goal)
    );
}

#[test]
fn exhausted_frame_defers_all_planning_but_keeps_a_valid_route() {
    struct NoQueries;
    impl Geometry for NoQueries {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            panic!("floor query after deadline")
        }
        fn body_clear(&self, _: Vec3, _: Agent) -> bool {
            panic!("collision query after deadline")
        }
    }
    let agent = Agent::default();
    let mut graph = Graph::default();
    let mut navigator = Navigator {
        path: VecDeque::from([Vec3::X]),
        agent: Some(agent),
        ..default()
    };
    graph.deadline = Some(std::time::Instant::now());
    let goal = Vec3::X * 2.;
    assert_eq!(
        navigator.steer(
            &mut graph,
            &NoQueries,
            agent,
            Vec3::ZERO,
            goal,
            0.016,
            &mut 192
        ),
        Some(Vec3::X)
    );
    graph.invalidate(Vec2::ZERO, Vec2::ONE);
    assert_eq!(
        navigator.steer(
            &mut graph,
            &NoQueries,
            agent,
            Vec3::ZERO,
            goal,
            0.016,
            &mut 192
        ),
        None
    );
    graph.deadline = None;
    assert_eq!(
        navigator.steer(
            &mut graph,
            &Hill { slope: 0. },
            agent,
            Vec3::ZERO,
            goal,
            0.016,
            &mut 192
        ),
        Some(goal)
    );
}

#[test]
fn time_budget_yields_mid_node_and_resumes_the_same_search() {
    struct Slow(std::cell::Cell<bool>);
    impl Geometry for Slow {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0.]
        }
        fn body_clear(&self, _: Vec3, _: Agent) -> bool {
            if self.0.get() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            true
        }
    }
    let world = Slow(std::cell::Cell::new(false));
    let agent = Agent::default();
    let mut graph = Graph::default();
    let mut search = Search::new(&mut graph, &world, agent, Vec3::ZERO, Vec3::X * 3.);
    // Nine columns and their nine flat-floor attachments; expansion is still
    // untouched, so this test specifically exercises its mid-node cursor.
    search.advance(&mut graph, &world, agent, &mut 18);
    assert!(search.start.is_none());
    world.0.set(true);
    graph.begin_frame();
    assert_eq!(
        search.advance(&mut graph, &world, agent, &mut 192).0,
        Status::Searching
    );
    assert!(
        search.pending.is_some(),
        "must preserve unfinished neighbors"
    );
    let pending = search.pending.unwrap();
    assert_eq!(
        search.advance(&mut graph, &world, agent, &mut 192).0,
        Status::Searching
    );
    assert_eq!(search.pending.unwrap().0.node, pending.0.node);
    assert_eq!(search.pending.unwrap().1, pending.1);
    world.0.set(false);
    graph.deadline = None;
    for _ in 0..100 {
        let (status, path) = search.advance(&mut graph, &world, agent, &mut 192);
        if status == Status::Complete {
            assert!(!path.is_empty());
            return;
        }
        assert_eq!(status, Status::Searching);
    }
    panic!("budgeted search lost its frontier");
}

#[test]
fn steep_bank_escape_is_downhill_only_and_does_not_enable_climbing() {
    let agent = Agent::default();
    let hill = Hill { slope: 2. };
    for distance in [0.01, 0.08, 0.5] {
        let down = walk(&hill, agent, Vec3::ZERO, -Vec2::X * distance).unwrap();
        assert!(down.y < 0.);
        assert!(walk(&hill, agent, Vec3::ZERO, Vec2::X * distance).is_none());
        assert!(walk(&hill, agent, Vec3::ZERO, Vec2::Y * distance).is_none());
    }
}

#[test]
fn wall_recovery_stays_on_the_same_side_and_rejects_deep_embedding() {
    struct Wall;
    impl Geometry for Wall {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0., 3.]
        }
        fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
            p.x.abs() >= agent.radius + 0.01
        }
    }
    let agent = Agent::default();
    for sign in [-1., 1.] {
        let mut p = Vec3::X * sign * 0.25;
        assert!(settle(&Wall, agent, &mut p, &mut 0., 0.016));
        assert!(p.x * sign >= 0.29 && p.y == 0.);
        assert!(walk(&Wall, agent, p, p.xz() + Vec2::X * sign * 0.1).is_some());
    }
    let mut embedded = Vec3::ZERO;
    assert!(!settle(&Wall, agent, &mut embedded, &mut 0., 0.016));
    assert_eq!(embedded, Vec3::ZERO);
}

struct Passage {
    closed: bool,
}
impl Geometry for Passage {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        if p.x.abs() <= 4. && p.y.abs() <= 1. {
            vec![0.]
        } else {
            vec![]
        }
    }
    fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
        p.z.abs() + agent.radius <= 1. && !(self.closed && p.x.abs() < agent.radius + 0.1)
    }
}
#[test]
fn cached_paths_stop_at_new_obstacles_and_recover_when_reopened() {
    let agent = Agent::default();
    let mut graph = Graph::default();
    let mut nav = Navigator::default();
    let from = Vec3::new(-3., 0., 0.);
    let goal = Vec3::new(3., 0., 0.);
    assert!(
        nav.steer(
            &mut graph,
            &Passage { closed: false },
            agent,
            from,
            goal,
            0.016,
            &mut 192
        )
        .is_some()
    );
    graph.invalidate(Vec2::splat(-1.), Vec2::splat(1.));
    assert!(
        nav.steer(
            &mut graph,
            &Passage { closed: true },
            agent,
            from,
            goal,
            0.016,
            &mut 192
        )
        .is_none()
    );
    graph.invalidate(Vec2::splat(-1.), Vec2::splat(1.));
    assert!(
        nav.steer(
            &mut graph,
            &Passage { closed: false },
            agent,
            from,
            goal,
            0.016,
            &mut 192
        )
        .is_some()
    );
}
#[test]
fn cached_columns_respect_different_agent_clearances() {
    let mut graph = Graph::default();
    let small = Agent::default();
    assert!(
        !graph
            .nodes(&Passage { closed: false }, small, IVec2::new(0, 0))
            .is_empty()
    );
    assert!(
        graph
            .nodes(
                &Passage { closed: false },
                Agent {
                    radius: 1.1,
                    ..small
                },
                IVec2::new(0, 0)
            )
            .is_empty()
    );
}
#[test]
fn concave_obstacle_requires_route_away_from_goal() {
    struct U;
    impl Geometry for U {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0.]
        }
        fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
            let r = agent.radius;
            !(p.z > -2. - r && p.z < 3. + r && p.x.abs() > 1.6 - r && p.x.abs() < 2.5 + r
                || p.z > 2.2 - r && p.z < 3. + r && p.x.abs() < 2.5 + r)
        }
    }
    let mut graph = Graph::default();
    let agent = Agent::default();
    let from = Vec3::ZERO;
    let goal = Vec3::Z * 5.;
    let mut search = Search::new(&mut graph, &U, agent, from, goal);
    for _ in 0..1000 {
        let (status, path) = search.advance(&mut graph, &U, agent, &mut 12);
        if status != Status::Searching {
            assert_eq!(status, Status::Complete);
            assert!(path.iter().any(|p| p.z <= -2.));
            let mut previous = from;
            for p in path {
                assert!(clear(&U, agent, previous, p));
                previous = p;
            }
            return;
        }
    }
    panic!("did not escape U obstacle");
}
