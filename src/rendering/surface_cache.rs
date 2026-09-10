//! Streamed surface clipmaps. Packed RGBA8 color and normal/snow mip chains are
//! stored in a bounded GPU buffer so a cache miss can retain procedural coverage.
use super::sdf::SdfMaterialHandle;
use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
    },
};
mod terrain_samples;
use terrain_samples::{TERRAIN_SLOTS, TerrainSamples};

const SIDE: usize = 256;
const PAGES: usize = 48;
const TEXELS: usize = (SIDE * SIDE * 4 - 1) / 3;
const SURFACE_BYTES: usize = (PAGES * TEXELS + 16 * SIDE * SIDE) * 8;
#[derive(Resource)]
pub(crate) struct SurfaceCache {
    pub pixels: Buffer,
    pub pages: Buffer,
    pub rock_pixels: Buffer,
    terrain: TerrainSamples,
    rock_input: Buffer,
    rock_pipeline: ComputePipeline,
    pipeline: ComputePipeline,
    mip_pipeline: ComputePipeline,
    publish_pipeline: ComputePipeline,
    layout: BindGroupLayout,
    resident: [Option<(IVec2, u32)>; PAGES],
    holes: [Vec4; 32],
    pending: Option<PageBake>,
    bindings: Vec<(UniformBuffer<BakeUniforms>, BindGroup)>,
    timing: Option<BakeTiming>,
    rows: usize,
    rock_timing: Option<BakeTiming>,
    rock_batch: usize,
    rock_binding: Option<(Buffer, UniformBuffer<BakeUniforms>, BindGroup)>,
}
#[derive(ShaderType, Clone)]
struct BakeUniforms {
    world_seed: UVec4,
    camera_position: Vec4,
    orc_holes: [Vec4; 32],
    page: Vec4,
    mip: UVec4,
    tile: UVec4,
}
struct PageBake {
    cell: IVec2,
    slot: usize,
    span: f32,
    seed: UVec4,
    holes: [Vec4; 32],
    row: usize,
}
// Nonblocking timestamps include the bake and, on completion, its mip chain.
// Query/readback buffers are reused. Unsupported devices use a conservative cap.
struct BakeTiming {
    kind: &'static str,
    queries: wgpu::QuerySet,
    resolve: Buffer,
    readback: Buffer,
    status: std::sync::Arc<std::sync::atomic::AtomicU8>,
    rows: usize,
}
impl BakeTiming {
    fn new(device: &RenderDevice, kind: &'static str) -> Option<Self> {
        if !device.features().contains(WgpuFeatures::TIMESTAMP_QUERY) {
            return None;
        }
        Some(Self {
            kind,
            queries: device
                .wgpu_device()
                .create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("Surface bake timings"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
            resolve: device.create_buffer(&BufferDescriptor {
                label: Some("Bake query resolve"),
                size: 256,
                usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&BufferDescriptor {
                label: Some("Bake query readback"),
                size: 16,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            status: default(),
            rows: 8,
        })
    }
    fn poll(&mut self, queue: &RenderQueue) -> Option<f64> {
        use std::sync::atomic::Ordering;
        match self.status.load(Ordering::Acquire) {
            2 => {
                let data = self.readback.slice(..).get_mapped_range();
                let start = u64::from_le_bytes(data[..8].try_into().unwrap());
                let end = u64::from_le_bytes(data[8..16].try_into().unwrap());
                let ms = end.saturating_sub(start) as f64 * queue.get_timestamp_period() as f64
                    / 1_000_000.;
                drop(data);
                self.readback.unmap();
                self.status.store(0, Ordering::Release);
                if std::env::var_os("HITHER_PROFILE_BAKES").is_some() {
                    if self.kind == "ROCK" {
                        info!("ROCK_BAKE vertices={} gpu_ms={:.4}", self.rows, ms);
                    } else if self.kind == "SURFACE" {
                        info!("SURFACE_BAKE rows={} gpu_ms={:.4}", self.rows, ms);
                    } else {
                        info!("TERRAIN_BAKE tiles={} gpu_ms={:.4}", self.rows, ms);
                    }
                }
                // Each caller adapts its own work units from the measured duration.
                Some(ms)
            }
            3 => {
                self.status.store(0, Ordering::Release);
                None
            }
            _ => None,
        }
    }
    fn available(&self) -> bool {
        self.status.load(std::sync::atomic::Ordering::Acquire) == 0
    }
    fn resolve(&self, encoder: &mut CommandEncoder) {
        encoder.resolve_query_set(&self.queries, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, 16);
    }
    fn submitted(&mut self, rows: usize) {
        use std::sync::atomic::Ordering;
        self.rows = rows;
        self.status.store(1, Ordering::Release);
        let status = self.status.clone();
        self.readback
            .slice(..)
            .map_async(MapMode::Read, move |result| {
                status.store(if result.is_ok() { 2 } else { 3 }, Ordering::Release)
            });
    }
}

fn shader() -> String {
    // Compile the same static surface functions as the visible shader. No CPU
    // approximation, duplicated noise implementation, lighting or footprints.
    let scene = include_str!("../../assets/shaders/sdf_scene.wgsl");
    let climate = scene
        .split("fn climate_lattice(")
        .nth(1)
        .unwrap()
        .split("fn tracks_shading(")
        .next()
        .unwrap();
    let surface = scene
        .split("const MOUNTAIN_SNOW_MIN_Y")
        .nth(1)
        .unwrap()
        .split("// Mipmapped static properties.")
        .next()
        .unwrap();
    format!(
        "{}\nfn climate_lattice({}\nconst MOUNTAIN_SNOW_MIN_Y{}\n{}",
        include_str!("../../assets/shaders/surface_cache_head.wgsl"),
        climate,
        surface,
        include_str!("../../assets/shaders/surface_cache_bake.wgsl")
    )
    .replace(
        "let distance = length(p - uniforms.camera_position.xyz);",
        "let distance = 100.0;",
    )
}
pub(crate) fn create(
    commands: &mut Commands,
    device: &RenderDevice,
) -> (Buffer, Buffer, Buffer, Buffer) {
    let pixels = device.create_buffer(&BufferDescriptor {
        label: Some("Surface mip cache and fine climate (40 MiB)"),
        size: SURFACE_BYTES as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let pages = device.create_buffer(&BufferDescriptor {
        label: Some("Surface page table"),
        size: ((PAGES + TERRAIN_SLOTS * 2) * 16) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let rock_pixels = device.create_buffer(&BufferDescriptor {
        label: Some("Static rock surfaces (32 MiB)"),
        size: 4 * 1024 * 1024 * 8,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let rock_input = device.create_buffer(&BufferDescriptor {
        label: Some("Unused rock input"),
        size: 32,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let terrain_pixels = TerrainSamples::buffer(device);
    let layout = device.create_bind_group_layout(
        "surface bake",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    );
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("surface cache"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let module = device.create_and_validate_shader_module(ShaderModuleDescriptor {
        label: Some("shared surface baker"),
        source: ShaderSource::Wgsl(shader().into()),
    });
    let pipeline = |entry| {
        device.create_compute_pipeline(&RawComputePipelineDescriptor {
            label: Some(entry),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some(entry),
            compilation_options: default(),
            cache: None,
        })
    };
    commands.insert_resource(SurfaceCache {
        pixels: pixels.clone(),
        pages: pages.clone(),
        rock_pixels: rock_pixels.clone(),
        terrain: TerrainSamples::new(
            device,
            terrain_pixels.clone(),
            pipeline("terrain_bake"),
            pipeline("terrain_bounds"),
        ),
        rock_input,
        rock_pipeline: pipeline("rock_bake"),
        pipeline: pipeline("bake"),
        mip_pipeline: pipeline("mip"),
        publish_pipeline: pipeline("publish"),
        layout,
        resident: [None; PAGES],
        holes: [Vec4::ZERO; 32],
        pending: None,
        bindings: Vec::new(),
        timing: BakeTiming::new(device, "SURFACE"),
        rows: 8,
        rock_timing: BakeTiming::new(device, "ROCK"),
        rock_batch: 512,
        rock_binding: None,
    });
    (pixels, pages, rock_pixels, terrain_pixels)
}
pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        stream
            .after(crate::world::terrain::TerrainDiscovery)
            .in_set(crate::world::streaming::Layer::Terrain)
            .after(super::sdf::update_shader_uniforms),
    );
}
fn page_slot(cell: IVec2, level: usize) -> usize {
    level * 16 + cell.x.rem_euclid(4) as usize + 4 * cell.y.rem_euclid(4) as usize
}
fn changed_clearings(old: &[Vec4; 32], new: &[Vec4; 32]) -> Vec<Vec4> {
    old.iter()
        .filter(|h| h.z > 0.0 && !new.contains(h))
        .chain(new.iter().filter(|h| h.z > 0.0 && !old.contains(h)))
        .copied()
        .collect()
}
fn clearing_intersects(cell: IVec2, level: usize, hole: Vec4) -> bool {
    let span = [32.0, 128.0, 512.0][level];
    let low = cell.as_vec2() * span;
    let nearest = hole.xy().clamp(low, low + Vec2::splat(span));
    // terrain_height's clearing reaches 44m; baked normals sample +/- 0.5m.
    nearest.distance_squared(hole.xy()) <= 44.5_f32.powi(2)
}
#[allow(clippy::too_many_arguments)]
fn stream(
    mut cache: Option<ResMut<SurfaceCache>>,
    mut coordinator: ResMut<crate::world::streaming::Coordinator>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    settings: Res<crate::app::settings::GraphicsSettings>,
    view: Res<crate::player::camera::CameraView>,
    handles: Res<SdfMaterialHandle>,
    materials: Res<Assets<super::sdf::SdfMaterial>>,
) {
    let Some(ref mut cache) = cache else {
        return;
    };
    let Some(material) = materials.get(&handles.0) else {
        return;
    };
    // Hole order follows camera distance and is not geometry. Compare sets and
    // retire only pages touched by an added/removed clearing (including normals).
    let holes = material.uniforms.orc_holes;
    let changed = changed_clearings(&cache.holes, &holes);
    for slot in 0..PAGES {
        if cache.resident[slot].is_some_and(|(cell, _)| {
            changed
                .iter()
                .any(|hole| clearing_intersects(cell, slot / 16, *hole))
        }) {
            cache.resident[slot] = None;
            queue.write_buffer(&cache.pages, (slot * 16) as u64, &[0; 16]);
        }
    }
    if cache.pending.as_ref().is_some_and(|job| {
        job.seed != material.uniforms.world_seed
            || changed
                .iter()
                .any(|hole| clearing_intersects(job.cell, job.slot / 16, *hole))
    }) {
        cache.pending = None;
    }
    cache.invalidate_terrain(&queue, &changed, material.uniforms.world_seed);
    cache.holes = holes;
    if cache.stream_terrain(&device, &queue, &mut coordinator, &material.uniforms) {
        return;
    }
    // Let measured headroom complete an entire page. A fixed quarter-page
    // ceiling prolongs procedural misses and repeats queue submissions even on
    // GPUs where the full bake and mip chain fit within the target.
    if let Some(ms) = cache.timing.as_mut().and_then(|timing| timing.poll(&queue)) {
        cache.rows = ((cache.timing.as_ref().unwrap().rows as f64 * 0.35 / ms.max(0.001)) as usize)
            .clamp(8, SIDE)
            / 8
            * 8;
    }
    let mut wanted = Vec::new();
    for (level, span) in [32., 128., 512.].into_iter().enumerate() {
        let center = (view.position.xz() / span).floor().as_ivec2();
        for z in -1..=2 {
            for x in -1..=2 {
                let cell = center + IVec2::new(x, z);
                let p = (cell.as_vec2() + Vec2::splat(0.5)) * span;
                if p.distance(view.position.xz()) > settings.render_distance + span * 0.71 {
                    continue;
                }
                let slot = page_slot(cell, level);
                if cache.resident[slot] != Some((cell, material.uniforms.world_seed.x)) {
                    wanted.push((
                        coordinator.visual_rank(Vec3::new(p.x, view.position.y, p.y), span * 0.71),
                        level,
                        cell,
                        slot,
                        span,
                    ));
                }
            }
        }
    }
    wanted.sort_unstable_by_key(|(rank, level, ..)| (*rank, std::cmp::Reverse(*level)));
    if cache.pending.as_ref().is_some_and(|job| {
        !wanted
            .iter()
            .any(|(_, _, cell, slot, _)| *cell == job.cell && *slot == job.slot)
    }) {
        cache.pending = None;
    }
    if cache.pending.is_none() {
        let Some((_, _, cell, slot, span)) = wanted.into_iter().next() else {
            return;
        };
        // Retire the old page before any texel is overwritten. A partially
        // baked page is never visible, even when this toroidal slot was reused.
        queue.write_buffer(&cache.pages, (slot * 16) as u64, &[0; 16]);
        cache.resident[slot] = None;
        cache.pending = Some(PageBake {
            cell,
            slot,
            span,
            seed: material.uniforms.world_seed,
            holes,
            row: 0,
        });
    }
    let Some(_install) = coordinator.install(crate::world::streaming::Layer::Terrain, 1, 0) else {
        return;
    };
    if !coordinator.bake(false) {
        return;
    }
    let mut job = cache.pending.take().unwrap();
    let rows = cache.rows.min(SIDE - job.row);
    let complete = job.row + rows == SIDE;
    let timed = cache.timing.as_ref().is_some_and(BakeTiming::available);
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Incremental surface bake"),
    });
    let mut offset = 0;
    let mut previous = 0;
    let mut size = SIDE;
    let mut index = 0;
    loop {
        let uniforms = BakeUniforms {
            world_seed: job.seed,
            camera_position: Vec4::ZERO,
            orc_holes: job.holes,
            page: Vec4::new(
                job.cell.x as f32 * job.span,
                job.cell.y as f32 * job.span,
                job.span,
                job.slot as f32,
            ),
            mip: UVec4::new(size as u32, offset as u32, previous as u32, SIDE as u32),
            tile: UVec4::new(job.row as u32, rows as u32, 0, 0),
        };
        if cache.bindings.len() <= index {
            let mut buffer = UniformBuffer::from(uniforms.clone());
            buffer.write_buffer(&device, &queue);
            let bind = device.create_bind_group(
                "Reusable surface bake",
                &cache.layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: buffer.binding().unwrap(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: cache.pixels.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: cache.pages.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: cache.rock_input.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: cache.rock_pixels.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: cache.terrain.pixels.as_entire_binding(),
                    },
                ],
            );
            cache.bindings.push((buffer, bind));
        }
        cache.bindings[index].0.set(uniforms);
        cache.bindings[index].0.write_buffer(&device, &queue);
        let last = !complete || size == 1;
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("surface bake stripe/mip"),
                timestamp_writes: if timed && (index == 0 || last) {
                    Some(wgpu::ComputePassTimestampWrites {
                        query_set: &cache.timing.as_ref().unwrap().queries,
                        beginning_of_pass_write_index: (index == 0).then_some(0),
                        end_of_pass_write_index: last.then_some(1),
                    })
                } else {
                    None
                },
            });
            pass.set_pipeline(if size == SIDE {
                &cache.pipeline
            } else {
                &cache.mip_pipeline
            });
            pass.set_bind_group(0, &*cache.bindings[index].1, &[]);
            pass.dispatch_workgroups(
                (size as u32).div_ceil(8),
                (if size == SIDE { rows } else { size } as u32).div_ceil(8),
                1,
            );
            if size == 1 {
                pass.set_pipeline(&cache.publish_pipeline);
                pass.dispatch_workgroups(1, 1, 1);
            }
        }
        if last {
            break;
        }
        previous = offset;
        offset += size * size;
        size /= 2;
        index += 1;
    }
    if timed {
        cache.timing.as_ref().unwrap().resolve(&mut encoder);
    }
    queue.submit([encoder.finish()]);
    if timed {
        cache.timing.as_mut().unwrap().submitted(rows);
    }
    job.row += rows;
    if complete {
        cache.resident[job.slot] = Some((job.cell, job.seed.x));
    } else {
        cache.pending = Some(job);
    }
}
impl SurfaceCache {
    pub(crate) fn rock_batch(&mut self, queue: &RenderQueue) -> usize {
        if let Some(ms) = self.rock_timing.as_mut().and_then(|t| t.poll(queue)) {
            self.rock_batch = ((self.rock_timing.as_ref().unwrap().rows as f64 * 0.35
                / ms.max(0.001)) as usize)
                .clamp(64, 8192)
                / 64
                * 64;
        }
        self.rock_batch
    }
    pub(crate) fn bake_rocks(
        &mut self,
        device: &RenderDevice,
        queue: &RenderQueue,
        offset: usize,
        vertices: &[[Vec4; 2]],
        position: Vec3,
    ) {
        if vertices.is_empty() {
            return;
        }
        let bytes: Vec<_> = vertices
            .iter()
            .flatten()
            .flat_map(|v| v.to_array())
            .flat_map(f32::to_le_bytes)
            .collect();
        assert!(
            vertices.len() <= 8192,
            "rock bake must be admitted in bounded batches"
        );
        let values = BakeUniforms {
            world_seed: crate::world::biome::shader_seed(),
            camera_position: Vec4::ZERO,
            orc_holes: crate::world::orcs::hole_uniforms(position, 64.0),
            page: Vec4::new(0., 0., 0., offset as f32),
            mip: UVec4::new(vertices.len() as u32, 0, 0, 0),
            tile: UVec4::ZERO,
        };
        if self.rock_binding.is_none() {
            let input = device.create_buffer(&BufferDescriptor {
                label: Some("Reusable rock surface input"),
                size: 8192 * 32,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut uniforms = UniformBuffer::from(values.clone());
            uniforms.write_buffer(device, queue);
            let bind = device.create_bind_group(
                "Reusable rock bake",
                &self.layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniforms.binding().unwrap(),
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
                        resource: input.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: self.rock_pixels.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: self.terrain.pixels.as_entire_binding(),
                    },
                ],
            );
            self.rock_binding = Some((input, uniforms, bind));
        }
        let (input, uniforms, bind) = self.rock_binding.as_mut().unwrap();
        queue.write_buffer(input, 0, &bytes);
        uniforms.set(values);
        uniforms.write_buffer(device, queue);
        let timed = self.rock_timing.as_ref().is_some_and(BakeTiming::available);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Static rock surfaces"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("bake rocks"),
                timestamp_writes: timed.then(|| wgpu::ComputePassTimestampWrites {
                    query_set: &self.rock_timing.as_ref().unwrap().queries,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            pass.set_pipeline(&self.rock_pipeline);
            pass.set_bind_group(0, &**bind, &[]);
            pass.dispatch_workgroups((vertices.len() as u32).div_ceil(64), 1, 1);
        }
        if timed {
            self.rock_timing.as_ref().unwrap().resolve(&mut encoder);
        }
        queue.submit([encoder.finish()]);
        if timed {
            self.rock_timing.as_mut().unwrap().submitted(vertices.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fine_climate_cache_preserves_ecological_transitions() {
        use crate::world::biome;
        let mut worst = Vec3::ZERO;
        for seed in [42, 721] {
            let fields = |p: Vec2| {
                Vec3::new(
                    biome::snow_for_seed(p, seed),
                    biome::mountain_for_seed(p, seed),
                    biome::forest_for_seed(p, seed) * biome::stand_density(p),
                )
            };
            for spacing in [0.125_f32] {
                let mut level_worst = Vec3::ZERO;
                for i in 0..5000_u32 {
                    let p = Vec2::new(
                        (i.wrapping_mul(2654435761) % 6400000) as f32 / 100.0 - 32000.0,
                        (i.wrapping_mul(2246822519) % 6400000) as f32 / 100.0 - 32000.0,
                    );
                    let q = p / spacing - Vec2::splat(0.5);
                    let cell = q.floor();
                    let f = q - cell;
                    let sample = |offset: Vec2| {
                        let v = fields((cell + offset + Vec2::splat(0.5)) * spacing);
                        Vec3::from_array(v.to_array().map(|v| half::f16::from_f32(v).to_f32()))
                    };
                    let cached = sample(Vec2::ZERO)
                        .lerp(sample(Vec2::X), f.x)
                        .lerp(sample(Vec2::Y).lerp(sample(Vec2::ONE), f.x), f.y);
                    worst = worst.max((cached - fields(p)).abs());
                    level_worst = level_worst.max((cached - fields(p)).abs());
                }
                println!("seed={seed} spacing={spacing} error={level_worst:?}");
            }
        }
        assert!(worst.max_element() < 0.002, "climate cache error {worst:?}");
    }
    #[test]
    fn shared_surface_baker_validates_with_climate_payload() {
        let source = shader();
        let module = wgpu::naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
    }
    #[test]
    fn hole_reordering_preserves_pages_and_geometry_edits_are_local() {
        let mut old = [Vec4::ZERO; 32];
        old[0] = Vec4::new(1., 1., 3., 4.);
        old[1] = Vec4::new(1000., -1000., 5., -6.);
        let mut new = old;
        new.swap(0, 1);
        assert!(changed_clearings(&old, &new).is_empty());
        new[1].x += 100.;
        let changed = changed_clearings(&old, &new);
        assert_eq!(changed.len(), 2);
        assert!(
            changed
                .iter()
                .any(|h| clearing_intersects(IVec2::ZERO, 0, *h))
        );
        assert!(
            changed
                .iter()
                .any(|h| clearing_intersects(IVec2::new(3, 0), 0, *h))
        );
        assert!(
            !changed
                .iter()
                .any(|h| clearing_intersects(IVec2::new(10, 10), 0, *h))
        );
        new[1] = Vec4::ZERO;
        assert_eq!(changed_clearings(&old, &new), vec![old[0]]);
        // The derivative sample just beyond a page edge must invalidate it too.
        assert!(clearing_intersects(
            IVec2::ZERO,
            0,
            Vec4::new(76.4, 10., 3., 4.)
        ));
        assert!(!clearing_intersects(
            IVec2::ZERO,
            0,
            Vec4::new(76.6, 10., 3., 4.)
        ));
    }
    #[test]
    fn negative_pages_have_unique_slots_and_bounded_mip_storage() {
        let mut slots = std::collections::HashSet::new();
        for level in 0..3 {
            for x in -3..1 {
                for y in -7..-3 {
                    assert!(slots.insert(page_slot(IVec2::new(x, y), level)));
                }
            }
        }
        assert_eq!(slots.len(), 48);
        const {
            assert!(SURFACE_BYTES <= 40 * 1024 * 1024);
        }
    }
}
