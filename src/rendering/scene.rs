use crate::app::settings::GraphicsSettings;
use crate::player::avatar;
use crate::player::camera::CameraRig;
use crate::rendering::sdf::{SdfCanvas, SdfMaterial, SdfMaterialHandle, SdfUniforms};
use crate::rendering::view_distance;
use crate::world::{biome, snow, terrain};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

pub(crate) fn setup_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SdfMaterial>,
    window: &Window,
    rig: &CameraRig,
    settings: &GraphicsSettings,
    device: &bevy::render::renderer::RenderDevice,
) {
    let view = rig.view();
    let mut camera = commands.spawn((
        Camera3d::default(),
        bevy::camera::Hdr,
        super::graphics::WorldPass,
        Camera {
            order: -3,
            output_mode: bevy::camera::CameraOutputMode::Skip,
            ..default()
        },
        Msaa::Off,
        bevy::core_pipeline::prepass::DepthPrepass,
        Projection::custom(view_distance::FinitePerspective::new(
            settings.render_distance,
        )),
        Tonemapping::None,
        Transform::from_translation(view.position).looking_to(view.forward, view.up),
        avatar::PlayerCamera,
        super::hand_lighting::WorldLighting::Source,
    ));
    if super::occlusion::mode().gpu() {
        camera.insert(bevy::render::occlusion_culling::OcclusionCulling);
    }
    let size = Vec2::new(window.width(), window.height());
    let contacts = crate::world::geology::contact::create(commands, device);
    let (surface_pixels, surface_pages, rock_surfaces, terrain_samples) =
        super::surface_cache::create(commands, device);
    let material = materials.add(SdfMaterial {
        surface_pixels,
        surface_pages,
        rock_surfaces,
        terrain_samples,
        contacts,
        terrain: false,
        rock: false,
        uniforms: SdfUniforms {
            cutouts: default(),
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
                size.x,
                size.y,
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
            tracks: [Vec4::ZERO; snow::MAX_TRACKS],
            tracks_meta: Vec4::ZERO,
            tracks_bounds: Vec4::ZERO,
            track_bins: snow::TrackBins::default(),
            shadow_quality: UVec4::splat(48),
            orc_holes: crate::world::orcs::hole_uniforms(view.position, settings.render_distance),
            goblin_limits: [Vec4::ZERO; 128],
            goblin_portals: crate::world::goblin_dens::portal_uniforms(
                view.position,
                settings.render_distance,
            ),
        },
    });

    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(2.0, 2.0))),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
        SdfCanvas,
        bevy::camera::primitives::Aabb::default(),
    ));
    let mut terrain_material = materials.get(&material).unwrap().clone();
    terrain_material.terrain = true;
    let terrain_handle = materials.add(terrain_material);
    terrain::spawn(
        commands,
        meshes,
        terrain_handle.clone(),
        settings.render_distance,
    );
    let mut rock_material = materials.get(&material).unwrap().clone();
    rock_material.rock = true;
    let rock_handle = materials.add(rock_material);
    commands.insert_resource(SdfMaterialHandle(material, terrain_handle, rock_handle));
}
