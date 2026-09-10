//! Cache immutable depth separately from live point-shadow casters.
//! Mixed pipeline batches remain live; unknown casters are never frozen.
#[path = "point_shadow_cache_depth.rs"]
mod depth;
#[path = "point_shadow_cache_queue.rs"]
mod queue;
use bevy::{
    asset::VisitAssetDependencies,
    camera::visibility::CubemapVisibleEntities,
    core_pipeline::{
        mip_generation::experimental::depth::early_downsample_depth, schedule::Core3d,
    },
    ecs::schedule::ScheduleCleanupPolicy,
    pbr::{
        EARLY_SHADOW_PASS, LATE_SHADOW_PASS, LightEntity, Shadow, ShadowView,
        SpecializedShadowMaterialPipelineCache, ViewShadowBindings,
        early_prepass_build_indirect_parameters, late_prepass_build_indirect_parameters,
        main_build_indirect_parameters, shared_shadow_pass,
    },
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_phase::{Draw, DrawError, DrawFunctions, TrackedRenderPass, ViewBinnedRenderPhases},
        render_resource::{
            CachedRenderPipelineId, LoadOp, Operations, PipelineCache,
            RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp, TextureId,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::{ExtractedView, RetainedViewEntity},
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

/// Opt in only immutable, opaque StandardMaterial geometry. Moving transforms,
/// material/mesh edits and visibility changes still invalidate the cached face.
#[derive(Component)]
pub(crate) struct StaticPointShadowCaster;

#[derive(Clone, PartialEq, Eq, Debug)]
struct FaceInput {
    epoch: u64,
    light: [u32; 3],
    casters: Vec<Entity>,
}
#[derive(Resource, Default)]
struct Inputs(
    HashMap<(Entity, usize), FaceInput>,
    HashMap<(Entity, usize), Vec<Entity>>,
    HashMap<Entity, Entity>,
);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Slot(TextureId, String);
#[derive(Clone, PartialEq, Eq, Debug)]
struct Signature {
    owner: RetainedViewEntity,
    input: FaceInput,
    matrices: [[u32; 16]; 2],
    pipelines: HashSet<CachedRenderPipelineId>,
}
#[derive(Resource)]
struct Cache(Mutex<HashMap<Slot, Entry>>, std::sync::atomic::AtomicBool);
impl Default for Cache {
    fn default() -> Self {
        Self(
            Mutex::default(),
            std::sync::atomic::AtomicBool::new(
                std::env::var_os("HITHER_VALIDATE_POINT_CACHE").is_some(),
            ),
        )
    }
}
struct Entry {
    signature: Signature,
    image: Option<depth::DepthImage>,
    output_static: bool,
}

#[derive(Component)]
struct Plan {
    slot: Slot,
    signature: Option<Signature>,
    size: u32,
    target: depth::Target,
    dynamic: bool,
    reuse: bool,
    restored: bool,
    mode: std::sync::atomic::AtomicU8,
}

pub(crate) fn plugin(app: &mut App) {
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render
        .init_resource::<Inputs>()
        .init_resource::<Cache>()
        .init_resource::<queue::Suspended>()
        .add_systems(ExtractSchedule, extract_inputs)
        .add_systems(
            Render,
            (depth::prepare, prepare)
                .chain()
                .in_set(RenderSystems::QueueMeshes)
                .before(bevy::pbr::queue_shadows),
        )
        .add_systems(
            Render,
            queue::queue
                .in_set(RenderSystems::QueueMeshes)
                .after(bevy::pbr::queue_shadows),
        );
    // Replace only the shared point/spot draw wrappers. Preserve the standard
    // early/late ordering and leave directional shadows entirely untouched.
    render
        .world_mut()
        .schedule_scope(Core3d, |world, schedule| {
            for removed in [
                schedule.remove_systems_in_set(
                    shared_shadow_pass::<EARLY_SHADOW_PASS>,
                    world,
                    ScheduleCleanupPolicy::RemoveSystemsOnly,
                ),
                schedule.remove_systems_in_set(
                    shared_shadow_pass::<LATE_SHADOW_PASS>,
                    world,
                    ScheduleCleanupPolicy::RemoveSystemsOnly,
                ),
            ] {
                assert_eq!(removed.expect("PBR shared shadow pass"), 1);
            }
        });
    render.add_systems(
        Core3d,
        (
            draw::<EARLY_SHADOW_PASS>
                .after(early_prepass_build_indirect_parameters)
                .before(early_downsample_depth)
                .before(draw::<LATE_SHADOW_PASS>),
            draw::<LATE_SHADOW_PASS>
                .after(late_prepass_build_indirect_parameters)
                .before(main_build_indirect_parameters)
                .before(bevy::core_pipeline::schedule::Core3dSystems::MainPass),
        ),
    );
}

fn event_id<A: Asset>(event: &AssetEvent<A>) -> AssetId<A> {
    match *event {
        AssetEvent::Added { id }
        | AssetEvent::Modified { id }
        | AssetEvent::Removed { id }
        | AssetEvent::Unused { id }
        | AssetEvent::LoadedWithDependencies { id } => id,
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn extract_inputs(
    mut inputs: ResMut<Inputs>,
    mut epoch: Local<u64>,
    mut revisions: Local<HashMap<Entity, u64>>,
    lights: Extract<Query<(Entity, &PointLight, &CubemapVisibleEntities)>>,
    casters: Extract<
        Query<
            (
                Entity,
                &Mesh3d,
                &MeshMaterial3d<StandardMaterial>,
                Ref<GlobalTransform>,
            ),
            With<StaticPointShadowCaster>,
        >,
    >,
    changed: Extract<
        Query<
            (),
            (
                With<StaticPointShadowCaster>,
                Or<(
                    Changed<Mesh3d>,
                    Changed<MeshMaterial3d<StandardMaterial>>,
                    Changed<StaticPointShadowCaster>,
                )>,
            ),
        >,
    >,
    render_entities: Extract<Query<&bevy::render::sync_world::RenderEntity>>,
    materials: Extract<Res<Assets<StandardMaterial>>>,
    mut mesh_events: Extract<MessageReader<AssetEvent<Mesh>>>,
    mut material_events: Extract<MessageReader<AssetEvent<StandardMaterial>>>,
    mut image_events: Extract<MessageReader<AssetEvent<Image>>>,
    mut shader_events: Extract<MessageReader<AssetEvent<Shader>>>,
) {
    // Revisions belong to dependencies, not the asset server as a whole.
    // Streaming an unrelated grass mesh must not evict every room shadow.
    let mesh_changes: HashSet<_> = mesh_events.read().map(event_id).collect();
    let material_changes: HashSet<_> = material_events.read().map(event_id).collect();
    let image_changes: HashSet<_> = image_events
        .read()
        .map(|event| event_id(event).untyped())
        .collect();
    // Shader imports can affect arbitrary specialized pipelines; keep this rare
    // development-time event conservative until the import graph is exposed.
    let shaders_changed = shader_events.read().count() > 0;
    *epoch = epoch.checked_add(1).expect("shadow revision exhausted");
    revisions.retain(|entity, _| casters.contains(*entity));
    for (entity, mesh, material, transform) in &casters {
        let mut dependency_changed = false;
        if let Some(material) = materials.get(material) {
            material.visit_dependencies(&mut |id| {
                dependency_changed |= image_changes.contains(&id);
            });
        }
        if shaders_changed
            || changed.contains(entity)
            || transform.is_changed()
            || mesh_changes.contains(&mesh.id())
            || material_changes.contains(&material.id())
            || dependency_changed
        {
            revisions.insert(entity, *epoch);
        }
    }
    inputs.0.clear();
    inputs.1.clear();
    inputs.2.clear();
    for (entity, light, faces) in &lights {
        if !light.shadow_maps_enabled {
            continue;
        }
        for (index, face) in faces.iter().enumerate() {
            let mut entities = Vec::with_capacity(face.entities.len());
            let mut dynamic = Vec::new();
            let mut face_revision = 0;
            for &caster in &face.entities {
                if let Ok(render) = render_entities.get(caster) {
                    inputs.2.insert(caster, render.id());
                }
                let Ok((_, _, material, _)) = casters.get(caster) else {
                    dynamic.push(caster);
                    continue;
                };
                if !materials
                    .get(material)
                    .is_some_and(|m| m.alpha_mode == AlphaMode::Opaque)
                {
                    dynamic.push(caster);
                    continue;
                }
                face_revision = face_revision.max(revisions.get(&caster).copied().unwrap_or(0));
                entities.push(caster);
            }
            {
                inputs.1.insert((entity, index), dynamic);
                entities.sort_unstable();
                inputs.0.insert(
                    (entity, index),
                    FaceInput {
                        epoch: face_revision,
                        light: [
                            light.shadow_depth_bias.to_bits(),
                            light.shadow_normal_bias.to_bits(),
                            light.shadow_map_near_z.to_bits(),
                        ],
                        casters: entities,
                    },
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    functions: Res<DrawFunctions<Shadow>>,
    mut wrapped: Local<usize>,
    mut seen: Local<HashMap<Slot, (Signature, u8)>>,
    specialized: Res<SpecializedShadowMaterialPipelineCache>,
    inputs: Res<Inputs>,
    cache: Res<Cache>,
    views: Query<(
        Entity,
        &LightEntity,
        &ExtractedView,
        &ShadowView,
        Option<&depth::Target>,
    )>,
    bindings: Query<&ViewShadowBindings>,
) {
    // Wrap existing IDs in place so Bevy's incremental bins remain valid.
    let mut draws = functions.write();
    for index in *wrapped..draws.draw_functions.len() {
        let original = std::mem::replace(&mut draws.draw_functions[index], Box::new(EmptyDraw));
        draws.draw_functions[index] = Box::new(FilteredDraw(original));
    }
    *wrapped = draws.draw_functions.len();
    drop(draws);
    let mut live = HashSet::new();
    // Bevy shares this texture across cameras. Refuse to cache if this invariant
    // ever changes rather than guessing which camera owns a shadow face.
    let size = bindings
        .iter()
        .next()
        .map_or(0, |b| b.point_light_depth_texture.width());
    let mut textures = bindings.iter().map(|b| b.point_light_depth_texture.id());
    let texture = textures
        .next()
        .filter(|id| textures.all(|other| other == *id));
    // Queue-time reuse decisions must remain valid until drawing. Multiple
    // cameras can overwrite the same physical slot within one frame, so those
    // slots retain their full native bins and draw normally.
    let mut owners = HashMap::<&str, usize>::new();
    for (_, light, _, shadow, _) in &views {
        if matches!(light, LightEntity::Point { .. }) {
            *owners.entry(shadow.pass_name.as_str()).or_default() += 1;
        }
    }
    for (entity, light, view, shadow, target) in &views {
        let LightEntity::Point { face_index, .. } = light else {
            continue;
        };
        let Some(target) = target else {
            commands.entity(entity).remove::<Plan>();
            continue;
        };
        let Some(texture) = texture else {
            commands.entity(entity).remove::<Plan>();
            continue;
        };
        // pass_name includes the physical array index and cubemap face. A slot
        // reassigned to another light must overwrite its previous owner entry.
        let slot = Slot(texture, shadow.pass_name.clone());
        live.insert(slot.clone());
        let key = (view.retained_view_entity.main_entity.id(), *face_index);
        let dynamic = inputs.1.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        let view_pipelines = specialized.get(&view.retained_view_entity);
        let pipeline_for = |entity: &Entity| {
            view_pipelines
                .and_then(|map| map.get(&bevy::render::sync_world::MainEntity::from(*entity)))
                .map(|(p, _)| *p)
        };
        let live_pipelines: HashSet<_> = dynamic.iter().filter_map(pipeline_for).collect();
        let signature = inputs
            .0
            .get(&(view.retained_view_entity.main_entity.id(), *face_index))
            .map(|input| Signature {
                pipelines: input
                    .casters
                    .iter()
                    .filter_map(pipeline_for)
                    .filter(|p| !live_pipelines.contains(p))
                    .collect(),
                owner: view.retained_view_entity,
                input: input.clone(),
                matrices: [
                    view.world_from_view
                        .to_matrix()
                        .to_cols_array()
                        .map(f32::to_bits),
                    view.clip_from_view.to_cols_array().map(f32::to_bits),
                ],
            });
        let signature = signature.filter(|s| {
            owners.get(shadow.pass_name.as_str()) == Some(&1)
                && (!s.pipelines.is_empty() || dynamic.is_empty())
        });
        if signature.is_none() {
            seen.remove(&slot);
        }
        // Moving lights and unstable streaming faces take one ordinary pass.
        // Do not pay for a static bake/copy that would be invalid next frame.
        let signature = signature.filter(|signature| {
            let previous = seen.entry(slot.clone()).or_insert((signature.clone(), 0));
            if previous.0 == *signature {
                previous.1 = previous.1.saturating_add(1).min(3);
            } else {
                *previous = (signature.clone(), 1);
            }
            previous.1 >= 3
        });
        commands.entity(entity).insert(Plan {
            slot,
            signature,
            size,
            target: target.clone(),
            dynamic: !dynamic.is_empty(),
            reuse: false,
            restored: false,
            mode: std::sync::atomic::AtomicU8::new(0),
        });
    }
    seen.retain(|slot, _| live.contains(slot));
    cache
        .0
        .lock()
        .unwrap()
        .retain(|slot, _| live.contains(slot));
}

type ShadowDrawView = (
    Entity,
    &'static ShadowView,
    &'static ExtractedView,
    Has<bevy::render::occlusion_culling::OcclusionCulling>,
    Option<&'static Plan>,
);

#[allow(clippy::type_complexity)] // One Bevy shadow-view query, matching the native draw wrapper.
fn draw<const LATE: bool>(world: &World, view: ViewQuery<ShadowDrawView>, mut ctx: RenderContext) {
    let (entity, shadow, view, occlusion, plan) = view.into_inner();
    if LATE && !occlusion {
        return;
    }
    let cache = world.resource::<Cache>();
    // Occlusion-culling views need their complete early/late depth lifecycle.
    let signature = plan
        .filter(|_| !occlusion)
        .and_then(|p| p.signature.as_ref());
    let phases = world.resource::<ViewBinnedRenderPhases<Shadow>>();
    let Some(phase) = phases.get(&view.retained_view_entity) else {
        return;
    };
    // A pending pipeline can skip a draw without reporting a render error.
    // Never retain such an incomplete map. All opted-in meshes must also have
    // reached the render world and shadow specialization cache.
    let pipelines = world.resource::<PipelineCache>();
    let specialized = world.resource::<SpecializedShadowMaterialPipelineCache>();
    let instances = world.resource::<bevy::pbr::RenderMeshInstances>();
    let meshes = world.resource::<RenderAssets<RenderMesh>>();
    let material_instances = world.resource::<bevy::pbr::RenderMaterialInstances>();
    let materials = world.resource::<bevy::render::erased_render_asset::ErasedRenderAssets<bevy::pbr::PreparedMaterial>>();
    let pending = world.resource::<bevy::pbr::PendingShadowQueues>();
    let queues_ready = pending
        .get(&view.retained_view_entity)
        .is_some_and(|q| q.current_frame.is_empty() && q.prev_frame.is_empty());
    let ready = plan.is_some_and(|p| p.reuse)
        || plan.is_some_and(|p| p.restored)
            && queues_ready
            // Initial mesh preparation can silently skip while its compute
            // pipelines compile. Only misses wait; unrelated compilation must
            // never discard an already-complete cached face.
            && pipelines.pipelines().all(|p| {
                matches!(p.state, bevy::render::render_resource::CachedPipelineState::Ok(_))
            })
            && signature.is_some_and(|s| {
                s.input.casters.iter().all(|caster| {
                    let main = bevy::render::sync_world::MainEntity::from(*caster);
                    specialized
                        .get(&view.retained_view_entity)
                        .and_then(|c| c.get(&main))
                        .is_some_and(|(pipeline, _)| {
                            pipelines.get_render_pipeline(*pipeline).is_some()
                        })
                        && material_instances
                            .instances
                            .get(&main)
                            .is_some_and(|i| materials.get(i.asset_id).is_some())
                        && instances
                            .render_mesh_queue_data(main)
                            .is_some_and(|i| meshes.get(i.mesh_asset_id()).is_some())
                })
            });
    let Some((plan, signature)) = plan.zip(signature).filter(|_| ready) else {
        if let Some(plan) = plan {
            cache.0.lock().unwrap().remove(&plan.slot);
        }
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some(&shadow.pass_name),
            color_attachments: &[],
            depth_stencil_attachment: Some(shadow.depth_attachment.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Err(error) = phase.render(&mut pass, world, entity) {
            error!("Point shadow draw failed: {error:?}");
        }
        return;
    };
    // Fully static faces reuse Bevy's target directly, without allocating or
    // restoring a second texture. Only mixed faces need separate static depth.
    if !plan.dynamic {
        let mut entries = cache.0.lock().unwrap();
        if entries
            .get(&plan.slot)
            .is_some_and(|entry| entry.signature == *signature && entry.output_static)
        {
            return;
        }
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some(&shadow.pass_name),
            color_attachments: &[],
            depth_stencil_attachment: Some(shadow.depth_attachment.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let result = phase.render(&mut pass, world, entity);
        drop(pass);
        match result {
            Ok(()) => {
                entries.insert(
                    plan.slot.clone(),
                    Entry {
                        signature: signature.clone(),
                        image: None,
                        output_static: true,
                    },
                );
            }
            Err(error) => {
                entries.remove(&plan.slot);
                error!("Point shadow draw failed: {error:?}");
            }
        }
        return;
    }
    let mut entries = cache.0.lock().unwrap();
    let hit = entries
        .get(&plan.slot)
        .is_some_and(|entry| entry.signature == *signature && entry.image.is_some());
    if !hit {
        let image = depth::DepthImage::new(world.resource::<RenderDevice>(), plan.size);
        plan.mode.store(1, std::sync::atomic::Ordering::Relaxed);
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("Bake static point-shadow depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &image.view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(0.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let result = phase.render(&mut pass, world, entity);
        drop(pass);
        plan.mode.store(0, std::sync::atomic::Ordering::Relaxed);
        if let Err(error) = result {
            entries.remove(&plan.slot);
            error!("Static point shadow draw failed: {error:?}");
            return;
        }
        entries.insert(
            plan.slot.clone(),
            Entry {
                signature: signature.clone(),
                image: Some(image),
                output_static: false,
            },
        );
    }
    let entry = entries.get_mut(&plan.slot).unwrap();
    entry
        .image
        .as_ref()
        .unwrap()
        .restore(ctx.command_encoder(), &plan.target);
    // The copy initialized the entire face. Consume the attachment's first-use
    // clear flag, then load that depth before rendering the live casters.
    let mut attachment = shadow.depth_attachment.get_attachment(StoreOp::Store);
    attachment.depth_ops.as_mut().unwrap().load = LoadOp::Load;
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("Restore static depth and draw live point shadows"),
        color_attachments: &[],
        depth_stencil_attachment: Some(attachment),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    plan.mode.store(2, std::sync::atomic::Ordering::Relaxed);
    let result = phase.render(&mut pass, world, entity);
    drop(pass);
    plan.mode.store(0, std::sync::atomic::Ordering::Relaxed);
    entry.output_static = !plan.dynamic;
    if let Err(error) = result {
        entries.remove(&plan.slot);
        error!("Live point shadow draw failed: {error:?}");
    } else if hit && plan.dynamic && cache.1.swap(false, std::sync::atomic::Ordering::Relaxed) {
        info!("Point shadow cache validation: restored static depth on a live resident face");
    }
}

struct EmptyDraw;
impl Draw<Shadow> for EmptyDraw {
    fn draw<'w>(
        &mut self,
        _: &'w World,
        _: &mut TrackedRenderPass<'w>,
        _: Entity,
        _: &Shadow,
    ) -> Result<(), DrawError> {
        Ok(())
    }
}
struct FilteredDraw(Box<dyn Draw<Shadow>>);
impl Draw<Shadow> for FilteredDraw {
    fn prepare(&mut self, world: &World) {
        self.0.prepare(world);
    }
    fn draw<'w>(
        &mut self,
        world: &'w World,
        pass: &mut TrackedRenderPass<'w>,
        view: Entity,
        item: &Shadow,
    ) -> Result<(), DrawError> {
        if let Some(plan) = world.get::<Plan>(view) {
            let mode = plan.mode.load(std::sync::atomic::Ordering::Relaxed);
            if mode != 0 {
                let is_static = plan
                    .signature
                    .as_ref()
                    .is_some_and(|s| s.pipelines.contains(&item.batch_set_key.pipeline));
                if (mode == 1) != is_static {
                    return Ok(());
                }
            }
        }
        self.0.draw(world, pass, view, item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::message::Messages,
        render::{MainWorld, sync_world::MainEntity},
    };

    fn fixture() -> (World, Schedule, Entity, Entity) {
        let mut main = MainWorld::default();
        main.init_resource::<Assets<Mesh>>();
        main.init_resource::<Assets<StandardMaterial>>();
        main.init_resource::<Messages<AssetEvent<Mesh>>>();
        main.init_resource::<Messages<AssetEvent<StandardMaterial>>>();
        main.init_resource::<Messages<AssetEvent<Image>>>();
        main.init_resource::<Messages<AssetEvent<Shader>>>();
        let mesh = main.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
        let material = main
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let caster = main
            .spawn((
                StaticPointShadowCaster,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                GlobalTransform::default(),
            ))
            .id();
        let mut visible = CubemapVisibleEntities::default();
        visible.get_mut(0).entities.push(caster);
        let light = main
            .spawn((
                PointLight {
                    shadow_maps_enabled: true,
                    ..default()
                },
                visible,
            ))
            .id();
        let mut world = World::new();
        world.insert_resource(main);
        world.init_resource::<Inputs>();
        let mut schedule = Schedule::default();
        schedule.add_systems(extract_inputs);
        (world, schedule, light, caster)
    }
    fn next(world: &mut World, schedule: &mut Schedule) {
        world.resource_mut::<MainWorld>().increment_change_tick();
        schedule.run(world);
        world.resource_mut::<MainWorld>().clear_trackers();
    }
    #[test]
    fn static_face_stays_identical_until_geometry_or_assets_change() {
        let (mut world, mut schedule, light, caster) = fixture();
        next(&mut world, &mut schedule);
        let first = world.resource::<Inputs>().0[&(light, 0)].clone();
        next(&mut world, &mut schedule);
        assert_eq!(first, world.resource::<Inputs>().0[&(light, 0)]);
        world
            .resource_mut::<MainWorld>()
            .entity_mut(caster)
            .insert(GlobalTransform::from_translation(Vec3::X));
        next(&mut world, &mut schedule);
        let moved = world.resource::<Inputs>().0[&(light, 0)].clone();
        assert_ne!(first.epoch, moved.epoch);
        let id = world
            .resource::<MainWorld>()
            .get::<Mesh3d>(caster)
            .unwrap()
            .0
            .id();
        world
            .resource_mut::<MainWorld>()
            .resource_mut::<Messages<AssetEvent<Mesh>>>()
            .write(AssetEvent::Modified { id });
        next(&mut world, &mut schedule);
        assert_ne!(moved.epoch, world.resource::<Inputs>().0[&(light, 0)].epoch);
    }
    #[test]
    fn resident_entering_a_face_keeps_static_inputs_and_is_classified_live() {
        let (mut world, mut schedule, light, _) = fixture();
        next(&mut world, &mut schedule);
        assert!(world.resource::<Inputs>().0.contains_key(&(light, 0)));
        let resident = world.resource_mut::<MainWorld>().spawn_empty().id();
        world
            .resource_mut::<MainWorld>()
            .get_mut::<CubemapVisibleEntities>(light)
            .unwrap()
            .get_mut(0)
            .entities
            .push(resident);
        next(&mut world, &mut schedule);
        assert_eq!(world.resource::<Inputs>().0[&(light, 0)].casters.len(), 1);
        assert_eq!(world.resource::<Inputs>().1[&(light, 0)], vec![resident]);
        world
            .resource_mut::<MainWorld>()
            .get_mut::<CubemapVisibleEntities>(light)
            .unwrap()
            .get_mut(0)
            .entities
            .clear();
        next(&mut world, &mut schedule);
        assert!(world.resource::<Inputs>().0[&(light, 0)].casters.is_empty());
    }
    #[test]
    fn alpha_materials_and_missing_materials_never_cache() {
        let (mut world, mut schedule, light, caster) = fixture();
        let handle = world
            .resource::<MainWorld>()
            .get::<MeshMaterial3d<StandardMaterial>>(caster)
            .unwrap()
            .0
            .clone();
        world
            .resource_mut::<MainWorld>()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&handle)
            .unwrap()
            .alpha_mode = AlphaMode::Mask(0.5);
        next(&mut world, &mut schedule);
        assert!(world.resource::<Inputs>().0[&(light, 0)].casters.is_empty());
        world
            .resource_mut::<MainWorld>()
            .resource_mut::<Assets<StandardMaterial>>()
            .remove(handle.id());
        next(&mut world, &mut schedule);
        assert!(world.resource::<Inputs>().0[&(light, 0)].casters.is_empty());
    }
    #[test]
    fn unrelated_assets_and_other_faces_do_not_invalidate() {
        let (mut world, mut schedule, light, caster) = fixture();
        next(&mut world, &mut schedule);
        let first = world.resource::<Inputs>().0[&(light, 0)].clone();
        let mesh_id = world
            .resource_mut::<MainWorld>()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::default())
            .id();
        world
            .resource_mut::<MainWorld>()
            .resource_mut::<Messages<AssetEvent<Mesh>>>()
            .write(AssetEvent::Modified { id: mesh_id });
        let material_id = world
            .resource_mut::<MainWorld>()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default())
            .id();
        world
            .resource_mut::<MainWorld>()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .write(AssetEvent::Modified { id: material_id });
        next(&mut world, &mut schedule);
        assert_eq!(first, world.resource::<Inputs>().0[&(light, 0)]);
        let empty = world.resource::<Inputs>().0[&(light, 1)].clone();
        world
            .resource_mut::<MainWorld>()
            .entity_mut(caster)
            .insert(GlobalTransform::from_translation(Vec3::Y));
        next(&mut world, &mut schedule);
        assert_ne!(first, world.resource::<Inputs>().0[&(light, 0)]);
        assert_eq!(empty, world.resource::<Inputs>().0[&(light, 1)]);
    }
    #[test]
    fn material_image_dependencies_invalidate_only_their_faces() {
        let (mut world, mut schedule, light, caster) = fixture();
        world
            .resource_mut::<MainWorld>()
            .init_resource::<Assets<Image>>();
        let image = world
            .resource_mut::<MainWorld>()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let handle = world
            .resource::<MainWorld>()
            .get::<MeshMaterial3d<StandardMaterial>>(caster)
            .unwrap()
            .0
            .clone();
        world
            .resource_mut::<MainWorld>()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&handle)
            .unwrap()
            .base_color_texture = Some(image.clone());
        next(&mut world, &mut schedule);
        let first = world.resource::<Inputs>().0[&(light, 0)].clone();
        let empty = world.resource::<Inputs>().0[&(light, 1)].clone();
        world
            .resource_mut::<MainWorld>()
            .resource_mut::<Messages<AssetEvent<Image>>>()
            .write(AssetEvent::Modified { id: image.id() });
        next(&mut world, &mut schedule);
        assert_ne!(first, world.resource::<Inputs>().0[&(light, 0)]);
        assert_eq!(empty, world.resource::<Inputs>().0[&(light, 1)]);
    }
    #[test]
    fn physical_slot_reassignment_and_light_projection_invalidate() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let signature = Signature {
            owner: RetainedViewEntity::new(MainEntity::from(a), None, 0),
            input: FaceInput {
                epoch: 1,
                light: [0; 3],
                casters: vec![a],
            },
            matrices: [[0; 16]; 2],
            pipelines: HashSet::new(),
        };
        let mut other = signature.clone();
        other.owner = RetainedViewEntity::new(MainEntity::from(b), None, 0);
        assert_ne!(signature, other);
        let mut other = signature.clone();
        other.matrices[0][12] = 1;
        assert_ne!(signature, other);
        let mut other = signature.clone();
        other.matrices[1][0] = 1;
        assert_ne!(signature, other);
        let mut other = signature.clone();
        other.input.light[1] = 1;
        assert_ne!(signature, other);
        let mut other = signature.clone();
        other.input.casters.clear();
        assert_ne!(signature, other);
    }
}
