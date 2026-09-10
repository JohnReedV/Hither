//! Remove cached static casters from per-frame GPU preprocessing. Bevy retains
//! dynamic bins; suspended static entries are explicitly requeued on a miss.
use super::{Cache, Inputs, Plan};
use bevy::{
    camera::visibility::RenderLayers,
    pbr::{
        LightEntity, PreparedMaterial, RenderMaterialInstances, RenderMeshInstanceFlags,
        RenderMeshInstances, Shadow, ShadowBatchSetKey, ShadowBinKey, ShadowsDepthOnlyDrawFunction,
        SpecializedShadowMaterialPipelineCache,
    },
    prelude::*,
    render::{
        batching::gpu_preprocessing::GpuPreprocessingSupport,
        erased_render_asset::ErasedRenderAssets,
        mesh::allocator::MeshAllocator,
        occlusion_culling::OcclusionCulling,
        render_phase::{BinnedRenderPhaseType, ViewBinnedRenderPhases},
        sync_world::MainEntity,
        view::{ExtractedView, RetainedViewEntity},
    },
};
use std::collections::{HashMap, HashSet};
#[derive(Resource, Default)]
pub(super) struct Suspended(HashMap<RetainedViewEntity, HashSet<Entity>>);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn queue(
    inputs: Res<Inputs>,
    mut suspend: Local<Option<bool>>,
    cache: Res<Cache>,
    mut suspended: ResMut<Suspended>,
    mut phases: ResMut<ViewBinnedRenderPhases<Shadow>>,
    specialized: Res<SpecializedShadowMaterialPipelineCache>,
    instances: Res<RenderMeshInstances>,
    material_instances: Res<RenderMaterialInstances>,
    materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    allocator: Res<MeshAllocator>,
    gpu: Res<GpuPreprocessingSupport>,
    mut views: Query<(
        &LightEntity,
        &ExtractedView,
        &RenderLayers,
        Has<OcclusionCulling>,
        Option<&mut Plan>,
    )>,
) {
    let suspend = *suspend.get_or_insert_with(|| {
        !(std::env::var_os("HITHER_PROFILE_FOREST").is_some()
            && std::env::var_os("HITHER_PROFILE_POINT_CACHE_DRAW_ONLY").is_some())
    });
    let mut live = HashSet::new();
    for (light, view, layers, occlusion, mut plan) in &mut views {
        let LightEntity::Point { face_index, .. } = light else {
            continue;
        };
        let retained = view.retained_view_entity;
        live.insert(retained);
        let key = (retained.main_entity.id(), *face_index);
        let Some(phase) = phases.get_mut(&retained) else {
            continue;
        };
        // Custom non-mesh draw commands have no main-world caster contract.
        if !phase.non_mesh_items.is_empty()
            && let Some(plan) = plan.as_mut()
        {
            plan.signature = None;
        }
        let pipelines = specialized.get(&retained);
        let reuse = !occlusion
            && plan.as_ref().is_some_and(|plan| {
                plan.signature.as_ref().is_some_and(|signature| {
                    cache
                        .0
                        .lock()
                        .unwrap()
                        .get(&plan.slot)
                        .is_some_and(|entry| {
                            entry.signature == *signature
                                && if plan.dynamic {
                                    entry.image.is_some()
                                } else {
                                    entry.output_static
                                }
                        })
                })
            });
        if let Some(plan) = plan.as_mut() {
            plan.reuse = reuse;
        }
        if !suspend {
            if let Some(plan) = plan.as_mut() {
                plan.restored = true;
            }
            continue;
        }
        let hidden = suspended.0.entry(retained).or_default();
        if reuse {
            let signature = plan.as_ref().unwrap().signature.as_ref().unwrap();
            for caster in &signature.input.casters {
                let main = MainEntity::from(*caster);
                if pipelines
                    .and_then(|p| p.get(&main))
                    .is_some_and(|(pipeline, _)| signature.pipelines.contains(pipeline))
                {
                    // Also remove entries that Bevy may have queued again due
                    // to unrelated material bind-group relocation this frame.
                    phase.remove(main);
                    hidden.insert(*caster);
                }
            }
            continue;
        }
        // A removed opt-in marker, a resident entering a shared pipeline, a
        // changed asset/light, and an occlusion-mode switch all restore bins.
        let visible: HashSet<_> = inputs
            .0
            .get(&key)
            .into_iter()
            .flat_map(|f| f.casters.iter())
            .chain(inputs.1.get(&key).into_iter().flatten())
            .copied()
            .collect();
        hidden.retain(|caster| {
            if !visible.contains(caster) {
                return false;
            }
            let main = MainEntity::from(*caster);
            let Some(render) = inputs.2.get(caster).copied() else {
                return true;
            };
            let Some(&(pipeline, draw_function)) = pipelines.and_then(|p| p.get(&main)) else {
                return true;
            };
            let Some(mesh) = instances.render_mesh_queue_data(main) else {
                return true;
            };
            if !mesh
                .flags()
                .contains(RenderMeshInstanceFlags::SHADOW_CASTER)
                || !layers.intersects(mesh.render_layers.as_ref().unwrap_or_default())
            {
                return false;
            }
            let Some(instance) = material_instances.instances.get(&main) else {
                return true;
            };
            let Some(material) = materials.get(instance.asset_id) else {
                return true;
            };
            let Some(slabs) = allocator.mesh_slabs(&mesh.mesh_asset_id()) else {
                return true;
            };
            let depth_only = material
                .properties
                .get_draw_function(ShadowsDepthOnlyDrawFunction);
            let material_bind_group_index = if Some(draw_function) == depth_only {
                None
            } else {
                Some(material.binding.group.0)
            };
            phase.remove(main);
            phase.add(
                ShadowBatchSetKey {
                    pipeline,
                    draw_function,
                    material_bind_group_index,
                    slabs,
                },
                ShadowBinKey {
                    asset_id: mesh.mesh_asset_id().into(),
                },
                (render, main),
                mesh.current_uniform_index,
                BinnedRenderPhaseType::mesh(mesh.should_batch(), &gpu),
            );
            false
        });
        if let Some(plan) = plan.as_mut() {
            plan.restored = hidden.is_empty();
        }
    }
    suspended.0.retain(|key, _| live.contains(key));
}
