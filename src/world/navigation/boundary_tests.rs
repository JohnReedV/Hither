use super::*;
struct RampStep;
impl Geometry for RampStep {
    fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
        vec![0.5 * p.x + if p.x >= 0. { 0.14 } else { 0. }]
    }
    fn body_clear(&self, _: Vec3, _: Agent) -> bool {
        true
    }
    fn support_boundaries(&self, a: Vec2, b: Vec2, _: Agent) -> Vec<f32> {
        vec![-a.x / (b.x - a.x)]
    }
}
#[test]
fn slope_plus_step_has_the_same_result_for_whole_and_partial_segments() {
    let agent = Agent::default();
    let from = Vec3::new(-0.08, -0.04, 0.);
    let to = Vec3::new(0.08, 0.18, 0.);
    assert!(clear(&RampStep, agent, from, to));
    let mut p = from;
    for i in 1..=32 {
        let xz = from.xz().lerp(to.xz(), i as f32 / 32.);
        p = walk(&RampStep, agent, p, xz).unwrap();
    }
    assert!(p.distance(to) < 0.001);
}
#[test]
fn graph_connection_must_reach_its_actual_floor() {
    struct Floors;
    impl Geometry for Floors {
        fn floors(&self, _: Vec2, _: Agent) -> Vec<f32> {
            vec![0., 0.18]
        }
        fn body_clear(&self, _: Vec3, _: Agent) -> bool {
            true
        }
    }
    assert!(!clear(
        &Floors,
        Agent::default(),
        Vec3::ZERO,
        Vec3::new(0.5, 0.18, 0.)
    ));
    assert_eq!(
        grounded_goal(&Floors, Agent::default(), Vec3::new(0., 0.03, 0.)),
        Vec3::ZERO
    );
}
#[test]
fn exact_support_boundaries_prevent_skipping_thin_gaps() {
    struct Gap;
    impl Geometry for Gap {
        fn floors(&self, p: Vec2, _: Agent) -> Vec<f32> {
            if (0.1001..0.1002).contains(&p.x) {
                vec![]
            } else {
                vec![0.]
            }
        }
        fn body_clear(&self, _: Vec3, _: Agent) -> bool {
            true
        }
        fn support_boundaries(&self, a: Vec2, b: Vec2, _: Agent) -> Vec<f32> {
            vec![(0.1001 - a.x) / (b.x - a.x), (0.1002 - a.x) / (b.x - a.x)]
        }
    }
    assert!(!clear(&Gap, Agent::default(), Vec3::ZERO, Vec3::X));
}
