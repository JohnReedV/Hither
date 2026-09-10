//! Broad, low fruiting crown based on full-tree orchard references.
//! See docs/ORCHARD.md for reference photographs and proportions.
use super::*;

pub(super) fn generate(seed: u64) -> Tree {
    let mut rng = Rng(seed);
    let mut wood = Geometry::default();
    let mut leaves = Geometry::default();
    let mut fruit_spurs = Vec::new();
    let mut camera_bounds = Vec::new();
    let rotation = rng.range(0.0, TAU);
    let lean = Vec3::new(rotation.cos(), 0.0, rotation.sin());
    // A short, subtly crooked trunk, mostly hidden by low fruiting growth.
    let trunk = [
        (Vec3::ZERO, 0.25),
        (lean * 0.035 + Vec3::Y * 0.35, 0.20),
        (-lean * 0.025 + Vec3::Y * 0.78, 0.16),
        (lean * 0.055 + Vec3::Y * 1.22, 0.105),
    ];
    branch(&mut wood, &trunk, 32, rotation);
    let trunk_curve = curved_path(&trunk);
    wood_bounds(&trunk_curve, &mut camera_bounds);
    for i in 0..6 {
        let a = rotation + i as f32 * TAU / 6.0 + rng.range(-0.2, 0.2);
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        branch(
            &mut wood,
            &[
                (d * 0.10 + Vec3::Y * 0.15, 0.075),
                (d * 0.29 + Vec3::Y * 0.035, 0.042),
                (d * rng.range(0.43, 0.61) - Vec3::Y * 0.015, 0.003),
            ],
            12,
            a,
        );
    }
    let count = 7;
    let mut scaffolds = Vec::new();
    for i in 0..count {
        let angle = rotation + i as f32 * TAU / count as f32 + rng.range(-0.22, 0.22);
        let d = Vec3::new(angle.cos(), 0.0, angle.sin());
        let side = Vec3::new(-d.z, 0.0, d.x);
        let root = path_point(&trunk_curve, rng.range(0.70, 1.0)).0;
        let reach = rng.range(1.28, 1.70);
        let y = rng.range(1.72, 2.08);
        // Broad scaffold: rises from the fork then flattens and bows outward.
        let path = vec![
            (root, rng.range(0.075, 0.105)),
            (
                d * reach * 0.27 + Vec3::Y * 1.65 + side * rng.range(-0.08, 0.08),
                0.065,
            ),
            (
                d * reach * 0.64 + Vec3::Y * (y + 0.18) + side * rng.range(-0.14, 0.14),
                0.033,
            ),
            (d * reach + Vec3::Y * y, 0.006),
        ];
        scaffolds.push((path, angle));
    }
    // Smaller upper limbs fill the crown's center, rather than two tall fans
    // separated by a hole. These emerge from existing scaffold wood.
    for i in 0..4 {
        let curve = curved_path(&scaffolds[i].0);
        let (root, radius) = path_point(&curve, 0.35);
        let angle = rotation + i as f32 * 2.39996 + 0.6;
        let d = Vec3::new(angle.cos(), 0.0, angle.sin());
        let end = d * rng.range(0.45, 0.85) + Vec3::Y * rng.range(2.50, 2.75);
        scaffolds.push((
            vec![
                (root, radius * 0.68),
                (root.lerp(end, 0.55) + d * 0.09, radius * 0.32),
                (end, 0.003),
            ],
            angle,
        ));
    }
    for (scaffold, angle) in scaffolds {
        branch(&mut wood, &scaffold, 16, angle);
        let curve = curved_path(&scaffold);
        wood_bounds(&curve, &mut camera_bounds);
        for fork in 0..5 {
            let t = 0.30 + fork as f32 * 0.175;
            let (root, radius) = path_point(&curve, t);
            let fan = angle + if fork % 2 == 0 { -1.0 } else { 1.0 } * rng.range(0.55, 1.25);
            let d = Vec3::new(fan.cos(), 0.0, fan.sin());
            let reach = rng.range(0.30, 0.55);
            let end = root + d * reach + Vec3::Y * rng.range(-0.25, 0.22);
            let path = [
                (root, radius * 0.62),
                (root.lerp(end, 0.5) + Vec3::Y * 0.10, radius * 0.30),
                (end, 0.002),
            ];
            branch(&mut wood, &path, 8, fan);
            let secondary = curved_path(&path);
            wood_bounds(&secondary, &mut camera_bounds);
            for twig in 0..4 {
                let (root, radius) = path_point(&secondary, 0.35 + twig as f32 * 0.21);
                let a = fan + rng.range(-1.5, 1.5);
                let reach = rng.range(0.22, 0.40);
                let end =
                    root + Vec3::new(a.cos() * reach, rng.range(-0.30, 0.24), a.sin() * reach);
                let twig_path = [
                    (root, (radius * 0.6).min(0.009)),
                    (
                        root.lerp(end, 0.55) + Vec3::Y * 0.025,
                        (radius * 0.3).min(0.004),
                    ),
                    (end, 0.001),
                ];
                branch(&mut wood, &twig_path, 6, a);
                let twig_curve = curved_path(&twig_path);
                for pair in twig_curve.windows(2).skip(2) {
                    fruit_spurs.push([pair[0].0, pair[1].0]);
                }
                for spray in 0..5 {
                    let base = path_point(&twig_curve, 0.18 + spray as f32 * 0.20).0;
                    let sa = a + if spray % 2 == 0 { -1.0 } else { 1.0 } * rng.range(0.5, 1.3);
                    let tip =
                        base + Vec3::new(sa.cos() * 0.11, rng.range(-0.07, 0.08), sa.sin() * 0.11);
                    let spray_path = [(base, 0.002), (tip, 0.0005)];
                    branch(&mut wood, &spray_path, 4, sa);
                    let spray_curve = curved_path(&spray_path);
                    for blade in 0..6 {
                        let foot = path_point(&spray_curve, blade as f32 / 5.0).0;
                        let la = sa + if blade % 2 == 0 { -1.0 } else { 1.0 } * rng.range(0.6, 1.5);
                        let axis = Vec3::new(la.cos(), rng.range(-0.7, 0.6), la.sin()).normalize();
                        let initial = axis.any_orthonormal_vector();
                        let side = axis.cross(Vec3::Y).normalize();
                        let roll = axis.dot(initial.cross(side)).atan2(initial.dot(side))
                            + rng.range(-0.65, 0.65);
                        let green = rng.range(0.88, 1.18);
                        leaf(
                            &mut leaves,
                            foot,
                            axis,
                            rng.range(0.095, 0.15),
                            roll,
                            Vec3::new(green * 0.93, green, green * 0.72),
                        );
                    }
                }
            }
        }
    }
    wood.finish_normals();
    leaves.finish_normals();
    let rays = RayMesh::new(wood.triangles().chain(leaves.triangles()));
    Tree {
        distant_wood: None,
        wood,
        leaves,
        rays,
        fruit_spurs,
        camera_bounds,
    }
}

pub(super) fn wood_bounds(path: &[(Vec3, f32)], bounds: &mut Vec<(Vec3, f32)>) {
    for pair in path.windows(2) {
        let steps = (pair[0].0.distance(pair[1].0) / 0.05).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            bounds.push((
                pair[0].0.lerp(pair[1].0, t),
                pair[0].1.lerp(pair[1].1, t) * 1.1 + 0.025,
            ));
        }
    }
}
