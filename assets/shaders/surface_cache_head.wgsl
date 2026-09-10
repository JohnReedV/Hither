struct BakeUniforms {
    world_seed: vec4<u32>, camera_position: vec4<f32>,
    orc_holes: array<vec4<f32>,32>, page: vec4<f32>, mip: vec4<u32>, tile: vec4<u32>,
}
@group(0) @binding(0) var<uniform> uniforms: BakeUniforms;
@group(0) @binding(1) var<storage,read_write> pixels: array<vec2<u32>>;
@group(0) @binding(2) var<storage,read_write> pages: array<vec4<f32>>;

struct RockBakeVertex {position:vec4<f32>,normal:vec4<f32>}
@group(0) @binding(3) var<storage,read> rock_input:array<RockBakeVertex>;
@group(0) @binding(4) var<storage,read_write> rock_pixels:array<vec2<u32>>;

@group(0) @binding(5) var<storage,read_write> terrain_samples:array<vec4<f32>>;
