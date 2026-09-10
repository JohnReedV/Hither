//! Compact evergreen citrus crown, separate from the open apple scaffolds.
//! Shape/leaf reference: NC State Extension's Citrus x sinensis photographs.
use super::*;

pub(super) const LEAF_VERTICES: usize = 15;

pub(super) fn generate(seed: u64) -> Tree {
    let mut rng = Rng(seed);
    let mut wood = Geometry::default();
    let mut leaves = Geometry::default();
    let mut camera_bounds = Vec::new();
    let mut fruit_spurs = Vec::new();
    let rotation = rng.range(0.0, TAU);
    let fork_height = rng.range(0.64, 1.05);
    let bend = Vec3::new(rng.range(-0.08, 0.08), 0.0, rng.range(-0.08, 0.08));
    let crown_width = rng.range(1.00, 1.34);
    let crown_height = rng.range(1.02, 1.38);
    let crown_center = rng.range(1.94, 2.20);
    let eccentricity = rng.range(0.82, 1.12);
    let lobe_phase = rng.range(0.0, TAU);
    let trunk = [
        (Vec3::ZERO, 0.19),
        (bend * 0.4 + Vec3::Y * fork_height * 0.45, 0.15),
        (bend + Vec3::Y * fork_height, 0.11),
    ];
    branch(&mut wood, &trunk, 20, rotation);
    apple_tree::wood_bounds(&curved_path(&trunk), &mut camera_bounds);
    for i in 0..5 {
        let a = rotation + i as f32 * TAU / 5.0;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        branch(
            &mut wood,
            &[
                (d * 0.07 + Vec3::Y * 0.12, 0.055),
                (d * 0.25 + Vec3::Y * 0.02, 0.025),
                (d * 0.42 - Vec3::Y * 0.01, 0.002),
            ],
            8,
            a,
        );
    }
    let mut scaffolds = Vec::new();
    let scaffold_count = 5 + (rng.unit() * 4.0) as usize;
    for i in 0..scaffold_count {
        let a = rotation + i as f32 * TAU / scaffold_count as f32 + rng.range(-0.20, 0.20);
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let path = [
            (trunk[2].0, 0.07),
            (
                d * rng.range(0.28, 0.50) + Vec3::Y * rng.range(1.25, 1.60),
                0.042,
            ),
            (d * rng.range(0.48, 0.75) + Vec3::Y * crown_center, 0.020),
            (
                d * rng.range(0.35, 0.60) + Vec3::Y * (crown_center + crown_height * 0.62),
                0.003,
            ),
        ];
        branch(&mut wood, &path, 12, a);
        apple_tree::wood_bounds(&curved_path(&path), &mut camera_bounds);
        scaffolds.push(curved_path(&path));
    }
    // Layered, irregular ellipsoid: not an empty fork and not a solid canopy
    // blob. Every leaf is connected through a twig to real scaffold wood.
    let mut distant_wood = wood.clone();
    let branch_count = 135 + (rng.unit() * 55.0) as usize;
    for i in 0..branch_count {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / branch_count as f32;
        let a = rotation + i as f32 * 2.399963 + rng.range(-0.15, 0.15);
        let radial = (1.0 - y * y).sqrt();
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let lobe = 1.0 + (a * 3.0 + lobe_phase).sin() * 0.14 + (a * 2.0 - lobe_phase).cos() * 0.07;
        let tip = d
            * Vec3::new(eccentricity, 1.0, 1.0 / eccentricity)
            * radial
            * crown_width
            * lobe
            * rng.range(0.93, 1.07)
            + Vec3::Y * (crown_center + y * crown_height)
            + bend;
        let scaffold = scaffolds
            .iter()
            .min_by(|p, q| {
                path_point(p, 0.65)
                    .0
                    .distance_squared(tip)
                    .total_cmp(&path_point(q, 0.65).0.distance_squared(tip))
            })
            .unwrap();
        let (root, radius) = path_point(scaffold, ((tip.y - fork_height) / 2.1).clamp(0.15, 0.95));
        let path = [
            (root, (radius * 0.45).min(0.024)),
            (root.lerp(tip, 0.50) + Vec3::Y * 0.06, 0.010),
            (tip, 0.0015),
        ];
        branch(&mut wood, &path, 6, a);
        // Sub-centimeter wood sits inside the dense leaf canopy. Only the
        // trunk, buttresses and primary scaffolds need distant geometry.
        let curve = curved_path(&path);
        for shoot in 0..5 {
            let root = path_point(&curve, 0.52 + shoot as f32 * 0.12).0;
            let fan = a + if shoot % 2 == 0 { -1.0 } else { 1.0 } * rng.range(0.55, 1.25);
            let end = root
                + Vec3::new(fan.cos(), rng.range(-0.55, 0.4), fan.sin()) * rng.range(0.20, 0.34);
            branch(&mut wood, &[(root, 0.003), (end, 0.0005)], 4, fan);
            fruit_spurs.push([root.lerp(end, 0.65), end]);
            for blade in 0..8 {
                let base = root.lerp(end, blade as f32 / 7.0);
                let la = fan + if blade % 2 == 0 { -1.0 } else { 1.0 } * rng.range(0.65, 1.15);
                let axis = Vec3::new(la.cos(), rng.range(-0.75, 0.6), la.sin()).normalize();
                // Five broader, alternating blades per shoot replace eight
                // heavily overlapping blades. The whole tree uses this shape,
                // not a distance-dependent density/material trick. Consume the
                // same random samples so branch and fruit-anchor layout stays stable.
                let length = rng.range(0.13, 0.20);
                let roll = rng.range(-0.75, 0.75);
                let color = Vec3::splat(rng.range(0.78, 1.18));
                if matches!(blade, 1 | 4 | 6) {
                    continue;
                }
                citrus_leaf(&mut leaves, base, axis, length * 1.25, roll, color);
            }
        }
    }
    wood.finish_normals();
    distant_wood.finish_normals();
    leaves.finish_normals();
    let rays = RayMesh::new(wood.triangles().chain(leaves.triangles()));
    Tree {
        distant_wood: Some(distant_wood),
        wood,
        leaves,
        rays,
        fruit_spurs,
        camera_bounds,
    }
}

fn citrus_leaf(mesh: &mut Geometry, base: Vec3, axis: Vec3, length: f32, roll: f32, color: Vec3) {
    let side = Quat::from_axis_angle(axis, roll) * axis.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = side.cross(axis).normalize();
    let first = mesh.positions.len() as u32;
    // Broad elliptical leathery blades, smooth edges and a folded midrib.
    // Four rows are enough for this small leaf; no serrated apple silhouette.
    for row in 0..=4 {
        let t = row as f32 / 4.0;
        let width = (PI * t).sin().max(0.0).powf(0.72) * length * 0.32;
        for col in 0..3 {
            let across = col as f32 - 1.0;
            let p = base
                + axis * length * t
                + side * width * across
                + up * length * ((PI * t).sin() * 0.07 * (1.0 - across.abs()) - t * t * 0.10);
            mesh.vertex(p, Vec2::new(col as f32 * 0.5, t), color);
        }
        if row > 0 {
            for col in 0..2 {
                let a = first + (row - 1) * 3 + col;
                mesh.triangle(a, a + 3, a + 1);
                mesh.triangle(a + 1, a + 3, a + 4);
            }
        }
    }
}
