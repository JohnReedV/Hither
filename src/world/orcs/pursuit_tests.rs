//! Continuous pursuit, with production planning and crowd locomotion. Each
//! named case asserts prompt motion, bounded stalls, support and arrival.
use super::*;
use crate::world::navigation::{self as nav, Agent, Geometry as _};

struct Landscape {
    shape: usize,
    origin: Vec2,
}
impl Landscape {
    fn height(&self, at: Vec2) -> f32 {
        let p = at - self.origin;
        match self.shape {
            0 => 0.,
            1 => 0.12 * (-((p.x - 2.) / 0.65).powi(2)).exp(),
            2 => 0.45 * (p.x * 0.25).sin() + 0.25 * (p.y * 0.35).sin(),
            _ => unreachable!("unknown regression fixture"),
        }
    }
    fn point(&self, at: Vec2) -> Vec3 {
        Vec3::new(at.x, self.height(at), at.y)
    }
}
impl nav::Geometry for Landscape {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        vec![self.height(p)]
    }
    fn body_clear(&self, p: Vec3, _: Agent) -> bool {
        p.y >= self.height(p.xz()) - 0.001
    }
}

fn continuous_pursuit(shape: usize, distance: f32, fps: f32, far: bool, moving: bool) {
    let world = Landscape {
        shape,
        origin: if far {
            Vec2::new(43189.13, -36791.7)
        } else {
            Vec2::ZERO
        },
    };
    let agent = Agent::default();
    let dt = 1. / fps;
    let mut graph = nav::Graph::default();
    let mut navigator = nav::Navigator::pursuit();
    let mut p = world.point(world.origin);
    let mut anchor = p;
    let mut stalled = 0.;
    let duration = distance / if moving { 0.85 } else { 1.35 } + 6.;
    for frame in 0..(duration * fps) as usize {
        let t = frame as f32 * dt;
        let goal = world.point(
            world.origin
                + Vec2::new(
                    distance + if moving { 0.45 * t } else { 0. },
                    distance * 0.17 + if moving { (t * 0.4).sin() } else { 0. },
                ),
        );
        graph.begin_slice(std::time::Duration::from_millis(1));
        if let Some(next) = navigator.steer(&mut graph, &world, agent, p, goal, dt, &mut 192) {
            p = navigation::step_on(&world, p, next, dt, &[], Entity::PLACEHOLDER);
        }
        assert!(world.body_clear(p, agent));
        assert!(
            (p.y - world.height(p.xz())).abs() < 0.002,
            "lost floor: {p:?}"
        );
        if p.xz().distance(goal.xz()) < 0.4 {
            return;
        }
        if p.distance(anchor) >= 0.04 {
            anchor = p;
            stalled = 0.;
        } else {
            stalled += dt;
        }
        assert!(
            stalled < 0.75,
            "stopped for {stalled:.2}s at {:?}, goal {:?}, status {:?}, frame {frame}",
            p - world.origin.extend(0.).xzy(),
            goal - world.origin.extend(0.).xzy(),
            navigator.status
        );
    }
    panic!(
        "pursuit never caught target: shape={shape} distance={distance} fps={fps} far={far} moving={moving}, at={p:?}"
    );
}

// Each fixture exercises a different regression; do not generate a product
// of frame rates, distances and origins merely to increase the case count.
#[test]
fn distant_moving_target_starts_without_waiting_for_a_complete_route() {
    continuous_pursuit(0, 80., 60., true, true);
}
#[test]
fn sub_step_height_crest_does_not_break_floor_following() {
    continuous_pursuit(1, 4., 60., false, false);
}
#[test]
fn moving_target_across_multiple_crests_does_not_starve_pursuit() {
    continuous_pursuit(2, 32., 60., false, true);
}

struct Obstacle {
    kind: usize,
}
impl nav::Geometry for Obstacle {
    fn floors(&self, at: Vec2, _: Agent) -> Vec<f32> {
        if !(-2. ..=40.).contains(&at.x) || at.y.abs() > 5. {
            return vec![];
        }
        if self.kind == 1 && (2. ..3.).contains(&at.x) {
            return vec![];
        }
        vec![if self.kind == 2 {
            ((at.x - 2.) * 1.7).clamp(0., 1.7)
        } else {
            0.
        }]
    }
    fn body_clear(&self, p: Vec3, agent: Agent) -> bool {
        !self.floors(p.xz(), agent).is_empty()
            && (self.kind != 0
                || p.x + agent.radius <= 5.
                || p.x - agent.radius >= 7.
                || p.z - agent.radius >= 2.)
    }
}
#[test]
fn blocked_forward_lookahead_does_not_make_local_steering_walk_backward() {
    let world = Obstacle { kind: 0 };
    let start = Vec3::new(4.3, 0., 0.);
    let end = navigation::step_on(
        &world,
        start,
        Vec3::X * 30.,
        1. / 30.,
        &[],
        Entity::PLACEHOLDER,
    );
    assert!(
        end.x >= start.x - 0.0001,
        "local steering reversed toward an already visited point: {end:?}"
    );
}

fn detour_pursuit(moving: bool, reverse: bool) {
    let world = Obstacle { kind: 0 };
    let agent = Agent::default();
    let mut navigator = nav::Navigator::pursuit();
    let mut graph = nav::Graph::default();
    let mut p = Vec3::ZERO;
    let mut crossed = false;
    for frame in 0..6000 {
        crossed |= p.x > 10.;
        let goal = if reverse && crossed {
            Vec3::ZERO
        } else {
            Vec3::new(
                30. + if moving {
                    (frame as f32 / 60. * 0.2).sin() * 3.
                } else {
                    0.
                },
                0.,
                0.,
            )
        };
        graph.begin_slice(std::time::Duration::from_millis(1));
        if let Some(next) = navigator.steer(&mut graph, &world, agent, p, goal, 1. / 60., &mut 192)
        {
            let before = p;
            p = navigation::step_on(&world, p, next, 1. / 60., &[], Entity::PLACEHOLDER);
            assert!(
                nav::clear(&world, agent, before, p),
                "unswept obstacle crossing"
            );
        }
        assert!(world.body_clear(p, agent));
        if crossed && p.distance(goal) < 0.4 {
            return;
        }
    }
    panic!(
        "detour stalled at {p:?}, moving={moving}, reverse={reverse}, {:?}",
        navigator.status
    );
}
#[test]
fn distant_pursuit_routes_around_wall_after_local_corridor_ends() {
    detour_pursuit(false, false);
}
#[test]
fn moving_target_does_not_pull_a_detour_back_into_wall() {
    detour_pursuit(true, false);
}
#[test]
fn reversing_target_rejoins_route_without_crossing_wall() {
    detour_pursuit(false, true);
}

fn rejects_unwalkable(kind: usize) {
    let world = Obstacle { kind };
    let mut navigator = nav::Navigator::pursuit();
    let mut graph = nav::Graph::default();
    let mut p = Vec3::ZERO;
    let goal = Vec3::new(30., if kind == 2 { 1.7 } else { 0. }, 0.);
    for _ in 0..120 {
        graph.begin_slice(std::time::Duration::from_millis(1));
        if let Some(next) = navigator.steer(
            &mut graph,
            &world,
            Agent::default(),
            p,
            goal,
            1. / 60.,
            &mut 192,
        ) {
            p = navigation::step_on(&world, p, next, 1. / 60., &[], Entity::PLACEHOLDER);
        }
        assert!(
            p.x < 2.01,
            "pursuit crossed an unwalkable boundary at {p:?}"
        );
        assert!(world.body_clear(p, Agent::default()));
    }
}
#[test]
fn distant_pursuit_cannot_walk_across_missing_floor() {
    rejects_unwalkable(1);
}
#[test]
fn distant_pursuit_cannot_climb_a_steep_face() {
    rejects_unwalkable(2);
}

#[test]
fn distant_lower_floor_uses_ramp_instead_of_following_overhead_ground() {
    struct Layers;
    impl nav::Geometry for Layers {
        fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
            if !(-8. ..=35.).contains(&p.x) || p.y.abs() > 3. {
                return vec![];
            }
            let mut floors = vec![-4.];
            if p.x >= 0. {
                floors.push(0.);
            } else if p.y >= 1. {
                floors.push(p.x * 0.5);
            }
            floors
        }
        fn body_clear(&self, p: Vec3, a: Agent) -> bool {
            !self
                .floors(p.xz(), a)
                .iter()
                .any(|y| *y > p.y + 0.02 && *y < p.y + a.height)
        }
    }
    let mut navigator = nav::Navigator::pursuit();
    let mut graph = nav::Graph::default();
    let mut p = Vec3::ZERO;
    let goal = Vec3::new(30., -4., 0.);
    let mut used_ramp = false;
    for _ in 0..6000 {
        graph.begin_slice(std::time::Duration::from_millis(1));
        if let Some(next) = navigator.steer(
            &mut graph,
            &Layers,
            Agent::default(),
            p,
            goal,
            1. / 60.,
            &mut 192,
        ) {
            p = navigation::step_on(&Layers, p, next, 1. / 60., &[], Entity::PLACEHOLDER);
        }
        used_ramp |= p.x < -7.;
        if !used_ramp {
            assert!(
                p.x < 0.5,
                "walked toward unreachable overhead target: {p:?}"
            );
        }
        if p.distance(goal) < 0.4 {
            assert!(used_ramp);
            return;
        }
    }
    panic!("failed to reach lower floor: {p:?}");
}
