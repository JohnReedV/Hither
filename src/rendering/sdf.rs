use crate::app::settings::GraphicsSettings;
use crate::player::camera::{CameraRig, CameraView};
use crate::world::{biome, snow};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

pub(crate) const SHADER_PATH: &str = "shaders/sdf_scene.wgsl";
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub(crate) struct SdfUniforms {
    pub(crate) camera_position: Vec4,
    pub(crate) camera_forward: Vec4,
    pub(crate) camera_right: Vec4,
    pub(crate) camera_up: Vec4,
    pub(crate) resolution: Vec4,
    pub(crate) player_position: Vec4,
    pub(crate) world_seed: UVec4,
    pub(crate) tracks: [Vec4; snow::MAX_TRACKS],
    pub(crate) tracks_meta: Vec4,
    pub(crate) tracks_bounds: Vec4,
    pub(crate) track_bins: snow::TrackBins,
    pub(crate) shadow_quality: UVec4,
    pub(crate) goblin_portals: [Vec4; crate::world::goblin_dens::MAX_PORTAL_SEGMENTS * 2],
    pub(crate) goblin_limits: [Vec4; 128],
    pub(crate) orc_holes: [Vec4; crate::world::orcs::MAX_HOLES],
    pub(crate) cutouts: CutoutBins,
}

/// Conservative world-space broad phase shared by every color/depth/shadow variant.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub(crate) struct CutoutBins {
    bounds: Vec4,
    goblins: [UVec4; 256],
    orcs: [UVec4; 64],
}
impl Default for CutoutBins {
    fn default() -> Self {
        Self {
            bounds: Vec4::ZERO,
            goblins: [UVec4::ZERO; 256],
            orcs: [UVec4::ZERO; 64],
        }
    }
}
impl CutoutBins {
    fn build(position: Vec3, distance: f32, holes: &[Vec4; 32], portals: &[Vec4; 256]) -> Self {
        if holes[0].z <= 0.0 && portals[0].w <= 0.0 {
            return Self::default();
        }
        let size = ((distance * 2.0 + 128.0) / 16.0).max(8.0);
        let low = (position.xz() / size).floor() * size - Vec2::splat(size * 7.0);
        let mut result = Self {
            bounds: Vec4::new(low.x, low.y, size, 1.0 / size),
            ..default()
        };
        let cells = |min: Vec2, max: Vec2| {
            let a = ((min - low) / size).floor().as_ivec2().max(IVec2::ZERO);
            let b = ((max - low) / size)
                .floor()
                .as_ivec2()
                .min(IVec2::splat(15));
            (a.y..=b.y).flat_map(move |y| (a.x..=b.x).map(move |x| (y * 16 + x) as usize))
        };
        for (i, h) in holes.iter().enumerate().take_while(|(_, h)| h.z > 0.0) {
            for cell in cells(h.xy() - h.zw().abs(), h.xy() + h.zw().abs()) {
                result.orcs[cell / 4][cell % 4] |= 1 << i;
            }
        }
        for (i, pair) in portals
            .chunks_exact(2)
            .enumerate()
            .take_while(|(_, p)| p[0].w > 0.0)
        {
            // The narrow phase only evaluates points within sqrt(15) of the segment.
            let a = pair[0].xz();
            let b = pair[1].xz();
            for cell in cells(
                a.min(b) - Vec2::splat(15.0_f32.sqrt()),
                a.max(b) + Vec2::splat(15.0_f32.sqrt()),
            ) {
                result.goblins[cell][i / 32] |= 1 << (i % 32);
            }
        }
        result
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(SdfMaterialKey)]
pub(crate) struct SdfMaterial {
    #[uniform(0)]
    pub(crate) uniforms: SdfUniforms,
    #[storage(1, read_only, buffer)]
    pub(crate) contacts: bevy::render::render_resource::Buffer,
    #[storage(2, read_only, buffer)]
    pub(crate) surface_pixels: bevy::render::render_resource::Buffer,
    #[storage(3, read_only, buffer)]
    pub(crate) surface_pages: bevy::render::render_resource::Buffer,
    #[storage(4, read_only, buffer)]
    pub(crate) rock_surfaces: bevy::render::render_resource::Buffer,
    #[storage(5, read_only, buffer)]
    pub(crate) terrain_samples: bevy::render::render_resource::Buffer,
    pub(crate) terrain: bool,
    pub(crate) rock: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SdfMaterialKey {
    pub(crate) terrain: bool,
    pub(crate) rock: bool,
}
impl From<&SdfMaterial> for SdfMaterialKey {
    fn from(material: &SdfMaterial) -> Self {
        Self {
            terrain: material.terrain,
            rock: material.rock,
        }
    }
}

impl Material for SdfMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Main and prepass pipelines must use the same attribute locations.
        let mut attributes = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
        if key.bind_group_data.terrain || key.bind_group_data.rock {
            attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(1));
        }
        if key.bind_group_data.rock {
            attributes.push(crate::world::geology::contact::INDEX.at_shader_location(2));
        }
        descriptor.vertex.buffers = vec![layout.0.get_layout(&attributes)?];
        if key.bind_group_data.terrain || key.bind_group_data.rock {
            descriptor.vertex.shader_defs.push("TERRAIN_MESH".into());
            descriptor
                .fragment
                .as_mut()
                .unwrap()
                .shader_defs
                .push("TERRAIN_MESH".into());
        }
        if key.bind_group_data.rock {
            descriptor.vertex.shader_defs.push("ROCK_MESH".into());
            descriptor
                .fragment
                .as_mut()
                .unwrap()
                .shader_defs
                .push("ROCK_MESH".into());
        }
        Ok(())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn enable_prepass() -> bool {
        super::occlusion::mode().depth()
    }

    fn alpha_mode(&self) -> AlphaMode {
        if !super::occlusion::mode().depth() {
            return AlphaMode::Opaque;
        }
        // These opaque surfaces discard cave holes and write ray-marched depth.
        // MAY_DISCARD keeps Bevy from substituting a vertex-only depth pass.
        AlphaMode::Mask(0.5)
    }
    fn enable_shadows() -> bool {
        true
    }
}

#[derive(Resource)]
pub(crate) struct SdfMaterialHandle(
    pub(crate) Handle<SdfMaterial>,
    pub(crate) Handle<SdfMaterial>,
    pub(crate) Handle<SdfMaterial>,
);

#[derive(Component)]
pub(crate) struct SdfCanvas;

#[allow(clippy::too_many_arguments)] // Bevy system inputs share the current camera snapshot.
pub(crate) fn update_shader_uniforms(
    settings: Res<GraphicsSettings>,
    tracks: Res<snow::Tracks>,
    mut bins: Local<(u64, snow::TrackBins)>,
    window: Single<&Window>,
    rig: Res<CameraRig>,
    view: Res<CameraView>,
    material_handle: Res<SdfMaterialHandle>,
    mut materials: ResMut<Assets<SdfMaterial>>,
    mut procedural_bounds: Query<
        &mut bevy::camera::primitives::Aabb,
        (
            With<bevy::camera::visibility::NoFrustumCulling>,
            With<MeshMaterial3d<SdfMaterial>>,
        ),
    >,
) {
    // These vertices are positioned by the shader, not their entity transform.
    // NoFrustumCulling alone does NOT bypass GPU occlusion tests. Keep their
    // bounds enclosing the camera and the entire rendered radial volume so the
    // fullscreen canvas and displaced terrain can never cull themselves.
    for mut bounds in &mut procedural_bounds {
        bounds.set_if_neq(bevy::camera::primitives::Aabb {
            center: view.position.into(),
            half_extents: Vec3::splat(settings.render_distance * 2.0).into(),
        });
    }
    let revision = tracks.revision;
    let (tracks, tracks_meta, tracks_bounds) = tracks.uniforms();
    if bins.0 != revision {
        *bins = (
            revision,
            snow::TrackBins::build(&tracks[..tracks_meta.x as usize]),
        );
    }
    let orc_holes = crate::world::orcs::hole_uniforms(view.position, settings.render_distance);
    let goblin_portals =
        crate::world::goblin_dens::portal_uniforms(view.position, settings.render_distance);
    let uniforms = SdfUniforms {
        camera_position: view
            .position
            .extend(settings.detail_distance.min(settings.render_distance)),
        camera_forward: view
            .forward
            .extend(if crate::rendering::lighting::validation_dark() {
                1.0
            } else {
                0.0
            }),
        camera_right: view.right.extend(0.0),
        camera_up: view.up.extend(0.0),
        resolution: Vec4::new(
            window.width(),
            window.height(),
            biome::snow_amount(view.position.xz()),
            settings.render_distance,
        ),
        player_position: Vec4::new(
            rig.position.x,
            rig.ground_influence_height(),
            rig.position.z,
            0.0,
        ),
        world_seed: crate::world::goblin_dens::shader_seed(view.position),
        tracks,
        tracks_meta,
        tracks_bounds,
        track_bins: bins.1,
        shadow_quality: UVec4::splat(match settings.shadow_quality {
            super::graphics::Quality::Low => 12,
            super::graphics::Quality::Medium => 24,
            super::graphics::Quality::High => 48,
        }),
        orc_holes,
        goblin_portals,
        goblin_limits: crate::world::goblin_dens::far_limits(
            view.position,
            settings.render_distance,
        ),
        cutouts: CutoutBins::build(
            view.position,
            settings.render_distance,
            &orc_holes,
            &goblin_portals,
        ),
    };
    // Assets::get_mut marks a material modified even if values are unchanged.
    // Keep idle/paused frames from re-extracting and uploading identical data.
    // All three draw paths share the same climate, cave masks and tracks.
    // Terrain, rocks and fullscreen geometry share one surface/lighting pipeline.
    for handle in [&material_handle.0, &material_handle.1, &material_handle.2] {
        if materials
            .get(handle)
            .is_some_and(|m| m.uniforms != uniforms)
            && let Some(mut material) = materials.get_mut(handle)
        {
            material.uniforms = uniforms;
        }
    }
}

#[cfg(test)]
mod cutout_tests {
    use super::*;
    #[test]
    fn bins_keep_overlapping_portals_and_holes_on_negative_cell_edges() {
        let mut holes = [Vec4::ZERO; 32];
        let mut portals = [Vec4::ZERO; 256];
        holes[0] = Vec4::new(-17., 33., 9., -14.);
        portals[0] = Vec4::new(-80., 10., -50., 3.5);
        portals[1] = Vec4::new(90., 25., 80., 712.);
        let bins = CutoutBins::build(Vec3::ZERO, 260., &holes, &portals);
        let index = |p: Vec2| {
            let q = ((p - bins.bounds.xy()) * bins.bounds.w).floor().as_ivec2();
            (q.y * 16 + q.x) as usize
        };
        for i in 0..101 {
            let t = i as f32 / 100.;
            for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                let p = portals[0].xz().lerp(portals[1].xz(), t) + d * 15.0_f32.sqrt();
                assert_ne!(bins.goblins[index(p)].x & 1, 0);
            }
        }
        for x in [-1., 1.] {
            for z in [-1., 1.] {
                let c = index(holes[0].xy() + holes[0].zw().abs() * Vec2::new(x, z));
                assert_ne!(bins.orcs[c / 4][c % 4] & 1, 0);
            }
        }
        assert_eq!(bins.goblins[index(Vec2::splat(220.))], UVec4::ZERO);
    }
}
