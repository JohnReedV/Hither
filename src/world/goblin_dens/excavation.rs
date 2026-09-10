//! Sparse marching tetrahedra produces a watertight union of the chamber and
//! every passage. The same triangles are used by the traversal BVHs.
use super::*;
use std::collections::{BTreeSet, HashMap};
const STEP: f32 = 0.7;
fn add_cells(cells: &mut BTreeSet<(i32, i32, i32)>, lo: Vec3, hi: Vec3) {
    let a = (lo / STEP).floor().as_ivec3();
    let b = (hi / STEP).ceil().as_ivec3();
    for x in a.x..=b.x {
        for y in a.y..=b.y {
            for z in a.z..=b.z {
                cells.insert((x, y, z));
            }
        }
    }
}
pub(super) fn shell(den: &Den) -> Geometry {
    let mut cells = BTreeSet::new();
    // Visit only the chamber's surface band, not its empty interior.
    let a = ((-den.radii - Vec3::splat(4.)) / STEP).floor().as_ivec3();
    let b = ((den.radii + Vec3::splat(4.)) / STEP).ceil().as_ivec3();
    for x in a.x..=b.x {
        for y in a.y..=b.y {
            for z in a.z..=b.z {
                let p = Vec3::new(x as f32, y as f32, z as f32) * STEP;
                let d = (((p / den.radii).length() - 1.) * den.radii.min_element()).max(-17. - p.y);
                if d.abs() < 4.0 {
                    cells.insert((x, y, z));
                }
            }
        }
    }
    for path in &den.tunnels {
        for w in path.windows(2) {
            let count = (w[0].distance(w[1]) / 2.).ceil() as usize;
            for i in 0..=count {
                let p = w[0].lerp(w[1], i as f32 / count as f32);
                add_cells(&mut cells, p - Vec3::splat(3.5), p + Vec3::splat(3.5));
            }
        }
    }
    let mut samples = HashMap::new();
    let mut normals = HashMap::new();
    let mut out = Geometry::default();
    let offsets = [
        IVec3::ZERO,
        IVec3::X,
        IVec3::new(1, 1, 0),
        IVec3::Y,
        IVec3::Z,
        IVec3::new(1, 0, 1),
        IVec3::ONE,
        IVec3::new(0, 1, 1),
    ];
    let tetra = [
        [0, 1, 2, 6],
        [0, 2, 3, 6],
        [0, 3, 7, 6],
        [0, 7, 4, 6],
        [0, 4, 5, 6],
        [0, 5, 1, 6],
    ];
    for (x, y, z) in cells {
        let keys = offsets.map(|v| v + IVec3::new(x, y, z));
        let points = keys.map(|v| v.as_vec3() * STEP);
        let values = keys.map(|k| {
            *samples
                .entry(k)
                .or_insert_with(|| den.air(k.as_vec3() * STEP))
        });
        if values.iter().all(|d| *d >= 0.) || values.iter().all(|d| *d < 0.) {
            continue;
        }
        for t in tetra {
            let mut polygon = Vec::new();
            let mut inward = Vec3::ZERO;
            for i in 0..4 {
                for j in i + 1..4 {
                    let (a, b) = (t[i], t[j]);
                    if (values[a] < 0.) != (values[b] < 0.) {
                        polygon
                            .push(points[a].lerp(points[b], values[a] / (values[a] - values[b])));
                        inward += (points[b] - points[a]) * if values[b] < 0. { 1. } else { -1. };
                    }
                }
            }
            if polygon.len() < 3 {
                continue;
            }
            let normal = inward.normalize();
            let u = normal.any_orthonormal_vector();
            let v = normal.cross(u);
            let center = polygon.iter().copied().sum::<Vec3>() / polygon.len() as f32;
            polygon.sort_by(|a, b| {
                let a = *a - center;
                let b = *b - center;
                a.dot(v)
                    .atan2(a.dot(u))
                    .total_cmp(&b.dot(v).atan2(b.dot(u)))
            });
            // Clip the shell exactly where it meets the existing terrain grid.
            // This leaves open mouths, rather than a capped tube or floating lip.
            let mut clipped = Vec::new();
            for i in 0..polygon.len() {
                let a = polygon[i];
                let b = polygon[(i + 1) % polygon.len()];
                let da = den.center.y + a.y - terrain::height(den.center.xz() + a.xz());
                let db = den.center.y + b.y - terrain::height(den.center.xz() + b.xz());
                if da <= 0. {
                    clipped.push(a);
                }
                if (da <= 0.) != (db <= 0.) {
                    clipped.push(a.lerp(b, da / (da - db)));
                }
            }
            if clipped.len() < 3 {
                continue;
            }
            let base = out.positions.len() as u32;
            for p in clipped.iter().copied() {
                let uv = if normal.y.abs() > 0.65 {
                    p.xz()
                } else if normal.x.abs() > normal.z.abs() {
                    Vec2::new(p.z, p.y)
                } else {
                    Vec2::new(p.x, p.y)
                } * 0.22;
                let soil = (((p / den.radii).length() - 1.0) * 2.).clamp(0., 1.);
                let i = out.vertex(
                    p,
                    uv,
                    geology::tint(p) * Vec3::ONE.lerp(Vec3::new(1.0, 0.84, 0.64), soil),
                );
                let key = (p * 10000.).round().as_ivec3();
                let smooth = *normals.entry(key).or_insert_with(|| {
                    let e = 0.025;
                    -Vec3::new(
                        den.air(p + Vec3::X * e) - den.air(p - Vec3::X * e),
                        den.air(p + Vec3::Y * e) - den.air(p - Vec3::Y * e),
                        den.air(p + Vec3::Z * e) - den.air(p - Vec3::Z * e),
                    )
                    .normalize_or(normal)
                });
                out.normals[i as usize] = smooth.to_array();
            }
            for i in 1..clipped.len() - 1 {
                out.triangle(base, base + i as u32, base + i as u32 + 1);
            }
        }
    }
    out
}
