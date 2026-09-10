use super::*;
struct Stacked {
    offset: f32,
    closed: bool,
}
impl Geometry for Stacked {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        let mut result = Vec::new();
        if (0. ..=12.).contains(&p.x) && (0. ..=4.).contains(&p.y) {
            result.extend([self.offset, self.offset - 4., self.offset - 8.]);
        }
        // Two ramps on opposite ends, each with a landing and a return
        // lane. Going straight down at the goal can never reach them.
        if (12. ..=18.).contains(&p.x) && (0. ..=4.).contains(&p.y) {
            result.push(self.offset - 4.);
            if p.y <= 1. {
                result.push(self.offset - (p.x - 12.) * 2. / 3.);
            }
        }
        if (-6. ..=0.).contains(&p.x) && (0. ..=4.).contains(&p.y) {
            result.push(self.offset - 8.);
            if p.y >= 3. {
                result.push(self.offset - 4. + p.x * 2. / 3.);
            }
        }
        result
    }
    fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
        if self.closed && p.x > 14. && p.x < 15. && p.y > self.offset - 3. && p.z < 1.5 {
            return false;
        }
        !self
            .floors(p.xz(), agent)
            .iter()
            .any(|y| *y > p.y + 0.02 && *y < p.y + agent.height)
    }
}
fn route(world: &impl Geometry, from: Vec3, to: Vec3) -> (Status, VecDeque<Vec3>) {
    let mut graph = Graph::default();
    let agent = Agent::default();
    let mut search = Search::new(&mut graph, world, agent, from, to);
    for _ in 0..10_000 {
        let mut budget = 13;
        let result = search.advance(&mut graph, world, agent, &mut budget);
        assert!(budget <= 13);
        if result.0 != Status::Searching {
            return result;
        }
    }
    panic!("search did not finish");
}
#[test]
fn three_stacked_floors_connect_only_via_opposite_ramps_at_any_altitude() {
    for offset in [-10_000., 0., 10_000.] {
        let world = Stacked {
            offset,
            closed: false,
        };
        let start = Vec3::new(6., offset, 2.);
        let goal = start - Vec3::Y * 8.;
        assert!(!clear(&world, Agent::default(), start, goal));
        let (status, path) = route(&world, start, goal);
        assert_eq!(status, Status::Complete);
        assert!(path.iter().any(|p| p.x >= 17.5));
        assert!(path.iter().any(|p| p.x <= -5.5));
        let mut previous = start;
        for p in path {
            assert!(clear(&world, Agent::default(), previous, p));
            previous = p;
        }
        assert_eq!(previous, goal);
    }
}
#[test]
fn closing_only_connection_makes_lower_floors_unreachable() {
    let world = Stacked {
        offset: 0.,
        closed: true,
    };
    assert_eq!(
        route(&world, Vec3::new(6., 0., 2.), Vec3::new(6., -8., 2.)).0,
        Status::Unreachable
    );
}
struct Flat;
impl Geometry for Flat {
    fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
        vec![0.]
    }
    fn body_clear(&self, _: Vec3, _: Agent) -> bool {
        true
    }
}
#[test]
fn routes_beyond_old_eighty_meter_limit_and_across_negative_tiles() {
    let (status, path) = route(&Flat, Vec3::new(-100., 0., 0.), Vec3::new(100., 0., 0.));
    assert_eq!(status, Status::Complete);
    assert_eq!(path.back(), Some(&Vec3::new(100., 0., 0.)));
}
#[test]
fn tile_invalidation_is_local_and_searches_do_not_use_old_geometry() {
    let mut graph = Graph::default();
    let agent = Agent::default();
    graph.nodes(&Flat, agent, IVec2::new(-1, 0));
    graph.nodes(&Flat, agent, IVec2::new(200, 0));
    let mut search = Search::new(&mut graph, &Flat, agent, Vec3::ZERO, Vec3::X * 100.);
    let far = graph.tiles[&IVec2::new(3, 0)].columns.len();
    graph.invalidate(Vec2::splat(-1.), Vec2::splat(1.));
    assert!(!graph.tiles.contains_key(&IVec2::new(-1, 0)));
    assert_eq!(graph.tiles[&IVec2::new(3, 0)].columns.len(), far);
    assert_ne!(
        search.advance(&mut graph, &Flat, agent, &mut 100).0,
        Status::Complete
    );
}
#[test]
fn zero_budget_does_not_expand_search() {
    let mut graph = Graph::default();
    let agent = Agent::default();
    let mut search = Search::new(&mut graph, &Flat, agent, Vec3::ZERO, Vec3::X * 10.);
    let count = search.costs.len();
    assert_eq!(
        search.advance(&mut graph, &Flat, agent, &mut 0).0,
        Status::Searching
    );
    assert_eq!(search.costs.len(), count);
}

#[test]
fn narrow_passage_between_old_grid_columns_has_connected_nodes() {
    struct Narrow;
    impl Geometry for Narrow {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0.]
        }
        fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
            (p.x - 0.24).abs() + agent.radius < 0.48
        }
    }
    assert_eq!(
        route(&Narrow, Vec3::new(0.24, 0., -2.), Vec3::new(0.24, 0., 2.)).0,
        Status::Complete
    );
}

#[test]
fn falling_lands_on_first_floor_without_crossing_obstructions() {
    struct Floors {
        obstacle: bool,
    }
    impl Geometry for Floors {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![-2., -6.]
        }
        fn body_clear(&self, p: Vec3, _: Agent) -> bool {
            !self.obstacle || p.y > -1.
        }
    }
    for obstacle in [false, true] {
        let mut feet = Vec3::ZERO;
        let mut velocity = -100.;
        let landed = settle(
            &Floors { obstacle },
            Agent::default(),
            &mut feet,
            &mut velocity,
            0.05,
        );
        if obstacle {
            assert!(!landed);
            assert!(feet.y > -1.);
        } else {
            assert!(landed);
            assert_eq!(feet.y, -2.);
        }
        assert_eq!(velocity, 0.);
    }
}

#[test]
fn slightly_embedded_feet_recover_only_to_local_clear_support() {
    struct Floors {
        blocked: bool,
    }
    impl Geometry for Floors {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0., -6.]
        }
        fn body_clear(&self, p: Vec3, _: Agent) -> bool {
            !self.blocked || p.y < -6.02
        }
    }
    let start = Vec3::new(0., -6.04, 0.);
    for blocked in [false, true] {
        let mut feet = start;
        let mut velocity = -1.;
        assert_eq!(
            settle(
                &Floors { blocked },
                Agent::default(),
                &mut feet,
                &mut velocity,
                1. / 60.
            ),
            !blocked
        );
        assert_eq!(feet, if blocked { start } else { start.with_y(-6.) });
        assert_eq!(velocity, 0.);
    }
}

#[test]
fn reachable_target_replaces_unfinished_unreachable_search() {
    let mut graph = Graph::default();
    let mut navigator = Navigator::default();
    let agent = Agent::default();
    // No floor exists at this altitude, so the query remains unfinished.
    assert!(
        navigator
            .steer(
                &mut graph,
                &Flat,
                agent,
                Vec3::ZERO,
                Vec3::new(4., -6., 0.),
                0.016,
                &mut 1
            )
            .is_none()
    );
    assert!(navigator.search.is_some());
    let outside = Vec3::X * 4.;
    assert_eq!(
        navigator.steer(&mut graph, &Flat, agent, Vec3::ZERO, outside, 0.016, &mut 1),
        Some(outside)
    );
    assert!(navigator.search.is_none());
}

#[test]
fn oscillation_replans_instead_of_counting_each_step_as_progress() {
    let mut graph = Graph::default();
    let agent = Agent::default();
    let goal = Vec3::X * 5.;
    let mut navigator = Navigator {
        path: VecDeque::from([-Vec3::X]),
        goal: Some(goal),
        agent: Some(agent),
        retry: 0.,
        ..Default::default()
    };
    for frame in 0..120 {
        let p = Vec3::X * if frame % 2 == 0 { 0.025 } else { -0.025 };
        if navigator.steer(&mut graph, &Flat, agent, p, goal, 1. / 60., &mut 192) == Some(goal) {
            return;
        }
    }
    panic!("oscillating body never replaced its stalled route");
}
