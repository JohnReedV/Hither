use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
#[derive(Default, Clone)]
pub(crate) struct Geometry {
    pub(crate) positions: Vec<[f32; 3]>,
    pub(crate) normals: Vec<[f32; 3]>,
    pub(crate) uvs: Vec<[f32; 2]>,
    pub(crate) colors: Vec<[f32; 4]>,
    pub(crate) indices: Vec<u32>,
}

impl Geometry {
    pub(crate) fn vertex(&mut self, p: Vec3, uv: Vec2, color: Vec3) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push(p.to_array());
        self.normals.push([0.0; 3]);
        self.uvs.push(uv.to_array());
        self.colors.push(color.extend(1.0).to_array());
        i
    }

    pub(crate) fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }

    pub(crate) fn finish_normals(&mut self) {
        self.normals.fill([0.0; 3]);
        for face in self.indices.chunks_exact(3) {
            let [a, b, c] = [face[0] as usize, face[1] as usize, face[2] as usize];
            let n = (Vec3::from(self.positions[b]) - Vec3::from(self.positions[a]))
                .cross(Vec3::from(self.positions[c]) - Vec3::from(self.positions[a]));
            for i in [a, b, c] {
                self.normals[i] = (Vec3::from(self.normals[i]) + n).to_array();
            }
        }
        for n in &mut self.normals {
            *n = Vec3::from(*n).normalize_or(Vec3::Y).to_array();
        }
    }

    pub(crate) fn mesh(&self) -> Mesh {
        // Lossless upload compaction: collapsed leaf tips/branch caps contain
        // zero-area faces and unused vertices. They cannot cover a pixel.
        // Preserve the original normals/tangents; do not weld shading seams.
        let tangents = self.tangents();
        let mut remap = vec![u32::MAX; self.positions.len()];
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        let mut compact_tangents = Vec::new();
        let mut indices = Vec::with_capacity(self.indices.len());
        for face in self.indices.chunks_exact(3) {
            let [a, b, c] = face.try_into().unwrap();
            let edge_a =
                Vec3::from(self.positions[b as usize]) - Vec3::from(self.positions[a as usize]);
            let edge_b =
                Vec3::from(self.positions[c as usize]) - Vec3::from(self.positions[a as usize]);
            if edge_a.cross(edge_b).length_squared() == 0.0 {
                continue;
            }
            for &index in face {
                let i = index as usize;
                if remap[i] == u32::MAX {
                    remap[i] = positions.len() as u32;
                    positions.push(self.positions[i]);
                    normals.push(self.normals[i]);
                    uvs.push(self.uvs[i]);
                    colors.push(self.colors[i]);
                    compact_tangents.push(tangents[i]);
                }
                indices.push(remap[i]);
            }
        }
        let indices = if positions.len() <= u16::MAX as usize + 1 {
            Indices::U16(indices.into_iter().map(|i| i as u16).collect())
        } else {
            Indices::U32(indices)
        };
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, compact_tangents)
        .with_inserted_indices(indices)
    }

    /// Partition an already shaded mesh by local spatial cells, with a bounded
    /// triangle payload. Copy attributes exactly, including authored tangents.
    /// Den lamps reach 14m: 32m bins span entire rooms and send nearly every
    /// triangle to several cubemap faces even when only a small part can cast.
    pub(crate) fn spatial_meshes(&self, cell_size: f32) -> Vec<Mesh> {
        assert!(cell_size.is_finite() && cell_size > 0.0);
        use bevy::mesh::VertexAttributeValues as V;
        let mesh = self.mesh();
        let mut bins: std::collections::BTreeMap<[i32; 3], Vec<usize>> = default();
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        let V::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() else {
            unreachable!()
        };
        for tri in indices.chunks_exact(3) {
            let center = (Vec3::from(positions[tri[0]])
                + Vec3::from(positions[tri[1]])
                + Vec3::from(positions[tri[2]]))
                / 3.0;
            bins.entry((center / cell_size).floor().as_ivec3().to_array())
                .or_default()
                .extend_from_slice(tri);
        }
        let mut out = Vec::new();
        for indices in bins.values() {
            for faces in indices.chunks(12_000) {
                let mut remap = std::collections::HashMap::new();
                let mut vertices = Vec::new();
                let indices: Vec<u16> = faces
                    .iter()
                    .map(|i| {
                        *remap.entry(*i).or_insert_with(|| {
                            vertices.push(*i);
                            (vertices.len() - 1) as u16
                        })
                    })
                    .collect();
                let mut part = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                );
                for (attribute, values) in mesh.attributes() {
                    let selected = match values {
                        V::Float32x2(v) => V::Float32x2(vertices.iter().map(|&i| v[i]).collect()),
                        V::Float32x3(v) => V::Float32x3(vertices.iter().map(|&i| v[i]).collect()),
                        V::Float32x4(v) => V::Float32x4(vertices.iter().map(|&i| v[i]).collect()),
                        _ => unreachable!("Geometry attributes changed"),
                    };
                    part.insert_attribute(*attribute, selected);
                }
                part.insert_indices(Indices::U16(indices));
                out.push(part);
            }
        }
        out
    }

    // These procedural meshes already split vertices at UV seams. Accumulate
    // their UV derivatives directly in linear time, avoiding MikkTSpace's
    // expensive welding/grouping of thousands of coincident leaf tips.
    // Imported avatar meshes keep their authored MikkTSpace tangents.
    pub(crate) fn tangents(&self) -> Vec<[f32; 4]> {
        let mut tangent = vec![Vec3::ZERO; self.positions.len()];
        let mut bitangent = tangent.clone();
        for tri in self.indices.chunks_exact(3) {
            let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            let e1 = Vec3::from(self.positions[b]) - Vec3::from(self.positions[a]);
            let e2 = Vec3::from(self.positions[c]) - Vec3::from(self.positions[a]);
            let uv1 = Vec2::from(self.uvs[b]) - Vec2::from(self.uvs[a]);
            let uv2 = Vec2::from(self.uvs[c]) - Vec2::from(self.uvs[a]);
            let determinant = uv1.perp_dot(uv2);
            if determinant.abs() < 1e-12 {
                continue;
            }
            let t = (e1 * uv2.y - e2 * uv1.y) / determinant;
            let b_vec = (e2 * uv1.x - e1 * uv2.x) / determinant;
            for i in [a, b, c] {
                tangent[i] += t;
                bitangent[i] += b_vec;
            }
        }
        tangent
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let n = Vec3::from(self.normals[i]).normalize_or(Vec3::Y);
                let t = (*t - n * n.dot(*t)).normalize_or(n.any_orthonormal_vector());
                let sign = if n.cross(t).dot(bitangent[i]) < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                t.extend(sign).to_array()
            })
            .collect()
    }

    pub(crate) fn triangles(&self) -> impl Iterator<Item = Triangle> + '_ {
        self.indices
            .chunks_exact(3)
            .map(|t| Triangle(t.map_indices(&self.positions)))
    }
}

// Keep the triangle representation independent of Bevy's render asset lifetime.
pub(crate) trait TriangleIndices {
    fn map_indices(&self, positions: &[[f32; 3]]) -> [Vec3; 3];
}
impl TriangleIndices for [u32] {
    fn map_indices(&self, positions: &[[f32; 3]]) -> [Vec3; 3] {
        [0, 1, 2].map(|i| Vec3::from(positions[self[i] as usize]))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Triangle(pub(crate) [Vec3; 3]);

impl Triangle {
    pub(crate) fn distance_squared(self, point: Vec3) -> f32 {
        let [a, b, c] = self.0;
        let normal = (b - a).cross(c - a);
        let n2 = normal.length_squared();
        if n2 > 1e-20 {
            let projected = point - normal * ((point - a).dot(normal) / n2);
            if [(a, b), (b, c), (c, a)]
                .iter()
                .all(|(u, v)| (*v - *u).cross(projected - *u).dot(normal) >= 0.0)
            {
                return point.distance_squared(projected);
            }
        }
        // Outside the face (or a degenerate triangle): nearest edge/vertex.
        [(a, b), (b, c), (c, a)]
            .iter()
            .map(|(u, v)| {
                let edge = *v - *u;
                let t = ((point - *u).dot(edge) / edge.length_squared().max(1e-20)).clamp(0.0, 1.0);
                point.distance_squared(*u + edge * t)
            })
            .fold(f32::INFINITY, f32::min)
    }

    // Barycentric containment for vertical rays does not depend on the ray's
    // height. Filter the column once, then retain the original per-level hit
    // arithmetic only for triangles actually under this footprint.
    fn contains_vertical(self, p: Vec2) -> bool {
        let [a, b, c] = self.0;
        let e1 = b - a;
        let e2 = c - a;
        let h = (-Vec3::Y).cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-9 {
            return false;
        }
        let s = Vec3::new(p.x, 0.0, p.y) - a;
        let u = s.dot(h) / det;
        let v = -s.cross(e1).y / det;
        u >= -1e-5 && v >= -1e-5 && u + v <= 1.0 + 1e-5
    }

    pub(crate) fn hit(self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let [a, b, c] = self.0;
        let e1 = b - a;
        let e2 = c - a;
        let h = direction.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-9 {
            return None;
        }
        let s = origin - a;
        let u = s.dot(h) / det;
        let q = s.cross(e1);
        let v = direction.dot(q) / det;
        let t = e2.dot(q) / det;
        // Include shared edges within floating-point roundoff. Without this,
        // an exactly equatorial ray can fall between two neighboring triangles.
        (u >= -1e-5 && v >= -1e-5 && u + v <= 1.0 + 1e-5 && t >= 0.0).then_some(t)
    }
}

pub(crate) struct BvhNode {
    pub(crate) min: Vec3,
    pub(crate) max: Vec3,
    pub(crate) children: Option<[usize; 2]>,
    pub(crate) range: std::ops::Range<usize>,
}

pub(crate) struct RayMesh {
    vertical_columns: Option<std::collections::HashMap<IVec2, Vec<usize>>>,
    pub(crate) triangles: Vec<Triangle>,
    pub(crate) nodes: Vec<BvhNode>,
}

impl RayMesh {
    pub(crate) fn overlaps_sphere(&self, center: Vec3, radius: f32) -> bool {
        let mut stack = vec![0];
        let r2 = radius * radius;
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            if center.distance_squared(center.clamp(node.min, node.max)) > r2 {
                continue;
            }
            if let Some(children) = node.children {
                stack.extend(children);
            } else if self.triangles[node.range.clone()]
                .iter()
                .any(|t| t.distance_squared(center) <= r2)
            {
                return true;
            }
        }
        false
    }

    pub(crate) fn new(triangles: impl Iterator<Item = Triangle>) -> Self {
        let mut result = Self {
            vertical_columns: None,
            triangles: triangles.collect(),
            nodes: Vec::new(),
        };
        result.split(0..result.triangles.len());
        result
    }
    /// The immutable den support mesh is queried vertically at many actor
    /// footprints. Build its XZ broad phase on the meshing worker once.
    pub(crate) fn new_floor(triangles: impl Iterator<Item = Triangle>) -> Self {
        let mut mesh = Self::new(triangles);
        let mut columns: std::collections::HashMap<IVec2, Vec<usize>> = default();
        for (index, triangle) in mesh.triangles.iter().enumerate() {
            let low = triangle
                .0
                .iter()
                .fold(Vec2::splat(f32::INFINITY), |a, p| a.min(p.xz()))
                .floor()
                .as_ivec2();
            let high = triangle
                .0
                .iter()
                .fold(Vec2::splat(f32::NEG_INFINITY), |a, p| a.max(p.xz()))
                .floor()
                .as_ivec2();
            for x in low.x..=high.x {
                for z in low.y..=high.y {
                    columns.entry(IVec2::new(x, z)).or_default().push(index);
                }
            }
        }
        mesh.vertical_columns = Some(columns);
        mesh
    }
    pub(crate) fn split(&mut self, range: std::ops::Range<usize>) -> usize {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for t in &self.triangles[range.clone()] {
            for p in t.0 {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let node = self.nodes.len();
        self.nodes.push(BvhNode {
            min,
            max,
            children: None,
            range: range.clone(),
        });
        if range.len() > 12 {
            let size = max - min;
            let axis = if size.x > size.y && size.x > size.z {
                0
            } else if size.y > size.z {
                1
            } else {
                2
            };
            let mid = range.start + range.len() / 2;
            self.triangles[range.clone()].select_nth_unstable_by(range.len() / 2, |a, b| {
                let center = |t: &Triangle| t.0.iter().map(|p| p[axis]).sum::<f32>();
                center(a).total_cmp(&center(b))
            });
            let left = self.split(range.start..mid);
            let right = self.split(mid..range.end);
            self.nodes[node].children = Some([left, right]);
        }
        node
    }
    /// Resolve all floors with one column lookup when indexed. Keep the exact
    /// nearest-hit restart arithmetic: thin triangles can otherwise move by
    /// millimetres when evaluated from a different ray origin.
    pub(crate) fn downward_hits(&self, origin: Vec3, bottom: f32) -> Vec<f32> {
        let mut heights = Vec::new();
        if let Some(columns) = &self.vertical_columns {
            let Some(indices) = columns.get(&origin.xz().floor().as_ivec2()) else {
                return heights;
            };
            let candidates: Vec<_> = indices
                .iter()
                .copied()
                .filter(|&index| self.triangles[index].contains_vertical(origin.xz()))
                .collect();
            let mut y = origin.y;
            for _ in 0..24 {
                let mut nearest = y - bottom;
                let mut found = false;
                for &index in &candidates {
                    if let Some(t) = self.triangles[index].hit(origin.with_y(y), -Vec3::Y)
                        && t < nearest
                    {
                        nearest = t;
                        found = true;
                    }
                }
                if !found {
                    break;
                }
                y -= nearest;
                heights.push(y);
                y -= 0.025;
            }
            return heights;
        } else {
            let mut stack = vec![0];
            while let Some(index) = stack.pop() {
                let node = &self.nodes[index];
                if origin.x < node.min.x
                    || origin.x > node.max.x
                    || origin.z < node.min.z
                    || origin.z > node.max.z
                    || node.min.y > origin.y
                    || node.max.y <= bottom
                {
                    continue;
                }
                if let Some(children) = node.children {
                    stack.extend(children);
                } else {
                    for triangle in &self.triangles[node.range.clone()] {
                        if let Some(t) = triangle.hit(origin, -Vec3::Y) {
                            let y = origin.y - t;
                            if y > bottom {
                                heights.push(y);
                            }
                        }
                    }
                }
            }
        }
        heights.sort_unstable_by(|a, b| b.total_cmp(a));
        // Match the support solver's 2.5cm restart gap and 24-floor bound.
        let mut unique = Vec::new();
        for y in heights {
            if unique.last().is_none_or(|last: &f32| y <= *last - 0.025) {
                unique.push(y);
                if unique.len() == 24 {
                    break;
                }
            }
        }
        unique
    }
    pub(crate) fn hit(&self, origin: Vec3, direction: Vec3, limit: f32) -> Option<f32> {
        let mut nearest = limit;
        let mut found = false;
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            let mut near: f32 = 0.0;
            let mut far = nearest;
            for axis in 0..3 {
                if direction[axis].abs() < 1e-8 {
                    if origin[axis] < node.min[axis] || origin[axis] > node.max[axis] {
                        far = -1.0;
                        break;
                    }
                } else {
                    let a = (node.min[axis] - origin[axis]) / direction[axis];
                    let b = (node.max[axis] - origin[axis]) / direction[axis];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                }
            }
            if far < near {
                continue;
            }
            if let Some(children) = node.children {
                stack.extend(children);
            } else {
                for t in &self.triangles[node.range.clone()] {
                    if let Some(distance) = t.hit(origin, direction)
                        && distance < nearest
                    {
                        nearest = distance;
                        found = true;
                    }
                }
            }
        }
        found.then_some(nearest)
    }
}

pub(crate) fn primitive(mesh: Mesh) -> Geometry {
    use bevy::mesh::VertexAttributeValues;
    let VertexAttributeValues::Float32x3(positions) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        unreachable!()
    };
    let VertexAttributeValues::Float32x3(normals) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
    else {
        unreachable!()
    };
    let VertexAttributeValues::Float32x2(uvs) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
    else {
        unreachable!()
    };
    Geometry {
        positions: positions.clone(),
        normals: normals.clone(),
        uvs: uvs.clone(),
        colors: vec![[1.0; 4]; positions.len()],
        indices: mesh.indices().unwrap().iter().map(|i| i as u32).collect(),
    }
}

pub(crate) fn append(target: &mut Geometry, source: &Geometry, transform: Transform) {
    let base = target.positions.len() as u32;
    for i in 0..source.positions.len() {
        target.positions.push(
            transform
                .transform_point(Vec3::from(source.positions[i]))
                .to_array(),
        );
        // Inverse transpose for nonuniformly scaled stems/calyxes.
        target.normals.push(
            (transform.rotation * (Vec3::from(source.normals[i]) / transform.scale))
                .normalize_or(Vec3::Y)
                .to_array(),
        );
    }
    target.uvs.extend_from_slice(&source.uvs);
    target.colors.extend_from_slice(&source.colors);
    target
        .indices
        .extend(source.indices.iter().map(|i| base + i));
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod spatial_tests {
    use super::*;
    #[test]
    fn spatial_parts_preserve_shading_and_bound_uploads() {
        let mut geometry = Geometry::default();
        for i in 0..5000 {
            let x = (i % 100) as f32;
            let z = (i / 100) as f32;
            let base = geometry.vertex(Vec3::new(x, 0., z), Vec2::ZERO, Vec3::ONE);
            geometry.vertex(Vec3::new(x, 0., z + 0.5), Vec2::Y, Vec3::ONE);
            geometry.vertex(Vec3::new(x + 0.5, 0., z), Vec2::X, Vec3::ONE);
            geometry.triangle(base, base + 1, base + 2);
        }
        geometry.finish_normals();
        let source = geometry.mesh();
        let parts = geometry.spatial_meshes(4.0);
        assert!(parts.len() > 4);
        assert_eq!(
            parts
                .iter()
                .map(|m| m.indices().unwrap().len())
                .sum::<usize>(),
            source.indices().unwrap().len()
        );
        for part in &parts {
            assert!(crate::world::streaming::mesh_bytes(part) <= 1024 * 1024);
            assert!(part.count_vertices() <= 12000);
            assert!(part.attribute(Mesh::ATTRIBUTE_TANGENT).is_some());
            use bevy::camera::primitives::MeshAabb;
            let bounds = part.compute_aabb().unwrap();
            assert!(bounds.half_extents.x <= 2.0 && bounds.half_extents.z <= 2.0);
        }
    }

    #[test]
    fn spatial_partition_preserves_every_triangle_attribute_and_winding() {
        use bevy::mesh::VertexAttributeValues;
        let mut geometry = Geometry::default();
        for x in [-9.0, -4.1, -4.0, -0.1, 0.0, 3.9, 4.0, 15.0] {
            // Include triangles spanning cell boundaries: their actual bounds
            // must include the whole triangle, never just the centroid's cell.
            let a = geometry.vertex(Vec3::new(x, 0., -1.), Vec2::ZERO, Vec3::X);
            geometry.vertex(Vec3::new(x + 6., 1., 0.), Vec2::X, Vec3::Y);
            geometry.vertex(Vec3::new(x, 0., 2.), Vec2::Y, Vec3::Z);
            geometry.triangle(a, a + 1, a + 2);
        }
        geometry.finish_normals();
        let triangles = |mesh: &Mesh| -> Vec<Vec<u32>> {
            let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
            indices
                .chunks_exact(3)
                .map(|face| {
                    let mut bits = Vec::new();
                    for &i in face {
                        for (_, values) in mesh.attributes() {
                            let value: &[f32] = match values {
                                VertexAttributeValues::Float32x2(v) => &v[i],
                                VertexAttributeValues::Float32x3(v) => &v[i],
                                VertexAttributeValues::Float32x4(v) => &v[i],
                                _ => panic!("unexpected geometry attribute"),
                            };
                            bits.extend(value.iter().map(|v| v.to_bits()));
                        }
                    }
                    bits
                })
                .collect()
        };
        let mut before = triangles(&geometry.mesh());
        let mut after: Vec<_> = geometry
            .spatial_meshes(4.0)
            .iter()
            .flat_map(triangles)
            .collect();
        before.sort();
        after.sort();
        assert_eq!(before, after);
    }
}

#[cfg(test)]
mod ray_levels_tests {
    use super::*;
    #[test]
    fn all_levels_match_repeated_nearest_hits() {
        let triangles = (-10..20).flat_map(|i| {
            let y = i as f32 * 0.3;
            [
                Triangle([
                    Vec3::new(-2., y, -2.),
                    Vec3::new(2., y, -2.),
                    Vec3::new(2., y, 2.),
                ]),
                Triangle([
                    Vec3::new(-2., y, -2.),
                    Vec3::new(2., y, 2.),
                    Vec3::new(-2., y, 2.),
                ]),
            ]
        });
        let mesh = RayMesh::new_floor(triangles);
        for x in [-3., -1., 0., 1., 3.] {
            let origin = Vec3::new(x, 10., 0.);
            let mut y = origin.y;
            let mut reference = Vec::new();
            for _ in 0..24 {
                let Some(t) = mesh.hit(origin.with_y(y), -Vec3::Y, y + 30.) else {
                    break;
                };
                y -= t;
                reference.push(y);
                y -= 0.025;
            }
            let actual = mesh.downward_hits(origin, -30.);
            assert_eq!(reference.len(), actual.len());
            for (a, b) in actual.iter().zip(reference) {
                assert!((*a - b).abs() < 0.00001);
            }
        }
    }
}
