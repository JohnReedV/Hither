//! Irregular excavation through the mountain: uneven bends and changing grades.
use super::*;
fn cubic(a: Vec2, b: Vec2, c: Vec2, d: Vec2, t: f32) -> Vec2 {
    0.5 * ((2. * b)
        + (-a + c) * t
        + (2. * a - 5. * b + 4. * c - d) * t * t
        + (-a + 3. * b - 3. * c + d) * t * t * t)
}
pub(super) fn excavate(
    start: Vec3,
    angle: f32,
    twist: f32,
    end_y: f32,
    rng: &mut Rng,
) -> Vec<Vec3> {
    let bends = 12 + (rng.unit() * 5.).floor() as usize;
    let advances: Vec<_> = (0..bends).map(|_| rng.range(0.35, 1.75)).collect();
    let sum: f32 = advances.iter().sum();
    let mut turn = 0.;
    let mut knots = Vec::new();
    let mut grades = Vec::new();
    for (i, advance) in advances
        .iter()
        .copied()
        .chain(std::iter::once(0.0))
        .enumerate()
    {
        let t = i as f32 / bends as f32;
        let radius = 28.
            + 28. * t
            + if i == 0 || i == bends {
                0.
            } else {
                rng.range(-3.8, 3.8)
            };
        let bearing = angle + turn / sum * twist;
        knots.push(Vec2::new(bearing.cos(), bearing.sin()) * radius);
        grades.push(rng.range(0.62, 1.35));
        turn += advance;
    }
    let mut path = vec![start];
    let mut rise = vec![0.];
    for j in 0..=100 {
        let t = j as f32 / 100. * bends as f32;
        let i = (t.floor() as usize).min(bends - 1);
        let f = (t - i as f32).min(1.);
        let p = cubic(
            knots[i.saturating_sub(1)],
            knots[i],
            knots[i + 1],
            knots[(i + 2).min(bends)],
            f,
        );
        let grade = grades[i].lerp(grades[i + 1], f);
        let v = Vec3::new(p.x, start.y, p.y);
        let accumulated = if j == 0 {
            0.
        } else {
            rise.last().unwrap() + path.last().unwrap().xz().distance(p) * grade
        };
        path.push(v);
        rise.push(accumulated);
    }
    let total = *rise.last().unwrap();
    for (p, distance) in path.iter_mut().zip(rise) {
        p.y = start.y.lerp(end_y, distance / total);
    }
    path
}
// Two equally likely outcomes out of four produce one route; one each produce
// two and three. Draw only after validating an equal three-route candidate pool.
pub(super) fn count(seed: u64) -> usize {
    match seed & 3 {
        0 | 1 => 1,
        2 => 2,
        _ => 3,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn goblin_tunnel_weights_and_excavation_are_repeatable_and_irregular() {
        let mut bins = [0_i32; 3];
        for seed in 0..8192 {
            bins[count(hash(IVec2::new(seed, -seed), 934721)) - 1] += 1;
        }
        for (found, expected) in bins.into_iter().zip([4096, 2048, 2048]) {
            assert!((found - expected).abs() < 180);
        }
        let build = || excavate(Vec3::new(5., -5., 7.), 0.3, 4.2, 45., &mut Rng(73));
        let a = build();
        assert_eq!(a, build());
        let grades: Vec<_> = a[1..]
            .windows(2)
            .map(|w| (w[1].y - w[0].y) / w[1].xz().distance(w[0].xz()))
            .collect();
        assert!(
            grades.iter().copied().fold(f32::MIN, f32::max)
                - grades.iter().copied().fold(f32::MAX, f32::min)
                > 0.08
        );
        let turns: Vec<_> = a[1..]
            .windows(3)
            .map(|w| {
                (w[1] - w[0])
                    .xz()
                    .normalize()
                    .perp_dot((w[2] - w[1]).xz().normalize())
            })
            .collect();
        assert!(turns.iter().any(|&v| v > 0.025) && turns.iter().any(|&v| v < -0.025));
    }
}
