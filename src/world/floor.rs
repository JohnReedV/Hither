//! Spatially indexed, layered support built from final rendered triangles.
//! Coordinates are feet coordinates. A missing hit is a void, never height zero.
use bevy::prelude::*;
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone, Copy)]
struct Face {
    vertices: [Vec3; 3],
    ceiling: bool,
}
impl Face {
    fn height(&self, at: Vec2) -> Option<f32> {
        let [a, b, c] = self.vertices;
        let u = b.xz() - a.xz();
        let v = c.xz() - a.xz();
        let p = at - a.xz();
        let det = u.perp_dot(v);
        if det.abs() < 1e-8 {
            return None;
        }
        let s = p.perp_dot(v) / det;
        let t = u.perp_dot(p) / det;
        // Use a distance tolerance rather than a barycentric tolerance: thin
        // triangles at bent seams need the same sub-millimetre weld as wide ones.
        let epsilon = 1e-5 / det.abs();
        (s >= -epsilon * v.length()
            && t >= -epsilon * u.length()
            && s + t <= 1. + epsilon * (v - u).length())
        .then_some(a.y + s * (b.y - a.y) + t * (c.y - a.y))
    }
    /// Highest feet position needed for a rounded capsule base to touch this
    /// triangle. Maximize the sphere/plane contact over the face and its edges.
    fn rounded_support(&self, at: Vec2, radius: f32) -> Option<f32> {
        let [a, b, c] = self.vertices;
        let normal = (b - a).cross(c - a);
        let mut best = f32::NEG_INFINITY;
        if normal.y.abs() > 1e-8 {
            let gradient = -normal.xz() / normal.y;
            let tangent = at + gradient * (radius / (1. + gradient.length_squared()).sqrt());
            if let Some(y) = self.height(tangent) {
                best = y
                    + (radius * radius - tangent.distance_squared(at))
                        .max(0.)
                        .sqrt()
                    - radius;
            }
        }
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let d = b - a;
            let length = d.xz().length_squared();
            let t = if length > 1e-10 {
                let mid = (at - a.xz()).dot(d.xz()) / length;
                let perpendicular = at.distance_squared(a.xz() + d.xz() * mid);
                if perpendicular > radius * radius {
                    continue;
                }
                (mid + d.y
                    * ((radius * radius - perpendicular) / (length * (length + d.y * d.y))).sqrt())
                .clamp(0., 1.)
            } else if b.y > a.y {
                1.
            } else {
                0.
            };
            let point = a + d * t;
            let remaining = radius * radius - point.xz().distance_squared(at);
            if remaining >= -1e-7 {
                best = best.max(point.y + remaining.max(0.).sqrt() - radius);
            }
        }
        best.is_finite().then_some(best)
    }
}
#[derive(Default)]
pub(crate) struct MeshFloors {
    faces: Vec<Face>,
    bins: HashMap<IVec2, Vec<usize>>,
    support_cache: Mutex<HashMap<[u32; 3], Option<f32>>>,
}
impl MeshFloors {
    pub fn insert(&mut self, vertices: [Vec3; 3], ceiling: bool) {
        self.support_cache.get_mut().unwrap().clear();
        let low = vertices
            .iter()
            .fold(Vec2::splat(f32::INFINITY), |a, p| a.min(p.xz()));
        let high = vertices
            .iter()
            .fold(Vec2::splat(f32::NEG_INFINITY), |a, p| a.max(p.xz()));
        let lo = ((low - Vec2::splat(1e-5)) * 2.).floor().as_ivec2();
        let hi = ((high + Vec2::splat(1e-5)) * 2.).floor().as_ivec2();
        let index = self.faces.len();
        self.faces.push(Face { vertices, ceiling });
        for x in lo.x..=hi.x {
            for z in lo.y..=hi.y {
                self.bins.entry(IVec2::new(x, z)).or_default().push(index);
            }
        }
    }
    pub fn bounds(&self) -> (Vec2, Vec2) {
        self.faces
            .iter()
            .filter(|face| !face.ceiling)
            .flat_map(|face| face.vertices)
            .fold(
                (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
                |(low, high), p| (low.min(p.xz()), high.max(p.xz())),
            )
    }
    pub fn heights(&self, at: Vec2, ceiling: bool) -> Vec<f32> {
        let mut hits: Vec<_> = self.hits(at, ceiling).collect();
        hits.sort_by(f32::total_cmp);
        hits.dedup_by(|a, b| (*a - *b).abs() < 0.001);
        hits
    }
    fn hits(&self, at: Vec2, ceiling: bool) -> impl Iterator<Item = f32> + '_ {
        self.bins
            .get(&(at * 2.).floor().as_ivec2())
            .into_iter()
            .flatten()
            .filter_map(move |i| {
                let face = &self.faces[*i];
                (face.ceiling == ceiling).then(|| face.height(at)).flatten()
            })
    }
    pub fn floor(&self, at: Vec2) -> Option<f32> {
        self.hits(at, false).min_by(f32::total_cmp)
    }
    /// Support on a connected floor mesh for the actor's rounded base. The
    /// center must remain over the mesh; edges never invent support in a void.
    pub fn rounded_support(&self, at: Vec2, radius: f32) -> Option<f32> {
        let key = [at.x.to_bits(), at.y.to_bits(), radius.to_bits()];
        if let Some(cached) = self.support_cache.lock().unwrap().get(&key) {
            return *cached;
        }
        let result = self.uncached_support(at, radius);
        let mut cache = self.support_cache.lock().unwrap();
        if cache.len() >= 4096 {
            cache.clear();
        }
        cache.insert(key, result);
        result
    }
    fn uncached_support(&self, at: Vec2, radius: f32) -> Option<f32> {
        let mut best = self.floor(at)?;
        let low = ((at - Vec2::splat(radius)) * 2.).floor().as_ivec2();
        let high = ((at + Vec2::splat(radius)) * 2.).floor().as_ivec2();
        for x in low.x..=high.x {
            for z in low.y..=high.y {
                if let Some(indices) = self.bins.get(&IVec2::new(x, z)) {
                    for &i in indices {
                        let face = &self.faces[i];
                        if !face.ceiling
                            && let Some(y) = face.rounded_support(at, radius)
                        {
                            best = best.max(y);
                        }
                    }
                }
            }
        }
        Some(best)
    }
    pub fn ceiling(&self, feet: Vec3, radius: f32) -> Option<f32> {
        footprint(feet.xz(), radius)
            .flat_map(|at| self.hits(at, true))
            .filter(|y| *y > feet.y)
            .min_by(f32::total_cmp)
    }
}
pub(crate) fn footprint(at: Vec2, radius: f32) -> impl Iterator<Item = Vec2> {
    std::iter::once(at).chain((0..8).map(move |i| {
        let angle = i as f32 * std::f32::consts::FRAC_PI_4;
        at + Vec2::new(angle.cos(), angle.sin()) * radius
    }))
}
/// Select the first floor a descending body can reach. Small penetration uses
/// the same tolerance as a step; overhead floors cannot steal underground actors.
pub(crate) fn support(levels: impl IntoIterator<Item = f32>, feet: f32, step: f32) -> Option<f32> {
    levels
        .into_iter()
        .filter(|y| y.is_finite() && *y <= feet + step + 0.001)
        .max_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounded_capsule_contact_matches_slopes_and_triangle_edges() {
        let face = Face {
            vertices: [
                Vec3::new(-2., -2., -2.),
                Vec3::new(2., 2., -2.),
                Vec3::new(0., 0., 3.),
            ],
            ceiling: false,
        };
        let radius = 0.28;
        assert!(
            (face.rounded_support(Vec2::ZERO, radius).unwrap() - radius * (2_f32.sqrt() - 1.))
                .abs()
                < 1e-5
        );
        for at in [
            Vec2::new(0., 2.9),
            Vec2::new(-1.9, -1.8),
            Vec2::new(0.2, 0.3),
        ] {
            let exact = face.rounded_support(at, radius).unwrap();
            let mut sampled = f32::NEG_INFINITY;
            let [a, b, c] = face.vertices;
            for i in 0..=400 {
                for j in 0..=400 - i {
                    let p = a + (b - a) * (i as f32 / 400.) + (c - a) * (j as f32 / 400.);
                    let d = radius * radius - p.xz().distance_squared(at);
                    if d >= 0. {
                        sampled = sampled.max(p.y + d.sqrt() - radius);
                    }
                }
            }
            assert!(
                exact >= sampled - 1e-5 && exact - sampled < 0.015,
                "contact {exact} sampled {sampled}"
            );
        }
    }
    #[test]
    fn layered_mesh_preserves_voids_edges_slopes_and_overhead_floors() {
        let mut mesh = MeshFloors::default();
        for base in [-6., 0.] {
            mesh.insert(
                [
                    Vec3::new(0., base, 0.),
                    Vec3::new(2., base + 1., 0.),
                    Vec3::new(0., base, 2.),
                ],
                false,
            );
        }
        assert_eq!(mesh.heights(Vec2::new(1., 0.5), false), vec![-5.5, 0.5]);
        assert_eq!(
            support(mesh.heights(Vec2::new(1., 0.5), false), -4., 0.2),
            Some(-5.5)
        );
        assert_eq!(
            support(mesh.heights(Vec2::new(1., 0.5), false), 4., 0.2),
            Some(0.5)
        );
        assert!(mesh.floor(Vec2::new(3., 0.)).is_none());
        assert_eq!(mesh.floor(Vec2::new(1., 1.)), Some(-5.5));
    }
}
