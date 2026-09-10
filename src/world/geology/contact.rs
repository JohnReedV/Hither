//! Exact static rock contacts, shared by color/depth passes and both LODs.
//! MeshTag selects an allocation without breaking mesh/material instancing.
use super::*;
use crate::world::streaming::{Coordinator, Layer};
use bevy::{
    camera::primitives::{Aabb, Frustum},
    mesh::{MeshTag, MeshVertexAttribute},
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::{Buffer, BufferDescriptor, BufferUsages, VertexFormat},
        renderer::{RenderDevice, RenderQueue},
    },
};
pub(crate) const INDEX: MeshVertexAttribute =
    MeshVertexAttribute::new("RockContactIndex", 0x726f636b, VertexFormat::Uint32);
const CAPACITY: usize = 4 * 1024 * 1024; // 64 MiB maximum; safe procedural fallback when full.
#[derive(Resource, Clone, ExtractResource)]
pub(crate) struct Uploads {
    pub buffer: Buffer,
    writes: Vec<(usize, Arc<Vec<u8>>)>,
}
pub(crate) fn create(commands: &mut Commands, device: &RenderDevice) -> Buffer {
    let size = (CAPACITY as u64 * 16).min(device.limits().max_storage_buffer_binding_size);
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Static rock contacts"),
        size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    commands.insert_resource(Uploads {
        buffer: buffer.clone(),
        writes: Vec::new(),
    });
    commands.insert_resource(Cache {
        free: std::iter::once(1..size as usize / 16).collect(),
        ..default()
    });
    buffer
}
pub(super) fn plugin(app: &mut App) {
    app.add_plugins(ExtractResourcePlugin::<Uploads>::default());
    app.add_systems(Update, update.in_set(Layer::Contacts).after(super::stream));
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(Render, upload.in_set(RenderSystems::PrepareResources));
    }
}
fn upload(data: Option<Res<Uploads>>, queue: Res<RenderQueue>) {
    if let Some(data) = data {
        for (offset, bytes) in &data.writes {
            queue.write_buffer(&data.buffer, (*offset * 16) as u64, bytes);
        }
    }
}
type ContactBuild = (Vec<Vec4>, Vec<[Vec4; 2]>);
type ContactBake = (Entity, Vec<Vec4>, Vec<[Vec4; 2]>, usize);
#[derive(Resource, Default)]
struct Cache {
    free: Vec<std::ops::Range<usize>>,
    live: HashMap<Entity, std::ops::Range<usize>>,
    waiting: VecDeque<Entity>,
    jobs: Vec<(Entity, crate::world::streaming::BuildTask<ContactBuild>)>,
    baking: Vec<ContactBake>,
}
impl Cache {
    fn release(&mut self, range: std::ops::Range<usize>) {
        self.free.push(range);
        self.free.sort_unstable_by_key(|r| r.start);
        let mut i = 1;
        while i < self.free.len() {
            if self.free[i - 1].end == self.free[i].start {
                self.free[i - 1].end = self.free[i].end;
                self.free.remove(i);
            } else {
                i += 1;
            }
        }
    }
    fn allocate(&mut self, n: usize) -> Option<std::ops::Range<usize>> {
        let i = self.free.iter().position(|r| r.len() >= n)?;
        let result = self.free[i].start..self.free[i].start + n;
        self.free[i].start += n;
        if self.free[i].is_empty() {
            self.free.remove(i);
        }
        Some(result)
    }
}
pub(super) fn enqueue(commands: &mut Commands, entity: Entity) {
    commands.queue(move |world: &mut World| {
        if let Some(mut cache) = world.get_resource_mut::<Cache>() {
            cache.waiting.push_back(entity);
        }
    });
}

fn contacts(model: usize, transform: Transform) -> Vec<Vec4> {
    let m = &models()[model];
    let mut values = HashMap::new();
    let height = |p: Vec2| {
        biome::terrain_for_seed(p, biome::world_seed()) * crate::world::orcs::terrain_clearance(p)
    };
    m.geometry
        .positions
        .iter()
        .chain(&m.low.positions)
        .map(|p| {
            let p = transform.transform_point(Vec3::from(*p)).xz();
            *values
                .entry([p.x.to_bits(), p.y.to_bits()])
                .or_insert_with(|| {
                    let n = Vec3::new(
                        height(p - Vec2::X * 0.5) - height(p + Vec2::X * 0.5),
                        1.,
                        height(p - Vec2::Y * 0.5) - height(p + Vec2::Y * 0.5),
                    )
                    .normalize();
                    n.extend(height(p))
                })
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
fn update(
    mut coordinator: ResMut<Coordinator>,
    mut commands: Commands,
    mut cache: Option<ResMut<Cache>>,
    mut uploads: Option<ResMut<Uploads>>,
    mut removed: RemovedComponents<RockLod>,
    mut surfaces: Option<ResMut<crate::rendering::surface_cache::SurfaceCache>>,
    device: Option<Res<RenderDevice>>,
    queue: Option<Res<RenderQueue>>,
    rocks: Query<(
        &RockLod,
        &Transform,
        Option<&Aabb>,
        Option<&GlobalTransform>,
    )>,
    cameras: Query<&Frustum, With<crate::player::avatar::PlayerCamera>>,
) {
    let (Some(cache), Some(uploads)) = (&mut cache, &mut uploads) else {
        return;
    };
    uploads.writes.clear();
    let mut removed_any = false;
    for e in removed.read() {
        removed_any = true;
        if let Some(range) = cache.live.remove(&e) {
            cache.release(range);
        }
    }
    if removed_any {
        cache.waiting.retain(|e| rocks.contains(*e));
        cache.baking.retain(|(e, ..)| rocks.contains(*e));
    }
    let mut i = 0;
    while i < cache.jobs.len() {
        if cache.jobs[i].1.is_ready() {
            let entity = cache.jobs[i].0;
            let installation = coordinator.install(Layer::Contacts, 1, 0);
            if rocks.contains(entity) && installation.is_none() {
                i += 1;
                continue;
            }
            let (e, mut task) = cache.jobs.swap_remove(i);
            let (values, vertices) = task.take();
            if rocks.contains(e)
                && let Some(range) = cache.allocate(values.len())
            {
                cache.live.insert(e, range);
                cache.baking.push((e, values, vertices, 0));
            }
        } else {
            i += 1;
        }
    }
    // One bounded rock batch per frame. Do not expose its MeshTag until every
    // contact and surface vertex is uploaded, so partial data is never sampled.
    if let Some((entity, values, vertices, cursor)) = cache.baking.first() {
        let entity = *entity;
        let start = *cursor;
        let batch = if let (Some(surfaces), Some(queue)) = (&mut surfaces, &queue) {
            surfaces.rock_batch(queue)
        } else {
            512
        };
        let end = (start + batch).min(vertices.len());
        let done = end == vertices.len();
        let bytes = (end - start) * 32 + if done { values.len() * 16 } else { 0 };
        if let Some(_scope) = coordinator.install(Layer::Contacts, 4, bytes)
            && coordinator.bake(true)
        {
            let range = cache.live[&entity].clone();
            if let (Some(surfaces), Some(device), Some(queue)) = (&mut surfaces, &device, &queue) {
                let position = rocks
                    .get(entity)
                    .map_or(Vec3::ZERO, |(_, t, ..)| t.translation);
                surfaces.bake_rocks(
                    device,
                    queue,
                    range.start + start,
                    &vertices[start..end],
                    position,
                );
            }
            if done {
                let bytes = values
                    .iter()
                    .flat_map(|v| v.to_array())
                    .flat_map(f32::to_le_bytes)
                    .collect();
                uploads.writes.push((range.start, Arc::new(bytes)));
                commands.entity(entity).insert(MeshTag(range.start as u32));
                cache.baking.remove(0);
            } else {
                cache.baking[0].3 = end;
            }
        }
    }
    let frustum = cameras.iter().next();
    for _ in 0..cache.waiting.len().min(128) {
        if cache.jobs.len() + cache.baking.len() >= 4 {
            break;
        }
        let Some(e) = cache.waiting.pop_front() else {
            break;
        };
        let Ok((lod, transform, aabb, global)) = rocks.get(e) else {
            continue;
        };
        if let Some(frustum) = frustum
            && !aabb
                .zip(global)
                .is_some_and(|(a, t)| frustum.intersects_obb(a, &t.affine(), true, true))
        {
            cache.waiting.push_back(e);
            continue;
        }
        let model = lod.index;
        let transform = *transform;
        let Some(permit) = coordinator.worker(Layer::Contacts) else {
            cache.waiting.push_front(e);
            break;
        };
        cache.jobs.push((
            e,
            crate::world::streaming::BuildTask::new(permit, move || {
                let source = &models()[model];
                let vertices = source
                    .geometry
                    .positions
                    .iter()
                    .zip(&source.geometry.normals)
                    .chain(source.low.positions.iter().zip(&source.low.normals))
                    .map(|(p, n)| {
                        let p = transform.transform_point(Vec3::from(*p));
                        let n = (transform.rotation * (Vec3::from(*n) / transform.scale))
                            .normalize_or(Vec3::Y);
                        [p.extend(0.2), n.extend(0.0)]
                    })
                    .collect();
                (contacts(model, transform), vertices)
            }),
        ));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocations_coalesce_and_cannot_overlap() {
        let mut c = Cache {
            free: std::iter::once(1..100).collect(),
            ..default()
        };
        let a = c.allocate(30).unwrap();
        let b = c.allocate(40).unwrap();
        assert_eq!(a, 1..31);
        assert_eq!(b, 31..71);
        assert!(c.allocate(30).is_none());
        c.release(a);
        c.release(b);
        assert_eq!(c.free, vec![1..100]);
    }
    #[test]
    fn contacts_cover_both_lods_with_unit_normals() {
        let t = Transform::from_xyz(592., 30., 1344.).with_scale(Vec3::splat(2.));
        let data = contacts(0, t);
        let model = &models()[0];
        assert_eq!(
            data.len(),
            model.geometry.positions.len() + model.low.positions.len()
        );
        for (v, p) in data
            .iter()
            .zip(model.geometry.positions.iter().chain(&model.low.positions))
        {
            assert!((v.truncate().length() - 1.).abs() < 0.00001);
            let p = t.transform_point(Vec3::from(*p)).xz();
            assert_eq!(
                v.w,
                biome::terrain_for_seed(p, biome::world_seed())
                    * crate::world::orcs::terrain_clearance(p)
            );
        }
    }
}
