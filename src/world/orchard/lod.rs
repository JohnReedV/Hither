//! Full botanical detail only where it is visible. Hysteresis prevents
//! repeated swaps at thresholds; all LODs share the same tree placement.
use super::*;

// Separate shadow geometry is never visible to the gameplay camera. The sun
// sees layers 0 and 1; the camera sees only layer 0. This avoids rendering tens
// of millions of sub-texel leaf triangles again in every shadow cascade.
pub(super) fn shadow_layers() -> bevy::camera::visibility::RenderLayers {
    bevy::camera::visibility::RenderLayers::layer(1)
}

pub(super) fn shadow_leaf_clusters(source: &Geometry) -> Geometry {
    let mut mesh = Geometry::default();
    // Input: five-vertex folded leaf blades. Stable selection across the
    // entire crown; slightly broader blades preserve porous canopy coverage.
    for leaf in (0..source.positions.len() / 5).step_by(6) {
        let start = leaf * 5;
        let center = Vec3::from(source.positions[start + 2]);
        let base = mesh.positions.len() as u32;
        for i in start..start + 5 {
            mesh.vertex(
                center + (Vec3::from(source.positions[i]) - center) * 2.25,
                Vec2::from(source.uvs[i]),
                Vec3::ONE,
            );
        }
        for face in [[0, 1, 2], [0, 2, 3], [1, 4, 2], [2, 4, 3]] {
            mesh.triangle(base + face[0], base + face[1], base + face[2]);
        }
    }
    mesh.finish_normals();
    mesh
}

pub(super) fn near(was_near: bool, distance: f32, scale: f32) -> bool {
    let threshold = (7.0 * scale).clamp(5.0, 10.0);
    distance < threshold + if was_near { 1.0 } else { -1.0 }
}
pub(super) fn near_detail(
    was_near: bool,
    distance: f32,
    scale: f32,
    detail: Option<&crate::rendering::view_distance::Detail>,
) -> bool {
    detail.map_or_else(
        || near(was_near, distance, scale),
        |d| d.0 > 0.0 && distance < d.0 + 2.0 * scale + if was_near { 1.0 } else { -1.0 },
    )
}

pub(super) use crate::rendering::geometry::{append, primitive};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shadow_clusters_are_shared_geometry_on_a_camera_excluded_layer() {
        let mut source = Geometry::default();
        for leaf in 0..9 {
            let origin = Vec3::X * leaf as f32;
            for p in [
                Vec3::ZERO,
                Vec3::new(-0.1, 0.2, 0.0),
                Vec3::new(0.0, 0.2, 0.03),
                Vec3::new(0.1, 0.2, 0.0),
                Vec3::Y * 0.4,
            ] {
                source.vertex(origin + p, Vec2::ZERO, Vec3::ONE);
            }
        }
        let shadow = shadow_leaf_clusters(&source);
        assert_eq!(shadow.positions.len(), 10);
        assert_eq!(shadow.indices.len(), 24);
        assert_eq!(shadow.positions, shadow_leaf_clusters(&source).positions);
        assert!(shadow.positions.iter().all(|p| Vec3::from(*p).is_finite()));
        assert!(!shadow_layers().intersects(&bevy::camera::visibility::RenderLayers::default()));
        assert!(
            shadow_layers().intersects(&bevy::camera::visibility::RenderLayers::from_layers(&[
                0, 1
            ]))
        );
    }
    #[test]
    fn zero_detail_disables_near_meshes() {
        let detail = crate::rendering::view_distance::Detail(0.0);
        for was_near in [false, true] {
            assert!(!near_detail(was_near, 0.0, 2.0, Some(&detail)));
        }
    }
    #[test]
    fn thresholds_have_hysteresis_and_scale_with_tree_not_world_grid() {
        assert!(near(true, 7.5, 1.0));
        assert!(!near(false, 7.5, 1.0));
        assert!(near(false, 5.0, 1.0));
        assert!(!near(true, 12.0, 1.7));
        assert!(!near(true, 8.0, 0.55));
    }
}
