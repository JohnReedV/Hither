use super::*;
use crate::world::orchard::fruit::APPLE_RADIUS;

#[test]
fn mesh_compaction_preserves_visible_attributes() {
    use bevy::mesh::VertexAttributeValues;
    let mut g = Geometry::default();
    g.vertex(Vec3::splat(99.0), Vec2::ZERO, Vec3::ONE); // unused
    g.vertex(Vec3::ZERO, Vec2::ZERO, Vec3::X);
    g.vertex(Vec3::X, Vec2::X, Vec3::Y);
    g.vertex(Vec3::Y, Vec2::Y, Vec3::Z);
    g.triangle(1, 2, 3);
    g.triangle(0, 0, 0); // no visible surface
    g.finish_normals();
    let tangents = g.tangents();
    let mesh = g.mesh();
    assert!(matches!(mesh.indices(), Some(Indices::U16(i)) if i == &[0, 1, 2]));
    for (attribute, expected) in [
        (Mesh::ATTRIBUTE_POSITION, &g.positions),
        (Mesh::ATTRIBUTE_NORMAL, &g.normals),
    ] {
        let Some(VertexAttributeValues::Float32x3(actual)) = mesh.attribute(attribute) else {
            panic!("missing attribute");
        };
        assert_eq!(actual, &expected[1..]);
    }
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
        panic!("missing UVs");
    };
    assert_eq!(uvs, &g.uvs[1..]);
    for (attribute, expected) in [
        (Mesh::ATTRIBUTE_COLOR, &g.colors),
        (Mesh::ATTRIBUTE_TANGENT, &tangents),
    ] {
        let Some(VertexAttributeValues::Float32x4(actual)) = mesh.attribute(attribute) else {
            panic!("missing attribute");
        };
        assert_eq!(actual, &expected[1..]);
    }
    assert_eq!(g.positions.len(), 4);
    assert_eq!(g.indices.len(), 6);
}

#[test]
fn procedural_tangents_follow_uvs_and_handle_collapsed_tips() {
    for mirror in [1.0, -1.0] {
        let mut g = Geometry::default();
        g.vertex(Vec3::ZERO, Vec2::ZERO, Vec3::ONE);
        g.vertex(Vec3::X, Vec2::X * mirror, Vec3::ONE);
        g.vertex(Vec3::Y, Vec2::Y, Vec3::ONE);
        g.vertex(Vec3::ZERO, Vec2::ZERO, Vec3::ONE);
        g.triangle(0, 1, 2);
        g.triangle(0, 3, 3); // collapsed blade/pole: finite fallback
        g.finish_normals();
        let tangents = g.tangents();
        for t in &tangents[..3] {
            assert!(Vec3::from_slice(t).distance(Vec3::X * mirror) < 1e-5);
            assert_eq!(t[3], mirror);
        }
        for (n, t) in g.normals.iter().zip(tangents) {
            let t = Vec4::from(t);
            assert!(t.is_finite());
            assert!((t.truncate().length() - 1.0).abs() < 1e-5);
            assert!(Vec3::from(*n).dot(t.truncate()).abs() < 1e-5);
        }
    }
}
#[test]
fn clearance_detects_diagonal_leaves_and_triangle_edges() {
    let blade = Triangle([
        Vec3::new(0.07, 0.06, 0.07),
        Vec3::new(0.10, 0.06, 0.07),
        Vec3::new(0.07, 0.10, 0.07),
    ]);
    let mesh = RayMesh::new(std::iter::once(blade));
    for direction in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
        assert!(
            mesh.hit(Vec3::ZERO, direction, APPLE_RADIUS * 1.12)
                .is_none()
        );
    }
    assert!(mesh.overlaps_sphere(Vec3::ZERO, APPLE_RADIUS * 1.12));
    assert!(!mesh.overlaps_sphere(Vec3::ZERO, 0.05));
    let flat = Triangle([Vec3::ZERO, Vec3::X, Vec3::Y]);
    assert!((flat.distance_squared(Vec3::new(0.2, 0.2, 0.1)) - 0.01).abs() < 1e-6);
    assert!((flat.distance_squared(Vec3::new(-1.0, 0.0, 0.0)) - 1.0).abs() < 1e-6);
    assert!((flat.distance_squared(Vec3::new(0.5, -1.0, 0.0)) - 1.0).abs() < 1e-6);
}
