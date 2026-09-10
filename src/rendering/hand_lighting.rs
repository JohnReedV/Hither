//! The hand pass has its own projection/depth, but the same world pose and XY
//! projection as the world pass. Reuse the complete world lighting bindings:
//! cascade matrices alone are insufficient because cluster indices, probe
//! indices and their uniforms must agree as well.
use bevy::{
    core_pipeline::prepass::{AlphaMask3dPrepass, Opaque3dPrepass},
    pbr::{MeshViewBindGroup, prepare_mesh_view_bind_groups},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_phase::ViewBinnedRenderPhases,
        view::{ExtractedView, ViewUniformOffset},
    },
};

#[derive(Component, Clone, Copy, ExtractComponent)]
pub(crate) enum WorldLighting {
    Source,
    Hands,
}

pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(ExtractComponentPlugin::<WorldLighting>::default());
    app.add_systems(
        PostUpdate,
        prune_unlit_sun_cascades
            .after(bevy::light::SimulationLightSystems::UpdateDirectionalLightCascades)
            .before(bevy::light::SimulationLightSystems::UpdateLightFrusta),
    );
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(Render, skip_hand_prepass.in_set(RenderSystems::CreateViews));
        render_app.add_systems(
            Render,
            share
                .in_set(RenderSystems::PrepareBindGroups)
                .after(prepare_mesh_view_bind_groups),
        );
    }
}

// Bevy 0.19 builds CPU cascades for every active camera, even cameras outside
// the light's layers and 2D views. Match its GPU admission policy before frusta
// and visible-mesh lists are generated, avoiding the duplicate CPU culling too.
fn prune_unlit_sun_cascades(
    views: Query<Option<&bevy::camera::visibility::RenderLayers>, With<Camera3d>>,
    mut suns: Query<
        (
            &bevy::camera::visibility::RenderLayers,
            &mut bevy::light::Cascades,
        ),
        With<super::lighting::Sun>,
    >,
) {
    for (layers, mut cascades) in &mut suns {
        cascades.cascades.retain(|entity, _| {
            views
                .get(*entity)
                .is_ok_and(|view_layers| layers.intersects(view_layers.unwrap_or_default()))
        });
    }
}

// Keep the depth-capable view layout for shared bindings, but queue and render
// no hand prepass. Bevy's opaque forward pass writes its own independent depth.
// Core3d's prepass node returns immediately when these phases are absent.
fn skip_hand_prepass(
    views: Query<(&WorldLighting, &ExtractedView)>,
    mut opaque: ResMut<ViewBinnedRenderPhases<Opaque3dPrepass>>,
    mut masked: ResMut<ViewBinnedRenderPhases<AlphaMask3dPrepass>>,
) {
    for (role, view) in &views {
        if matches!(role, WorldLighting::Hands) {
            opaque.remove(&view.retained_view_entity);
            masked.remove(&view.retained_view_entity);
        }
    }
}

fn share(
    mut commands: Commands,
    views: Query<(
        Entity,
        &WorldLighting,
        &MeshViewBindGroup,
        &ViewUniformOffset,
    )>,
) {
    let Some((_, _, source, _)) = views
        .iter()
        .find(|(_, role, _, _)| matches!(role, WorldLighting::Source))
    else {
        return;
    };
    for (entity, role, _, view_offset) in &views {
        if !matches!(role, WorldLighting::Hands) {
            continue;
        }
        let mut offsets = source.main_offsets.clone();
        // Binding zero selects the hand projection. All lighting offsets and
        // cluster buffers stay paired with the world camera that generated them.
        offsets[0] = view_offset.offset;
        commands.entity(entity).insert(MeshViewBindGroup {
            main: source.main.clone(),
            main_offsets: offsets,
            binding_array: source.binding_array.clone(),
            empty: source.empty.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::{
        batching::gpu_preprocessing::GpuPreprocessingMode, view::RetainedViewEntity,
    };
    #[test]
    fn cpu_sun_culling_only_keeps_lit_3d_views() {
        use bevy::camera::visibility::RenderLayers;
        let mut app = App::new();
        app.add_systems(Update, prune_unlit_sun_cascades);
        let source = app.world_mut().spawn(Camera3d::default()).id();
        let hands = app
            .world_mut()
            .spawn((Camera3d::default(), RenderLayers::layer(2)))
            .id();
        let ui = app.world_mut().spawn(Camera2d).id();
        let mut cascades = bevy::light::Cascades::default();
        for camera in [source, hands, ui] {
            cascades.cascades.insert(camera, Vec::new());
        }
        let sun = app
            .world_mut()
            .spawn((
                super::super::lighting::Sun,
                cascades,
                RenderLayers::from_layers(&[0, 1]),
            ))
            .id();
        app.update();
        let kept = &app
            .world()
            .get::<bevy::light::Cascades>(sun)
            .unwrap()
            .cascades;
        assert_eq!(kept.len(), 1);
        assert!(kept.contains_key(&source));
    }
    #[test]
    fn hands_never_queue_a_prepass_and_world_prepasses_are_preserved() {
        let mut app = App::new();
        app.init_resource::<ViewBinnedRenderPhases<Opaque3dPrepass>>()
            .init_resource::<ViewBinnedRenderPhases<AlphaMask3dPrepass>>()
            .add_systems(Update, skip_hand_prepass);
        let mut keys = Vec::new();
        for role in [WorldLighting::Source, WorldLighting::Hands] {
            let entity = app.world_mut().spawn_empty().id();
            let key = RetainedViewEntity::new(entity.into(), None, 0);
            app.world_mut().entity_mut(entity).insert((
                role,
                ExtractedView {
                    retained_view_entity: key,
                    clip_from_view: Mat4::IDENTITY,
                    world_from_view: GlobalTransform::IDENTITY,
                    clip_from_world: None,
                    target_format: bevy::render::render_resource::TextureFormat::Rgba16Float,
                    viewport: UVec4::new(0, 0, 1280, 720),
                    color_grading: default(),
                    invert_culling: false,
                },
            ));
            keys.push(key);
        }
        // Extraction recreates these entries each frame, including after mode switches.
        for _ in 0..3 {
            for key in &keys {
                app.world_mut()
                    .resource_mut::<ViewBinnedRenderPhases<Opaque3dPrepass>>()
                    .prepare_for_new_frame(*key, GpuPreprocessingMode::None);
                app.world_mut()
                    .resource_mut::<ViewBinnedRenderPhases<AlphaMask3dPrepass>>()
                    .prepare_for_new_frame(*key, GpuPreprocessingMode::None);
            }
            app.update();
            let opaque = app
                .world()
                .resource::<ViewBinnedRenderPhases<Opaque3dPrepass>>();
            let masked = app
                .world()
                .resource::<ViewBinnedRenderPhases<AlphaMask3dPrepass>>();
            assert!(opaque.contains_key(&keys[0]) && masked.contains_key(&keys[0]));
            assert!(!opaque.contains_key(&keys[1]) && !masked.contains_key(&keys[1]));
        }
    }
}
