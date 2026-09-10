//! Radial visibility with a conservative finite reverse-Z projection.
use bevy::{
    camera::{CameraProjection, SubCameraView},
    math::Vec3A,
    prelude::*,
};
pub const DEFAULT: f32 = 48.0;
pub const MIN: f32 = 16.0;
pub const MAX: f32 = 1024.0;
#[derive(Resource)]
pub struct Detail(pub f32);
impl Default for Detail {
    fn default() -> Self {
        Self(24.0)
    }
}

#[derive(Clone, Debug)]
pub struct FinitePerspective(pub PerspectiveProjection);
impl FinitePerspective {
    pub fn new(far: f32) -> Self {
        Self(PerspectiveProjection {
            fov: 2.0 * 0.66_f32.atan(),
            near: 0.05,
            far,
            ..default()
        })
    }
    fn finite(&self, mut matrix: Mat4) -> Mat4 {
        matrix.z_axis.z = self.0.near / (self.0.far - self.0.near);
        matrix.w_axis.z = self.0.near * self.0.far / (self.0.far - self.0.near);
        matrix
    }
}
impl CameraProjection for FinitePerspective {
    fn get_clip_from_view(&self) -> Mat4 {
        self.finite(self.0.get_clip_from_view())
    }
    fn get_clip_from_view_for_sub(&self, sub: &SubCameraView) -> Mat4 {
        self.finite(self.0.get_clip_from_view_for_sub(sub))
    }
    fn update(&mut self, w: f32, h: f32) {
        self.0.update(w, h);
    }
    fn far(&self) -> f32 {
        self.0.far
    }
    fn get_frustum_corners(&self, n: f32, f: f32) -> [Vec3A; 8] {
        self.0.get_frustum_corners(n, f)
    }
}

#[derive(Resource)]
pub struct Range {
    pub load: f32,
}
impl Default for Range {
    fn default() -> Self {
        Self {
            load: DEFAULT * 2.0,
        }
    }
}
/// Shadow texel density has its own budget, capped by visible world coverage.
/// Keep projection math nonzero even when shadow maps are disabled.
pub(crate) fn shadow_distance(settings: &crate::app::settings::GraphicsSettings) -> f32 {
    settings
        .shadow_distance
        .min(settings.render_distance)
        .max(1.0)
}

pub fn sync(
    settings: Res<crate::app::settings::GraphicsSettings>,
    mut range: ResMut<Range>,
    mut detail: ResMut<Detail>,
    mut cameras: Query<&mut Projection, With<crate::player::avatar::PlayerCamera>>,
    mut shadows: Query<(&mut bevy::light::CascadeShadowConfig, &mut DirectionalLight)>,
) {
    // Visibility is spherical, independent of aspect ratio and camera rotation.
    // Stream around the player with room for the exterior camera offset.
    range.load = settings.render_distance + 4.0;
    let effective = settings.detail_distance.min(settings.render_distance);
    if detail.0 != effective {
        detail.0 = effective;
    }
    if !settings.is_changed() {
        return;
    }
    for (mut config, mut light) in &mut shadows {
        light.shadow_maps_enabled = settings.shadow_distance > 0.0;
        *config = bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 2,
            first_cascade_far_bound: shadow_distance(&settings) * 0.25,
            // Both maps cover the near volume. Radial cascade selection must
            // remain covered at oblique viewing angles as well as straight ahead.
            overlap_proportion: 0.999,
            maximum_distance: shadow_distance(&settings),
            ..default()
        }
        .build();
    }
    for mut projection in &mut cameras {
        if let Projection::Custom(p) = &mut *projection
            && let Some(p) = p.get_mut::<FinitePerspective>()
        {
            p.0.far = settings.render_distance;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_shadow_distance_disables_and_restores_sun_shadows() {
        let mut app = App::new();
        app.init_resource::<crate::app::settings::GraphicsSettings>()
            .init_resource::<Range>()
            .init_resource::<Detail>()
            .add_systems(Update, sync);
        let sun = app
            .world_mut()
            .spawn((
                DirectionalLight::default(),
                bevy::light::CascadeShadowConfigBuilder::default().build(),
            ))
            .id();
        for distance in [24.0, 0.0, 1.0, 24.0] {
            app.world_mut()
                .resource_mut::<crate::app::settings::GraphicsSettings>()
                .shadow_distance = distance;
            app.update();
            assert_eq!(
                app.world()
                    .get::<DirectionalLight>(sun)
                    .unwrap()
                    .shadow_maps_enabled,
                distance > 0.0
            );
            let settings = app
                .world()
                .resource::<crate::app::settings::GraphicsSettings>();
            assert!(shadow_distance(settings) >= 1.0);
        }
    }
    #[test]
    fn shadow_range_is_independent_and_never_exceeds_visibility() {
        let mut settings = crate::app::settings::GraphicsSettings {
            render_distance: 1024.0,
            ..default()
        };
        assert_eq!(shadow_distance(&settings), 24.0);
        settings.detail_distance = 1024.0;
        assert_eq!(shadow_distance(&settings), 24.0);
        settings.shadow_distance = 205.0;
        assert_eq!(shadow_distance(&settings), 205.0);
        settings.shadow_quality = crate::rendering::graphics::Quality::Low;
        assert_eq!(shadow_distance(&settings), 205.0);
        settings.render_distance = 16.0;
        assert_eq!(shadow_distance(&settings), 16.0);
    }
    #[test]
    fn radial_sky_depth_occludes_meshes_beyond_range_at_every_view_angle() {
        for far in [MIN, DEFAULT, MAX] {
            let projection = FinitePerspective::new(far).get_clip_from_view();
            for angle in [0.0_f32, 0.4, 0.8, 1.1] {
                let direction = Vec3::new(angle.sin(), 0.0, -angle.cos());
                let project = |distance: f32| {
                    let clip = projection * (direction * distance).extend(1.0);
                    clip.z / clip.w
                };
                let boundary = project(far);
                assert!(project(far * 0.99) > boundary);
                assert!(project(far * 1.01) < boundary);
            }
        }
        let shader = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/shaders/sdf_scene.wgsl"
        ));
        assert!(shader.contains("let visible_travel = select(uniforms.resolution.w,"));
        assert!(shader.contains("hit.distance > uniforms.resolution.w"));
        assert!(shader.contains("inside_orc_hole(mesh.world_position)"));
        assert!(shader.contains("out.depth = hit_depth;"));
    }
    #[test]
    fn underground_rays_do_not_hit_the_terrain_half_space() {
        let shader = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/shaders/sdf_scene.wgsl"
        ));
        let raymarch = shader
            .split("fn raymarch(")
            .nth(1)
            .unwrap()
            .split("fn scene_normal(")
            .next()
            .unwrap();
        assert!(!raymarch.contains("terrain_height("));
        assert!(!raymarch.contains("2048"));
        assert!(!raymarch.contains("map_scene("));
        assert!(!raymarch.contains("castle_distance("));
    }
    #[test]
    fn meshes_and_sdf_share_reverse_depth_and_cutoff() {
        for far in [MIN, DEFAULT, MAX] {
            let p = FinitePerspective::new(far);
            let m = p.get_clip_from_view();
            for z in [0.05, 1.0, far, far + 1.0] {
                let q = m * Vec4::new(0.0, 0.0, -z, 1.0);
                let sdf = (0.05 * far / z - 0.05) / (far - 0.05);
                assert!((q.z / q.w - sdf).abs() < 1e-6);
                if z > far {
                    assert!(q.z < 0.0);
                }
            }
        }
    }

    #[test]
    fn terrain_color_and_camera_depth_clip_together_but_shadows_keep_casters() {
        let shader = include_str!("../../assets/shaders/sdf_scene.wgsl");
        assert!(shader.contains("@invariant @builtin(position) position:"));
        let clipping = shader
            .split("let hit = surface(length(offset), 1.0);")
            .nth(1)
            .unwrap()
            .split("if inside_orc_hole")
            .next()
            .unwrap();
        // Evaluate the actual shader guards for each view, rather than merely
        // checking that a discard occurs somewhere in the source file.
        for (prepass, world_camera, expected) in [
            (false, true, true),
            (true, true, true),
            (true, false, false),
        ] {
            let mut enabled = vec![true];
            let mut clips = false;
            for line in clipping.lines().map(str::trim) {
                if line.starts_with("#ifdef ") || line.starts_with("#ifndef ") {
                    let value = match line.split_whitespace().nth(1).unwrap() {
                        "PREPASS_PIPELINE" => prepass,
                        "VIEW_PROJECTION_NONSTANDARD" => world_camera,
                        other => panic!("Review new depth condition: {other}"),
                    };
                    enabled.push(if line.starts_with("#ifndef") {
                        !value
                    } else {
                        value
                    });
                } else if line == "#else" {
                    let last = enabled.last_mut().unwrap();
                    *last = !*last;
                } else if line == "#endif" {
                    enabled.pop();
                } else if line.contains("discard;") && enabled.iter().all(|v| *v) {
                    clips = true;
                }
            }
            assert_eq!(
                clips, expected,
                "prepass={prepass}, world_camera={world_camera}"
            );
        }
    }
}
