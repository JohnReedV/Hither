//! Exact procedural vertex samples, shared by color, depth and shadow passes.
//! Toroidal aliases always miss; CPU-baked meshes stop requesting these pages.
use super::*;
use std::collections::{HashSet, VecDeque};

pub(super) const TERRAIN_SLOTS: usize = 32 * 32;
fn slot(cell: IVec2) -> usize {
    cell.x.rem_euclid(32) as usize + cell.y.rem_euclid(32) as usize * 32
}
struct Residency {
    cells: Vec<Option<IVec2>>,
    seed: Option<u32>,
}
impl Default for Residency {
    fn default() -> Self {
        Self {
            cells: vec![None; TERRAIN_SLOTS],
            seed: None,
        }
    }
}
impl Residency {
    fn contains(&self, cell: IVec2) -> bool {
        self.cells[slot(cell)] == Some(cell)
    }
    fn publish(&mut self, cell: IVec2) -> Option<IVec2> {
        self.cells[slot(cell)].replace(cell)
    }
    fn invalidate(&mut self, changed: &[Vec4], seed: UVec4) -> Vec<(usize, IVec2)> {
        let seed_changed = self.seed != Some(seed.x);
        self.seed = Some(seed.x);
        if !seed_changed && changed.is_empty() {
            return Vec::new();
        }
        let mut invalidated = Vec::new();
        for (index, resident) in self.cells.iter_mut().enumerate() {
            if let Some(cell) = *resident
                && (seed_changed
                    || changed
                        .iter()
                        .any(|hole| clearing_intersects(cell, 0, *hole)))
            {
                *resident = None;
                invalidated.push((index, cell));
            }
        }
        invalidated
    }
}
#[derive(Default)]
struct Requests {
    wanted: HashSet<IVec2>,
    queued: HashSet<IVec2>,
    queue: VecDeque<IVec2>,
    cpu: std::collections::HashMap<IVec2, std::time::Instant>,
}
impl Requests {
    fn enqueue(&mut self, cell: IVec2) {
        if self.wanted.contains(&cell) && self.queued.insert(cell) {
            self.queue.push_back(cell);
        }
    }
    // A toroidal collision must converge on one winner while the view is
    // unchanged. A displaced request may wait, but cannot evict a better page.
    fn may_replace<K: Ord>(
        &self,
        resident: Option<IVec2>,
        candidate: IVec2,
        rank: impl Fn(IVec2) -> K,
    ) -> bool {
        resident.is_none_or(|old| !self.wanted.contains(&old) || rank(candidate) < rank(old))
    }
    fn pop(&mut self) -> Option<IVec2> {
        let cell = self.queue.pop_front()?;
        self.queued.remove(&cell);
        Some(cell)
    }
}
pub(super) struct TerrainSamples {
    pub(super) pixels: Buffer,
    pipeline: ComputePipeline,
    bounds_pipeline: ComputePipeline,
    bounds_readback: Buffer,
    bounds_status: std::sync::Arc<std::sync::atomic::AtomicU8>,
    bounds_snapshot: Vec<(Option<IVec2>, u64)>,
    revisions: Vec<u64>,
    published_bounds: Vec<Option<u64>>,
    bounds_dirty: bool,
    bounds_updates: Vec<(IVec2, Option<Vec2>)>,
    resident: Residency,
    requests: Requests,
    bindings: Vec<(UniformBuffer<BakeUniforms>, BindGroup)>,
    timing: Option<BakeTiming>,
    batch: usize,
    enabled: bool,
    // Give surface material pages every other terrain turn during movement.
    yield_surface: bool,
}
impl TerrainSamples {
    pub(super) fn buffer(device: &RenderDevice) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("Reusable terrain height and normal samples"),
            size: (TERRAIN_SLOTS * 1089 * 16) as u64,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        })
    }
    pub(super) fn new(
        device: &RenderDevice,
        pixels: Buffer,
        pipeline: ComputePipeline,
        bounds_pipeline: ComputePipeline,
    ) -> Self {
        Self {
            pixels,
            pipeline,
            bounds_pipeline,
            bounds_readback: device.create_buffer(&BufferDescriptor {
                label: Some("Terrain tile bounds readback"),
                size: (TERRAIN_SLOTS * 16) as u64,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            bounds_status: default(),
            bounds_snapshot: Vec::new(),
            revisions: vec![0; TERRAIN_SLOTS],
            published_bounds: vec![None; TERRAIN_SLOTS],
            bounds_dirty: false,
            bounds_updates: Vec::new(),
            resident: default(),
            requests: default(),
            bindings: Vec::new(),
            timing: BakeTiming::new(device, "TERRAIN"),
            batch: 1,
            enabled: std::env::var_os("HITHER_PROFILE_NO_TERRAIN_CACHE").is_none(),
            yield_surface: false,
        }
    }
}
impl TerrainSamples {
    fn poll_bounds(&mut self, device: &RenderDevice, queue: &RenderQueue, pages: &Buffer) {
        use std::sync::atomic::Ordering;
        match self.bounds_status.load(Ordering::Acquire) {
            2 => {
                let data = self.bounds_readback.slice(..).get_mapped_range();
                for (index, &(cell, revision)) in self.bounds_snapshot.iter().enumerate() {
                    let Some(cell) = cell else { continue };
                    if self.published_bounds[index] == Some(revision)
                        || self.revisions[index] != revision
                        || !self.resident.contains(cell)
                        || !self.requests.wanted.contains(&cell)
                    {
                        continue;
                    }
                    let row: [f32; 4] = std::array::from_fn(|channel| {
                        let start = index * 16 + channel * 4;
                        f32::from_le_bytes(data[start..start + 4].try_into().unwrap())
                    });
                    if Vec2::new(row[0], row[1]) == cell.as_vec2() * 32.0
                        && row[2].is_finite()
                        && row[3].is_finite()
                        && row[2] <= row[3]
                    {
                        self.published_bounds[index] = Some(revision);
                        self.bounds_updates
                            .push((cell, Some(Vec2::new(row[2], row[3]))));
                    }
                }
                drop(data);
                self.bounds_readback.unmap();
                self.bounds_status.store(0, Ordering::Release);
            }
            3 => {
                self.bounds_status.store(0, Ordering::Release);
                self.bounds_dirty = true;
            }
            _ => {}
        }
        if self.bounds_dirty && self.bounds_status.load(Ordering::Acquire) == 0 {
            self.bounds_snapshot = self
                .resident
                .cells
                .iter()
                .copied()
                .zip(self.revisions.iter().copied())
                .collect();
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Publish conservative terrain bounds"),
            });
            encoder.copy_buffer_to_buffer(
                pages,
                ((PAGES + TERRAIN_SLOTS) * 16) as u64,
                &self.bounds_readback,
                0,
                (TERRAIN_SLOTS * 16) as u64,
            );
            queue.submit([encoder.finish()]);
            self.bounds_dirty = false;
            self.bounds_status.store(1, Ordering::Release);
            let status = self.bounds_status.clone();
            self.bounds_readback
                .slice(..)
                .map_async(MapMode::Read, move |result| {
                    status.store(if result.is_ok() { 2 } else { 3 }, Ordering::Release);
                });
        }
    }
}
impl SurfaceCache {
    pub(crate) fn take_terrain_bounds(&mut self) -> Vec<(IVec2, Option<Vec2>)> {
        std::mem::take(&mut self.terrain.bounds_updates)
    }
    pub(crate) fn request_terrain(&mut self, cell: IVec2) {
        if self.terrain.requests.wanted.insert(cell) && self.terrain.resident.contains(cell) {
            self.terrain.published_bounds[slot(cell)] = None;
            self.terrain.bounds_dirty = true;
        }
        if !self.terrain.resident.contains(cell) {
            self.terrain.requests.enqueue(cell);
        }
    }
    pub(crate) fn cpu_terrain_pending(&mut self, cell: IVec2) {
        self.terrain
            .requests
            .cpu
            .entry(cell)
            .or_insert_with(std::time::Instant::now);
    }
    pub(crate) fn forget_terrain(&mut self, cell: IVec2) {
        self.terrain.requests.cpu.remove(&cell);
        self.terrain.requests.wanted.remove(&cell);
    }
    pub(super) fn invalidate_terrain(
        &mut self,
        queue: &RenderQueue,
        changed: &[Vec4],
        seed: UVec4,
    ) {
        let terrain = &mut self.terrain;
        for (index, cell) in terrain.resident.invalidate(changed, seed) {
            terrain.revisions[index] = terrain.revisions[index].wrapping_add(1);
            terrain.bounds_updates.push((cell, None));
            queue.write_buffer(&self.pages, ((PAGES + index) * 16) as u64, &[0; 16]);
            terrain.requests.enqueue(cell);
        }
    }

    pub(super) fn stream_terrain(
        &mut self,
        device: &RenderDevice,
        queue: &RenderQueue,
        coordinator: &mut crate::world::streaming::Coordinator,
        source: &super::super::sdf::SdfUniforms,
    ) -> bool {
        let terrain = &mut self.terrain;
        terrain.poll_bounds(device, queue, &self.pages);
        if !terrain.enabled {
            return false;
        }
        if let Some(ms) = terrain
            .timing
            .as_mut()
            .and_then(|timing| timing.poll(queue))
        {
            terrain.batch = ((terrain.timing.as_ref().unwrap().rows as f64 * 0.35 / ms.max(0.001))
                as usize)
                .clamp(1, 8);
        }
        if terrain.yield_surface {
            terrain.yield_surface = false;
            return false;
        }
        let rank = |cell: IVec2| {
            let p = (cell.as_vec2() + Vec2::splat(0.5)) * 32.;
            (
                coordinator.visual_rank(Vec3::new(p.x, source.camera_position.y, p.y), 24.),
                cell.x,
                cell.y,
            )
        };
        let mut candidates = Vec::new();
        for _ in 0..terrain.requests.queue.len().min(128) {
            let cell = terrain.requests.pop().unwrap();
            // Give an admitted worker a short head start. Slow/stalled workers
            // cannot leave visible fallback vertices procedural indefinitely.
            if terrain
                .requests
                .cpu
                .get(&cell)
                .is_some_and(|start| start.elapsed().as_millis() < 20)
            {
                terrain.requests.enqueue(cell);
                continue;
            }
            terrain.requests.cpu.remove(&cell);
            if terrain.requests.wanted.contains(&cell) && !terrain.resident.contains(cell) {
                if !terrain
                    .requests
                    .may_replace(terrain.resident.cells[slot(cell)], cell, rank)
                {
                    terrain.requests.enqueue(cell);
                    continue;
                }
                candidates.push((rank(cell), cell));
            }
        }
        if candidates.is_empty() {
            return false;
        }
        candidates.sort_unstable_by_key(|(rank, cell)| (*rank, cell.x, cell.y));
        let admission = coordinator.install(crate::world::streaming::Layer::Terrain, 1, 0);
        if admission.is_none() || !coordinator.bake(false) {
            for (_, cell) in candidates {
                terrain.requests.enqueue(cell);
            }
            return false;
        }
        let mut selected = Vec::new();
        for (_, cell) in candidates {
            if selected.len() < terrain.batch && !selected.iter().any(|c| slot(*c) == slot(cell)) {
                selected.push(cell);
            } else {
                terrain.requests.enqueue(cell);
            }
        }
        let timed = terrain.timing.as_ref().is_some_and(BakeTiming::available);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Terrain sample bake"),
        });
        for (index, cell) in selected.iter().copied().enumerate() {
            let page_slot = slot(cell);
            terrain.revisions[page_slot] = terrain.revisions[page_slot].wrapping_add(1);
            if let Some(old) = terrain.resident.publish(cell) {
                terrain.requests.enqueue(old);
                terrain.bounds_updates.push((old, None));
            }
            terrain.bounds_dirty = true;
            queue.write_buffer(&self.pages, ((PAGES + page_slot) * 16) as u64, &[0; 16]);
            let uniforms = BakeUniforms {
                world_seed: source.world_seed,
                camera_position: Vec4::ZERO,
                orc_holes: source.orc_holes,
                page: Vec4::new(
                    cell.x as f32 * 32.,
                    cell.y as f32 * 32.,
                    32.,
                    page_slot as f32,
                ),
                mip: UVec4::ZERO,
                tile: UVec4::ZERO,
            };
            if terrain.bindings.len() <= index {
                let mut buffer = UniformBuffer::from(uniforms.clone());
                buffer.write_buffer(device, queue);
                let bind = device.create_bind_group(
                    "Terrain sample bindings",
                    &self.layout,
                    &[
                        BindGroupEntry {
                            binding: 0,
                            resource: buffer.binding().unwrap(),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: self.pixels.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: self.pages.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 3,
                            resource: self.rock_input.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 4,
                            resource: self.rock_pixels.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 5,
                            resource: terrain.pixels.as_entire_binding(),
                        },
                    ],
                );
                terrain.bindings.push((buffer, bind));
            }
            terrain.bindings[index].0.set(uniforms);
            terrain.bindings[index].0.write_buffer(device, queue);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Terrain height and normal tile"),
                    timestamp_writes: (timed && (index == 0 || index + 1 == selected.len())).then(
                        || wgpu::ComputePassTimestampWrites {
                            query_set: &terrain.timing.as_ref().unwrap().queries,
                            beginning_of_pass_write_index: (index == 0).then_some(0),
                            end_of_pass_write_index: (index + 1 == selected.len()).then_some(1),
                        },
                    ),
                });
                pass.set_pipeline(&terrain.pipeline);
                pass.set_bind_group(0, &*terrain.bindings[index].1, &[]);
                pass.dispatch_workgroups(5, 5, 1);
                pass.set_pipeline(&terrain.bounds_pipeline);
                pass.dispatch_workgroups(1, 1, 1);
            }
        }
        if timed {
            terrain.timing.as_ref().unwrap().resolve(&mut encoder);
        }
        queue.submit([encoder.finish()]);
        if timed {
            terrain.timing.as_mut().unwrap().submitted(selected.len());
        }
        terrain.yield_surface = true;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliased_requests_converge_and_follow_view_priority() {
        let a = IVec2::ZERO;
        let b = IVec2::new(32, 0);
        let mut requests = Requests::default();
        requests.wanted.extend([a, b]);
        let mut resident = Residency::default();
        let rank = |p: IVec2| (p.x.abs(), p.y);
        let mut bakes = 0;
        for _ in 0..100 {
            for cell in [b, a] {
                if !resident.contains(cell)
                    && requests.may_replace(resident.cells[slot(cell)], cell, rank)
                {
                    resident.publish(cell);
                    bakes += 1;
                }
            }
        }
        assert_eq!(bakes, 2);
        assert!(resident.contains(a));
        assert!(requests.may_replace(Some(a), b, |p: IVec2| (p - b).length_squared()));
        requests.wanted.remove(&a);
        assert!(requests.may_replace(Some(a), b, rank));
    }
    #[test]
    fn negative_cells_wrap_and_aliases_require_origin_check() {
        assert_eq!(slot(IVec2::new(-1, -1)), TERRAIN_SLOTS - 1);
        assert_eq!(slot(IVec2::ZERO), slot(IVec2::splat(32)));
        let mut resident = Residency::default();
        assert_eq!(resident.publish(IVec2::ZERO), None);
        assert!(resident.contains(IVec2::ZERO));
        assert!(!resident.contains(IVec2::splat(32)));
        assert_eq!(resident.publish(IVec2::splat(32)), Some(IVec2::ZERO));
        assert!(!resident.contains(IVec2::ZERO));
        let cells: HashSet<_> = (-37..-5)
            .flat_map(|x| (-19..13).map(move |y| slot(IVec2::new(x, y))))
            .collect();
        assert_eq!(cells.len(), TERRAIN_SLOTS);
    }
    #[test]
    fn seed_and_clearing_edits_invalidate_only_affected_residency() {
        let mut resident = Residency::default();
        let seed = UVec4::new(721, 0, 0, 0);
        resident.invalidate(&[], seed);
        resident.publish(IVec2::ZERO);
        resident.publish(IVec2::new(10, 10));
        assert!(resident.invalidate(&[], seed).is_empty());
        assert!(
            resident
                .invalidate(&[], UVec4::new(721, 1, 0, 0))
                .is_empty()
        );
        let edge_hole = Vec4::new(76.4, 10., 3., 4.);
        assert_eq!(
            resident.invalidate(&[edge_hole], seed),
            vec![(slot(IVec2::ZERO), IVec2::ZERO)]
        );
        assert!(!resident.contains(IVec2::ZERO));
        assert!(resident.contains(IVec2::new(10, 10)));
        let invalidated = resident.invalidate(&[], UVec4::ONE);
        assert_eq!(invalidated.len(), 1);
        assert!(resident.cells.iter().all(Option::is_none));
    }
    #[test]
    fn requests_deduplicate_and_allow_reentry_after_forget() {
        let mut requests = Requests::default();
        let cell = IVec2::new(-2, 3);
        requests.wanted.insert(cell);
        requests.enqueue(cell);
        requests.enqueue(cell);
        assert_eq!(requests.queue.len(), 1);
        requests.wanted.remove(&cell);
        assert_eq!(requests.pop(), Some(cell));
        requests.enqueue(cell);
        assert!(requests.queue.is_empty());
        requests.wanted.insert(cell);
        requests.enqueue(cell);
        assert_eq!(requests.pop(), Some(cell));
        assert!(requests.queued.is_empty());
    }
}
