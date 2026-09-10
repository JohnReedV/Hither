// Sky uses a fullscreen pass; terrain and world objects use hardware rasterization.
// All three variants share world-space surfaces, climate and lighting.
#ifdef TERRAIN_MESH
#import bevy_pbr::view_transformations::position_world_to_clip
#endif
#ifdef TERRAIN_MESH
#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_normal_local_to_world, get_tag}
#endif
#ifndef PREPASS_PIPELINE
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#ifdef MULTISAMPLED
#import bevy_pbr::mesh_view_bindings as view_bindings
#endif
#endif
#endif
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage,read> rock_surfaces:array<vec2<u32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<storage, read> rock_contacts: array<vec4<f32>>;

struct VertexOutput {
    // Color and depth are separately compiled programs. Their raster positions
    // must agree even far from the origin, where contraction/rounding differences
    // can otherwise reject a whole terrain triangle in the color pass.
    @invariant @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
#ifdef TERRAIN_MESH
    @location(1) world_position: vec3<f32>,
    @location(2) normal: vec3<f32>,
#ifdef ROCK_MESH
    @location(3) ground_height: f32,
    @location(4) ground_normal: vec3<f32>,
    @location(5) contact_scale: f32,
    @location(6) static_color:vec4<f32>,
    @location(7) static_normal:vec4<f32>,
#endif
#endif
};

@vertex
fn vertex(@location(0) position: vec3<f32>,
#ifdef TERRAIN_MESH
    @builtin(instance_index) instance_index: u32,
    @location(1) vertex_normal: vec3<f32>,
#ifdef ROCK_MESH
    @location(2) contact_index: u32,
#endif
#endif
) -> VertexOutput {
    var out: VertexOutput;
#ifdef TERRAIN_MESH
#ifdef ROCK_MESH
    let world_from_local = get_world_from_local(instance_index);
    let world = (world_from_local * vec4<f32>(position,1.0)).xyz;
    out.contact_scale = clamp(length(world_from_local[1].xyz)*0.12,0.035,0.55);
    out.position = position_world_to_clip(world);
    out.world_position = world;
    out.normal = mesh_normal_local_to_world(vertex_normal,instance_index);
    let contact_offset = get_tag(instance_index);
    out.static_color=vec4<f32>(0.0);out.static_normal=vec4<f32>(0.0,1.0,0.0,0.0);
    if contact_offset != 0u {
        let packed=rock_surfaces[contact_offset+contact_index];
        out.static_color=unpack4x8unorm(packed.x);
        let n=unpack4x8unorm(packed.y);
        out.static_normal=vec4<f32>(n.xyz*2.0-1.0,n.w);
        let contact = rock_contacts[contact_offset + contact_index];
        out.ground_height = contact.w;
        out.ground_normal = contact.xyz;
    } else {
    out.ground_height = terrain_height(world.xz);
    out.ground_normal = normalize(vec3<f32>(
        terrain_height(world.xz - vec2<f32>(0.5,0.0)) - terrain_height(world.xz + vec2<f32>(0.5,0.0)),
        1.0,
        terrain_height(world.xz - vec2<f32>(0.0,0.5)) - terrain_height(world.xz + vec2<f32>(0.0,0.5))));
    }
    out.uv = vec2<f32>(0.0);
#else
    let world_from_local = get_world_from_local(instance_index);
    var world = (world_from_local * vec4<f32>(position,1.0)).xyz;
    var normal = vertex_normal;
    // Only newly streamed tiles use procedural elevation. Cached tiles carry
    // their final heights and normals and skip all noise in both render passes.
    if dot(normal,normal) == 0.0 {
        let p = world.xz;
        let origin = world_from_local[3].xz;
        let cell = vec2<i32>(round(origin / 32.0));
        let wrap = ((cell % vec2<i32>(32)) + vec2<i32>(32)) % vec2<i32>(32);
        let slot = u32(wrap.x + wrap.y * 32);
        let page = surface_pages[48u + slot];
        if page.w > 0.0 && all(page.xy == origin) {
            let local = vec2<u32>(round(position.xz));
            let sample = terrain_samples[slot * 1089u + local.y * 33u + local.x];
            world.y = sample.x;
            normal = sample.yzw;
        } else {
            world.y = terrain_height(p);
            normal = normalize(vec3<f32>(
                terrain_height(p - vec2<f32>(0.5,0.0)) - terrain_height(p + vec2<f32>(0.5,0.0)),
                1.0,
                terrain_height(p - vec2<f32>(0.0,0.5)) - terrain_height(p + vec2<f32>(0.0,0.5))));
        }
    }
    out.position = position_world_to_clip(world);
    out.world_position = world;
    out.normal = normal;
    out.uv = vec2<f32>(0.0);
#endif
#else
    out.position = vec4<f32>(position.xy, 0.0, 1.0);
    out.uv = vec2<f32>(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
#endif
    return out;
}

struct FragmentOutput {
#ifndef PREPASS_PIPELINE
    @location(0) color: vec4<f32>,
#endif
#ifdef PREPASS_PIPELINE
    @builtin(frag_depth) depth: f32,
#else
#ifndef TERRAIN_MESH
    @builtin(frag_depth) depth: f32,
#endif
#endif
};

struct TrackBins {
    heads: array<vec4<u32>,256>,
    next: array<vec4<u32>,128>,
};
struct CutoutBins { bounds:vec4<f32>, goblins:array<vec4<u32>,256>, orcs:array<vec4<u32>,64> };
struct SdfUniforms {
    camera_position: vec4<f32>,
    camera_forward: vec4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
    resolution: vec4<f32>,
    player_position: vec4<f32>,
    world_seed: vec4<u32>,
    tracks: array<vec4<f32>, 512>,
    tracks_meta: vec4<f32>,
    tracks_bounds: vec4<f32>,
    track_bins: TrackBins,
    shadow_quality: vec4<u32>,
    goblin_portals: array<vec4<f32>, 256>,
    goblin_limits: array<vec4<f32>, 128>,
    orc_holes: array<vec4<f32>, 32>,
    cutouts: CutoutBins,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> uniforms: SdfUniforms;

const MAX_STEPS: i32 = 128;
const SURFACE_EPSILON: f32 = 0.001;

struct Surface {
    distance: f32,
    material: f32,
};

fn surface(distance: f32, material: f32) -> Surface {
    return Surface(distance, material);
}

fn closer(a: Surface, b: Surface) -> Surface {
    if a.distance < b.distance {
        return a;
    }
    return b;
}

fn sd_round_box(p: vec3<f32>, half_size: vec3<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec3<f32>(radius);
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - radius;
}

fn sd_torus(p: vec3<f32>, radii: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - radii.x, p.y);
    return length(q) - radii.y;
}

fn sd_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, radius: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - radius;
}

fn sd_capped_cylinder(p: vec3<f32>, half_height: f32, radius: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xz), p.y)) - vec2<f32>(radius, half_height);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn map_scene(world_p: vec3<f32>) -> Surface {
    return surface(world_p.y, 1.0);
}

// Matches goblin_dens::geology; the terrain cut follows the actual rough wall.
fn goblin_lattice(p: vec3<i32>) -> f32 {
    let q = bitcast<vec3<u32>>(p);
    var h = q.x * 0x8da6b343u ^ q.y * 0xd8163841u ^ q.z * 0xcb1ab31fu;
    h ^= h >> 16u; h *= 0x7feb352du; h ^= h >> 15u;
    return f32(h & 0xffffffu) / 16777215.0;
}
fn goblin_noise(p: vec3<f32>) -> f32 {
    let cell = vec3<i32>(floor(p)); let f = fract(p);
    let u = f * f * (vec3<f32>(3.0) - 2.0 * f);
    var value = 0.0;
    for (var x = 0; x < 2; x += 1) {
        for (var y = 0; y < 2; y += 1) {
            for (var z = 0; z < 2; z += 1) {
                let w = select(vec3<f32>(1.0) - u, u, vec3<i32>(x,y,z) == vec3<i32>(1));
                value += goblin_lattice(cell + vec3<i32>(x,y,z)) * w.x * w.y * w.z;
            }
        }
    }
    return value;
}
fn goblin_profile(world: vec3<f32>, seed: f32) -> vec4<f32> {
    let p = world + vec3<f32>(seed);
    return vec4<f32>(1.30 + goblin_noise(p * 0.12) * 0.95,
        1.30 + goblin_noise(p * 0.16 + vec3<f32>(31.0)) * 0.90,
        1.40 + goblin_noise(p * 0.27 + vec3<f32>(87.0)) * 0.80,
        (goblin_noise(p * 2.1) - 0.5) * 0.24 + (goblin_noise(p * 0.8) - 0.5) * 0.24);
}
fn goblin_burrow(v: vec3<f32>, forward: vec3<f32>, profile: vec4<f32>) -> f32 {
    let x = dot(v, cross(vec3<f32>(0.0,1.0,0.0), forward));
    let sides = max(-x - profile.x + v.y * 0.14, x - profile.y - v.y * 0.08);
    let roof = v.y + max(x * 0.43, -x * 0.66) - profile.z;
    return max(max(sides, roof), max(-v.y - 1.68, abs(dot(v, forward)) - 2.25)) + profile.w;
}
fn cutout_cell(p:vec3<f32>)->i32 {
    let q=vec2<i32>(floor((p.xz-uniforms.cutouts.bounds.xy)*uniforms.cutouts.bounds.w));
    if uniforms.cutouts.bounds.w<=0.0 || any(q<vec2<i32>(0)) || any(q>=vec2<i32>(16)) {return -1;}
    return q.y*16+q.x;
}
fn inside_goblin_portal(p: vec3<f32>) -> bool {
    if uniforms.goblin_portals[0].w <= 0.0 { return false; }
    let cell=cutout_cell(p);
    var candidates=vec4<u32>(0xffffffffu);
    if cell>=0 {candidates=uniforms.cutouts.goblins[u32(cell)];}
    for(var word=0u;word<4u;word++) {
    var bits=candidates[word];
    while bits!=0u {
        let i=word*32u+firstTrailingBit(bits); bits=bits&(bits-1u);
        let a = uniforms.goblin_portals[i * 2u];
        if a.w <= 0.0 { break; }
        let b = uniforms.goblin_portals[i * 2u + 1u];
        let delta = b.xyz - a.xyz;
        let t = clamp(dot(p - a.xyz, delta) / dot(delta, delta), 0.0, 1.0);
        let v = p - (a.xyz + delta * t);
        if dot(v,v) < 0.64 { return true; }
        if dot(v,v) < 15.0 {
            let forward = normalize(vec3<f32>(delta.x, 0.0, delta.z));
            if goblin_burrow(v, forward, goblin_profile(p, b.w)) < -0.12 { return true; }
        }
    }
    }
    return false;
}

fn inside_orc_hole(p: vec3<f32>) -> bool {
    if uniforms.orc_holes[0].z <= 0.0 { return false; }
    let cell=cutout_cell(p);
    var bits=0xffffffffu;
    if cell>=0 {bits=uniforms.cutouts.orcs[u32(cell)/4u][u32(cell)%4u];}
    while bits!=0u {
        let i=firstTrailingBit(bits); bits=bits&(bits-1u);
        let hole=uniforms.orc_holes[i];
        if hole.z <= 0.0 { break; }
        let rounding=select(0.6,0.4,hole.w<0.0);
        let q=abs(p.xz-hole.xy)-abs(hole.zw)+vec2<f32>(rounding);
        if length(max(q,vec2<f32>(0.0)))+min(max(q.x,q.y),0.0)<rounding {return true;}
    }
    return false;
}

fn raymarch(ray_origin: vec3<f32>, ray_direction: vec3<f32>, limit: f32) -> Surface {
    // Spawn structures are removed; terrain and vegetation are rasterized.
    return surface(1e20, -1.0);
}

fn scene_normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.0015;
    let a = vec3<f32>(1.0, -1.0, -1.0);
    let b = vec3<f32>(-1.0, -1.0, 1.0);
    let c = vec3<f32>(-1.0, 1.0, -1.0);
    let d = vec3<f32>(1.0, 1.0, 1.0);
    return normalize(
        a * map_scene(p + a * e).distance +
        b * map_scene(p + b * e).distance +
        c * map_scene(p + c * e).distance +
        d * map_scene(p + d * e).distance
    );
}

#ifndef PREPASS_PIPELINE
#import bevy_pbr::{
    pbr_types::pbr_input_new,
    pbr_functions::{hither_pbr_lighting, main_pass_post_lighting_processing},
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    mesh_view_bindings::lights,
}
#endif

fn ambient_occlusion(p: vec3<f32>, normal: vec3<f32>) -> f32 {
    if max(abs(p.x), abs(p.z)) > 6.0 {
        return 1.0;
    }
    var occlusion = 0.0;
    var weight = 1.0;
    let samples = select(select(3,4,uniforms.shadow_quality.x >= 24u),5,uniforms.shadow_quality.x >= 48u);
    for (var i = 1; i <= samples; i += 1) {
        let distance = f32(i) * 0.1;
        occlusion += (distance - map_scene(p + normal * distance).distance) * weight;
        weight *= 0.58;
    }
    return clamp(1.0 - occlusion * 1.4, 0.15, 1.0);
}

// Same seeded value-noise field as src/biome.rs, including negative cells.
fn climate_lattice(p: vec2<i32>, seed: u32) -> f32 {
    let cell = bitcast<vec2<u32>>(p);
    var h = cell.x * 0x9e3779b9u ^ cell.y * 0x85ebca6bu ^ seed;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777215.0;
}
fn climate_noise(p: vec2<f32>, seed: u32) -> f32 {
    let c = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(climate_lattice(c, seed), climate_lattice(c + vec2<i32>(1, 0), seed), u.x),
        mix(climate_lattice(c + vec2<i32>(0, 1), seed), climate_lattice(c + vec2<i32>(1, 1), seed), u.x),
        u.y);
}
fn snow_amount(p: vec2<f32>) -> f32 {
    let q = p / 180.0 + vec2<f32>(0.37, 0.61);
    let s = uniforms.world_seed.x;
    let climate = climate_noise(q, s) * 0.75
        + climate_noise(q * 2.03 + vec2<f32>(19.1, -7.3), s ^ 0xa511e9b3u) * 0.25;
    return smoothstep(0.47, 0.53, climate);
}

fn forest_amount(p: vec2<f32>) -> f32 {
    let q = p / 240.0 + vec2<f32>(8.17, -3.41);
    let s = uniforms.world_seed.x ^ 0x666f7265u;
    let cover = climate_noise(q, s) * 0.78
        + climate_noise(q * 2.71 + vec2<f32>(-9.3,17.8), s ^ 0xa511e9b3u) * 0.22;
    return smoothstep(0.46, 0.54, cover);
}

fn mountain_amount(p: vec2<f32>) -> f32 {
    let s = uniforms.world_seed.x ^ 0x616c7069u;
    let region = climate_noise(p / 700.0 + vec2<f32>(-4.7,9.2), s) * 0.8
        + climate_noise(p / 290.0 + vec2<f32>(7.1,-2.8), s ^ 0xa511e9b3u) * 0.2;
    return smoothstep(0.53,0.73,region) * smoothstep(100.0,280.0,length(p));
}
// Mirror biome::mountain_relief, including the rounded ridges at grid scale.
fn mountain_relief(p: vec2<f32>) -> f32 {
    let s = uniforms.world_seed.x ^ 0x7065616bu;
    let warp = vec2<f32>(climate_noise(p / 310.0,s),
        climate_noise(p / 310.0 + vec2<f32>(37.2,-19.4),s ^ 0xa511e9b3u)) - vec2<f32>(0.5);
    let q = p + warp * 150.0;
    let axis = vec2<f32>(q.x * 0.86 + q.y * 0.51,q.y * 0.86 - q.x * 0.51);
    let main = climate_noise(axis / vec2<f32>(240.0,390.0),s) * 2.0 - 1.0;
    let ridge = 1.0 - sqrt(main * main + 0.0036);
    let spur = climate_noise((axis + vec2<f32>(83.1,-27.7)) / 105.0,s ^ 0x73706972u) * 2.0 - 1.0;
    let secondary = 1.0 - sqrt(spur * spur + 0.0064);
    let ribs = climate_noise(q / 38.0,s ^ 0x72696273u) * 2.0 - 1.0;
    let rib = 1.0 - sqrt(ribs * ribs + 0.0144);
    let character = climate_noise(p / 580.0 + vec2<f32>(17.2,-9.6),s ^ 0x7374796cu);
    let broad = ridge * ridge * (0.65 + 0.35 * ridge);
    let sharp = ridge * ridge * ridge * ridge;
    let mass = mix(broad,sharp,smoothstep(0.30,0.75,character));
    let crown = climate_noise(q / 145.0,s ^ 0x63726f77u);
    let backbone = mass * (135.0 + secondary * 145.0 + crown * 65.0);
    let foothills = ridge * ridge * (rib * 32.0 + secondary * 24.0);
    let gullies = (climate_noise(q / 17.0,s ^ 0x67756c6cu) - 0.5) * 17.0;
    let rubble = (climate_noise(p / 5.0,s ^ 0x73637265u) - 0.5) * 1.5;
    return 5.0 + climate_noise(p / 90.0,s ^ 0x76616c6cu) * 9.0
        + backbone + foothills + ridge * ridge * (gullies + rubble);
}

// Keep in sync with biome::terrain_for_seed and orcs::terrain_clearance.
fn terrain_height(p: vec2<f32>) -> f32 {
    let wooded = forest_amount(p);
    let mountain = mountain_amount(p);
    if wooded == 0.0 && mountain == 0.0 { return 0.0; }
    let s = uniforms.world_seed.x ^ 0x68696c6cu;
    let roll = 3.8 * climate_noise(p / 38.0, s)
        + 0.65 * climate_noise(p / 15.0 + vec2<f32>(7.3), s ^ 0xa511e9b3u);
    let region = smoothstep(0.64, 0.84, climate_noise(p / 310.0 + vec2<f32>(13.7,-8.1), s ^ 0x72617265u));
    let warp = (climate_noise(p / 125.0, s ^ 0x77617270u) - 0.5) * 1.6;
    let across = dot(p, vec2<f32>(0.8,0.6)) / 48.0 + warp;
    let along = dot(p, vec2<f32>(-0.6,0.8)) / 190.0;
    let ridge = climate_noise(vec2<f32>(across,along), s ^ 0x72696467u);
    let hills = 28.0 * region * ridge * ridge;
    var clearing = smoothstep(12.0, 52.0, length(p));
    for (var i = 0u; i < 32u; i += 1u) {
        let hole = uniforms.orc_holes[i];
        if hole.z <= 0.0 { break; }
        clearing *= smoothstep(18.0, 44.0, distance(p, hole.xy));
    }
    var elevation = wooded * (roll + hills);
    if mountain > 0.0 { elevation = mix(elevation,mountain_relief(p),mountain); }
    return elevation * clearing;
}

// Same stand mask used by tree placement and CPU grass: glades grow grass,
// dense stands collect litter. No unrelated checkerboard habitat boundaries.
fn stand_density(p: vec2<f32>) -> f32 {
    let seed = uniforms.world_seed.x ^ 0x7374616eu;
    let clearing = climate_noise(p / 28.0 + vec2<f32>(5.7,-3.1), seed);
    return (1.0 - smoothstep(0.60,0.75,clearing))
        * (0.82 + 0.18 * climate_noise(p / 9.0, seed ^ 0xa511e9b3u));
}

fn ground_hash(p: vec2<f32>) -> f32 {
    let q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    let r = q + dot(q, q.yzx + vec3<f32>(33.33));
    return fract((r.x + r.y) * r.z);
}

// Same coherent coverage mask as biome::ground_snow, used to bury CPU blades.
fn ground_snow(p: vec2<f32>, climate: f32) -> f32 {
    if climate <= 0.0 || climate >= 1.0 { return climate; }
    let seed = uniforms.world_seed.x ^ 0x736e6f77u;
    let coverage_noise = clamp(climate_noise(p * 0.7,seed) * 0.7
        + climate_noise(p * 2.1 + vec2<f32>(13.7),seed) * 0.3,0.20,0.80);
    return smoothstep(coverage_noise-0.12,coverage_noise+0.12,climate);
}

fn ground_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(ground_hash(cell), ground_hash(cell + vec2<f32>(1.0, 0.0)), u.x),
        mix(ground_hash(cell + vec2<f32>(0.0, 1.0)), ground_hash(cell + vec2<f32>(1.0)), u.x), u.y);
}

// Compact-support simplex kernels overlap smoothly, unlike square value-noise
// derivatives which visibly flatten along every lattice row. Return height and
// its exact gradient together so powder color and lighting share one surface.
fn powder_kernel(cell: vec2<i32>, offset: vec2<f32>) -> vec3<f32> {
    let angle = climate_lattice(cell, 0x706f7764u) * 6.2831853;
    let g = vec2<f32>(cos(angle), sin(angle));
    let t = max(0.5-dot(offset,offset),0.0);
    let projection = dot(g,offset);
    let t3 = t*t*t;
    let t4 = t3*t;
    return vec3<f32>(t4*projection,t4*g-8.0*t3*projection*offset)*70.0;
}
fn powder_field(p: vec2<f32>) -> vec3<f32> {
    let cell = floor(p + (p.x+p.y)*0.3660254);
    let a = p-cell+(cell.x+cell.y)*0.21132487;
    let step = select(vec2<f32>(0.0,1.0),vec2<f32>(1.0,0.0),a.x>a.y);
    return powder_kernel(vec2<i32>(cell),a)
        + powder_kernel(vec2<i32>(cell+step),a-step+0.21132487)
        + powder_kernel(vec2<i32>(cell+1.0),a-1.0+0.42264974);
}

fn sole_ellipse(p: vec2<f32>, r: vec2<f32>) -> f32 {
    return (length(p / r) - 1.0) * min(r.x,r.y);
}
fn sole_union(a: f32, b: f32) -> f32 {
    let h = max(0.014 - abs(a-b),0.0) / 0.014;
    return min(a,b) - h*h*0.0035;
}
// Hiking-boot silhouette: broad rounded toe, medial arch, narrow separate heel.
// About 28 cm long and 11 cm wide, following the bundled shoes02 boot geometry.
fn boot_distance(q: vec2<f32>) -> f32 {
    let toe = sole_ellipse(q-vec2<f32>(0.004,0.055),vec2<f32>(0.054,0.102));
    let waist = sole_ellipse(q-vec2<f32>(-0.009,-0.016),vec2<f32>(0.036,0.075));
    let heel = sole_ellipse(q-vec2<f32>(-0.004,-0.088),vec2<f32>(0.039,0.050));
    return sole_union(sole_union(toe,waist),heel);
}

fn orc_foot_distance(q: vec2<f32>) -> f32 {
    var d=sole_ellipse(q-vec2<f32>(0.0,-0.048),vec2<f32>(0.051,0.092));
    d=sole_union(d,sole_ellipse(q-vec2<f32>(0.0,0.035),vec2<f32>(0.073,0.075)));
    for(var i=0u;i<5u;i+=1u){
        let t=f32(i);
        let center=vec2<f32>(-0.052+t*0.026,0.108-t*0.006);
        d=min(d,sole_ellipse(q-center,vec2<f32>(0.018-t*0.0017,0.027-t*0.002)));
        // Thin forward furrows made by the overgrown claws.
        d=min(d,sole_ellipse(q-center-vec2<f32>(0.0,0.033),vec2<f32>(0.005,0.025-t*0.002)));
    }
    return d;
}

fn footprint_distance(q: vec2<f32>, orc: bool) -> f32 {
    if orc {return orc_foot_distance(q);}
    return boot_distance(q);
}

// Compaction, irregular pressure walls and churned powder, not stamped rectangles.
fn tracks_shading(p: vec2<f32>) -> vec2<f32> {
    if uniforms.tracks_meta.x < 1.0 ||
        any(p < uniforms.tracks_bounds.xy) || any(p > uniforms.tracks_bounds.zw) {
        return vec2<f32>(0.0);
    }
    var shade = vec2<f32>(0.0);
    let lo = vec2<i32>(floor(p - vec2<f32>(0.3)));
    let hi = vec2<i32>(floor(p + vec2<f32>(0.3)));
    for (var y = lo.y; y <= hi.y; y += 1) {
    for (var x = lo.x; x <= hi.x; x += 1) {
    let bucket = ((bitcast<u32>(x) * 0x9e3779b9u) ^ (bitcast<u32>(y) * 0x85ebca6bu)) & 1023u;
    var link = uniforms.track_bins.heads[bucket / 4u][bucket % 4u];
    for (var visited = 0u; visited < 512u && link != 0xffffffffu; visited += 1u) {
        let i = link;
        link = uniforms.track_bins.next[i / 4u][i % 4u];
        let foot = uniforms.tracks[i];
        let delta = p - foot.xy;
        if dot(delta, delta) > 0.08 { continue; }
        let c = cos(foot.z);
        let s = sin(foot.z);
        let orc = foot.z >= 12.5663706;
        let facing=foot.z-select(0.0,12.5663706,orc);
        let handed = select(1.0,-1.0,facing >= 6.2831853);
        let q = vec2<f32>(dot(delta,vec2<f32>(c,s))*handed, dot(delta,vec2<f32>(s,-c)));
        let grain = ground_noise(p * 230.0);
        let clumps = ground_noise(p * 71.0 + foot.xy * 3.7);
        let churn = ground_noise(p * 34.0);
        let d = footprint_distance(q,orc) + (clumps-0.5)*0.008;
        let fade = 1.0 - smoothstep(27.0,30.0,foot.w);
        let sole = 1.0 - smoothstep(-0.008,0.006,d);
        // Chevron lugs, a heel block and a flex groove, partially filled by snow.
        let lug = smoothstep(0.2,0.65,cos((q.y + abs(q.x)*0.52)*205.0));
        let split = smoothstep(0.003,0.009,abs(q.x));
        let heel_block = (1.0-smoothstep(-0.068,-0.055,q.y));
        let pressure = 0.55 + 0.25*heel_block + 0.20*smoothstep(0.0,0.07,q.y);
        let tread = select(lug * split * smoothstep(0.22,0.60,clumps),0.0,orc);
        let packed = (0.32 + churn*0.22 + grain*0.12 + tread*0.18)*pressure;
        // Side-wall shading follows the light, giving a sunken rather than painted sole.
        let grad = normalize(vec2<f32>(
            footprint_distance(q+vec2<f32>(0.002,0.0),orc)-footprint_distance(q-vec2<f32>(0.002,0.0),orc),
            footprint_distance(q+vec2<f32>(0.0,0.002),orc)-footprint_distance(q-vec2<f32>(0.0,0.002),orc)) + vec2<f32>(0.00001));
        let local_light = vec2<f32>((-0.55*c+0.42*s)*handed, -0.55*s-0.42*c);
        let slope = dot(grad,local_light);
        // WGSL pow has an undefined result for a negative base, even when
        // the exponent is 2. These signed distances must be squared directly.
        let wall_distance = (d+0.003)/0.007;
        let lip_distance = (d-0.012-(churn-0.5)*0.012)/0.013;
        let wall = exp(-wall_distance*wall_distance);
        let lip = exp(-lip_distance*lip_distance)
            * (0.25+clumps*0.75) * smoothstep(0.14,0.48,grain);
        let dark = sole*packed + wall*max(slope,0.0)*0.65;
        let light = lip*(0.35+max(-slope,0.0)) + wall*max(-slope,0.0)*0.22;
        shade = max(shade,vec2<f32>(dark,light)*fade);
    }
    }
    }
    return shade;
}

// Mirror biome::surface_snow_at and the snow-tree cutoff, including foothills.
const MOUNTAIN_SNOW_MIN_Y: f32 = 200.0;
// Horizontal, low-frequency fields can be cached even on close terrain and rocks.
// Snow patches, altitude, slope retention and all fine surface detail stay live.
fn surface_climate(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(snow_amount(p), mountain_amount(p),
        forest_amount(p)*stand_density(p),
        climate_noise(p/31.0,uniforms.world_seed.x ^ 0x736e6c6eu));
}
fn surface_snow(p: vec3<f32>, normal: vec3<f32>) -> f32 {
    return climate_snow(p,normal,surface_climate(p.xz));
}
fn climate_snow(p: vec3<f32>, normal: vec3<f32>, fields: vec4<f32>) -> f32 {
    let climate = fields.x;
    let mountain = fields.y;
    if mountain == 0.0 { return ground_snow(p.xz,climate); }
    if !(p.y > MOUNTAIN_SNOW_MIN_Y) { return 0.0; }
    let snowline = max(MOUNTAIN_SNOW_MIN_Y,MOUNTAIN_SNOW_MIN_Y + fields.w * 38.0 - normal.z * 14.0);
    let alpine = max(climate,smoothstep(snowline,snowline + 25.0,p.y))
        * smoothstep(MOUNTAIN_SNOW_MIN_Y,MOUNTAIN_SNOW_MIN_Y + 25.0,p.y);
    let accumulation = mix(climate,alpine,smoothstep(0.0,1.0,mountain));
    let retention = mix(1.0,smoothstep(0.58,0.88,normal.y),smoothstep(0.02,0.20,mountain));
    return ground_snow(p.xz,accumulation) * retention;
}

// Ground shading range only: leave geometry, vegetation and snow masks alone.
// Evaluate visibility at half the distance/footprint for a 2x detail range.
const GROUND_DETAIL_RANGE: f32 = 2.0;
fn ground_detail_footprint(footprint:f32)->f32 { return footprint/GROUND_DETAIL_RANGE; }

// Cellular fracture planes: each outcrop has its own facet, recessed joint,
// and chipped edge. The derivative is analytic and drives real lighting.
fn fracture_plane(p: vec2<f32>) -> vec4<f32> {
    let cell = floor(p);
    var nearest = 100.0;
    var second = 100.0;
    var delta = vec2<f32>(0.0);
    var other = vec2<f32>(0.0);
    var mineral = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = cell + vec2<f32>(f32(x),f32(y));
            let jitter = vec2<f32>(climate_lattice(vec2<i32>(c),0x726f636bu),
                climate_lattice(vec2<i32>(c),0x6372616bu));
            let d = p - c - (0.18 + jitter * 0.64);
            let distance = dot(d,d);
            if distance < nearest {
                second = nearest;
                other = delta;
                nearest = distance;
                delta = d;
                mineral = climate_lattice(vec2<i32>(c),0x6d696e65u);
            } else if distance < second {
                second = distance;
                other = d;
            }
        }
    }
    let edge = max(second-nearest,0.0);
    let joint = (1.0 - smoothstep(0.015,0.12,edge)) * smoothstep(0.18,0.48,mineral);
    let bevel = (1.0-smoothstep(0.02,0.22,edge)) * 2.4;
    let gradient = (delta-other) * bevel + delta * (0.1 + mineral * 0.2);
    return vec4<f32>(mineral,joint,gradient);
}
struct MountainSurface {
    color: vec3<f32>,
    normal: vec3<f32>,
}
// Filter each octave by its projected pixel footprint, independent of the
// geometry detail radius. Zero-mean detail converges to its average, never mud.
fn surface_band(p: vec2<f32>, frequency: f32, footprint: f32) -> vec3<f32> {
    let weight = 1.0-smoothstep(0.20,0.85,footprint*frequency);
    if weight <= 0.0 { return vec3<f32>(0.0); }
    return powder_field(p * frequency) * weight;
}
fn mountain_surface(p: vec3<f32>, normal: vec3<f32>, bedrock: f32, footprint: f32) -> MountainSurface {
    return mountain_surface_snow(p,normal,bedrock,footprint,surface_snow(p,normal));
}
fn mountain_surface_snow(p: vec3<f32>, normal: vec3<f32>, bedrock: f32, pixel_footprint: f32, cover: f32) -> MountainSurface {
    let footprint=ground_detail_footprint(pixel_footprint);
    let w = pow(abs(normal),vec3<f32>(4.0));
    let weights = w / max(dot(w,vec3<f32>(1.0)),0.001);
    let detail = 1.0-smoothstep(0.20,0.85,footprint * 5.5);
    let grain_weight = 1.0-smoothstep(0.20,0.85,footprint*13.0);
    var grain = 0.5;
    if grain_weight > 0.0 || cover > 0.0 {
        var gx = 0.0; var gy = 0.0; var gz = 0.0;
        if weights.x > 0.0 { gx = ground_noise(p.yz * 13.0); }
        if weights.y > 0.0 { gy = ground_noise(p.xz * 13.0); }
        if weights.z > 0.0 { gz = ground_noise(p.xy * 13.0); }
        grain = gx * weights.x + gy * weights.y + gz * weights.z;
    }
    // A fully covered surface uses only the snow color and geometric normal.
    // Keep the original grain and normalization, without evaluating buried rock.
    if cover == 1.0 {
        return snow_micro_surface(p,normal,pixel_footprint);
    }
    let fracture_slope = 1.0-smoothstep(0.82,0.97,normal.y);
    var fractures = 0.0;
    if fracture_slope > 0.0 {
    let jointed_range = smoothstep(0.62,0.80,climate_noise(p.xz / 420.0,
        uniforms.world_seed.x ^ 0x6a6f696eu));
    var ox = 0.0; var oy = 0.0; var oz = 0.0;
    if weights.x > 0.0 { ox = ground_noise(p.yz / 23.0); }
    if weights.y > 0.0 { oy = ground_noise(p.xz / 23.0); }
    if weights.z > 0.0 { oz = ground_noise(p.xy / 23.0); }
    let outcrop_field = ox * weights.x + oy * weights.y + oz * weights.z;
    let outcrop = smoothstep(0.58,0.76,outcrop_field);
    fractures = max(jointed_range * 0.9,outcrop) * fracture_slope;
    }
    // Metre-scale blocks remain legible across the valley; smaller gravel and
    // mineral grain resolve as the player approaches. No vertical UV stretching.
    let scale = 0.34;
    var warp = vec3<f32>(0.0);
    if weights.y > 0.0 || weights.z > 0.0 { warp.x = ground_noise(p.yz * 0.21); }
    if weights.x > 0.0 || weights.z > 0.0 { warp.y = ground_noise(p.xz * 0.19); }
    if weights.x > 0.0 || weights.y > 0.0 { warp.z = ground_noise(p.xy * 0.23); }
    let fractured = p * vec3<f32>(1.0,1.7,1.0) + warp * 2.7;
    var fx = vec4<f32>(0.0);
    var fy = vec4<f32>(0.0);
    var fz = vec4<f32>(0.0);
    if fractures > 0.0 && weights.x > 0.01 { fx = fracture_plane(fractured.yz * scale); }
    if fractures > 0.0 && weights.y > 0.01 { fy = fracture_plane(fractured.xz * scale); }
    if fractures > 0.0 && weights.z > 0.01 { fz = fracture_plane(fractured.xy * scale); }
    let blocks = fx.x * weights.x + fy.x * weights.y + fz.x * weights.z;
    let joints = fx.y * weights.x + fy.y * weights.y + fz.y * weights.z;
    var stone_x = vec3<f32>(0.0);
    if weights.x > 0.0 { stone_x = surface_band(fractured.yz,0.65,footprint*1.7); }
    var stone_y = vec3<f32>(0.0);
    if weights.y > 0.0 { stone_y = surface_band(fractured.xz,0.65,footprint*1.7); }
    var stone_z = vec3<f32>(0.0);
    if weights.z > 0.0 { stone_z = surface_band(fractured.xy,0.65,footprint*1.7); }
    let stone_relief = stone_x.x*weights.x + stone_y.x*weights.y + stone_z.x*weights.z;
    var cx = 0.0; var cy = 0.0; var cz = 0.0;
    if weights.x > 0.0 { cx = powder_field(p.yz * 0.065).x; }
    if weights.y > 0.0 { cy = powder_field(p.xz * 0.065).x; }
    if weights.z > 0.0 { cz = powder_field(p.xy * 0.065).x; }
    let coarse = 0.5 + 0.34*(cx * weights.x + cy * weights.y + cz * weights.z);
    let geology = ground_noise(p.xz / 63.0 + p.y * 0.003);
    // Most faces weather into continuous stone. Only local outcrops and a few
    // coherent geological regions expose strongly jointed bedrock.
    // Tilted, broken sediment beds and ochre weathering, rather than regular
    // sinusoidal contour stripes. Pale mineral seams cut through darker stone.
    let bed = ground_noise(vec2<f32>(p.x * 0.021 + p.z * 0.017,p.y * 0.32 + p.x * 0.09));
    var rock = mix(vec3<f32>(0.22,0.245,0.26),vec3<f32>(0.34,0.30,0.235),geology);
    rock *= 0.78 + mix(0.5,blocks,fractures) * 0.18 + coarse * 0.32 + stone_relief * 0.075;
    rock = mix(rock,vec3<f32>(0.58,0.55,0.46),smoothstep(0.74,0.86,bed)*smoothstep(0.45,0.72,coarse)*0.24);
    let weathered = smoothstep(0.22,0.72,coarse);
    rock *= (1.0-joints*0.32*weathered*fractures*(1.0-smoothstep(0.15,0.65,footprint*scale))) * (1.0 + (grain-0.5) * 0.40 * grain_weight);
    let soil = mix(vec3<f32>(0.105,0.063,0.029),vec3<f32>(0.27,0.19,0.105),coarse);
    var gravel = vec4<f32>(0.5,0.0,0.0,0.0);
    if detail > 0.0 && normal.y > 0.65 { gravel = fracture_plane(p.xz * 5.5); }
    let stones = mix(0.65,smoothstep(0.32,0.60,gravel.x),detail);
    let micro_weight=1.0-smoothstep(0.018,0.09,footprint);
    var rock_micro=GroundMicro(vec3<f32>(1.0),vec3<f32>(0.0));
    var scree_micro=GroundMicro(vec3<f32>(1.0),vec3<f32>(0.0));
    if micro_weight>0.0 {
        rock_micro=stone_micro_surface(p,normal,pixel_footprint);
        rock*=mix(vec3<f32>(1.0),rock_micro.color,micro_weight);
        if bedrock<1.0 && normal.y>0.65 {
            scree_micro=litter_micro_surface(p.xz,pixel_footprint,0.0);
        }
    }
    let scree = mix(soil,rock * (0.8 + gravel.x * 0.5),stones)
        *mix(vec3<f32>(1.0),scree_micro.color,micro_weight);
    // Broad ecological patches, medium tussocks and fine turf all survive
    // beyond blade range. Rotated, warped simplex fields avoid lattice bands.
    var broad = vec3<f32>(0.0);
    var patches = vec3<f32>(0.0);
    var tussocks = vec3<f32>(0.0);
    var turf = vec3<f32>(0.0);
    if bedrock < 1.0 {
        let meadow_warp = vec2<f32>(powder_field(p.xz * 0.013).x,
            powder_field(p.xz * 0.013 + vec2<f32>(31.7,-19.3)).x) * 9.0;
        let q = p.xz + meadow_warp;
        broad = surface_band(q,0.035,footprint);
        patches = surface_band(vec2<f32>(q.x*0.8-q.y*0.6,q.x*0.6+q.y*0.8),0.19,footprint);
        tussocks = surface_band(q,1.4,footprint);
        turf = surface_band(q,8.0,footprint);
    }
    let moisture = clamp(0.52 + broad.x*0.40 + patches.x*0.13,0.0,1.0);
    var meadow = mix(vec3<f32>(0.105,0.125,0.038),vec3<f32>(0.037,0.081,0.025),moisture);
    meadow *= 1.0 + patches.x*0.16 + tussocks.x*0.19 + turf.x*0.10;
    let dry = smoothstep(0.35,0.72,patches.x + broad.x*0.35);
    meadow = mix(meadow,vec3<f32>(0.16,0.145,0.067),dry*0.32);
    // Rock emerges on steep faces and in broken talus pockets. Altitude
    // raises the likelihood gradually; it does not paint a horizontal band.
    let exposure = (1.0-normal.y) + broad.x*0.055 + patches.x*0.035;
    let cliff = smoothstep(0.15,0.40,exposure);
    let highland = smoothstep(95.0,220.0,p.y + broad.x*32.0);
    let talus = smoothstep(0.12,0.32,exposure) * smoothstep(-0.1,0.55,patches.x)
        * (0.25 + highland*0.65);
    let meadow_cover = (1.0-bedrock)*(1.0-cliff)*(1.0-highland*0.78);
    var meadow_gradient=vec3<f32>(0.0);
    let meadow_detail=(1.0-smoothstep(0.008,0.035,footprint))*(1.0-cover);
    if meadow_detail*meadow_cover*(1.0-talus)>0.001 {
        let blades=turf_detail(p.xz,pixel_footprint);
        // Short alpine turf retains the meadow's cooler greens and dry patches.
        meadow*=mix(vec3<f32>(1.0),blades.color*vec3<f32>(1.30,1.36,1.21),meadow_detail);
        meadow_gradient=vec3<f32>(blades.gradient.x,0.0,blades.gradient.y)*meadow_detail;
    }
    var surface = mix(rock,mix(meadow,scree,talus),meadow_cover);
    let lichen = bedrock * (1.0-smoothstep(80.0,145.0,p.y)) * smoothstep(0.35,0.85,normal.y)
        * smoothstep(0.58,0.78,ground_noise(p.xz*1.3+p.y*0.3));
    surface = mix(surface,vec3<f32>(0.15,0.17,0.075),lichen*0.18);
    var powder=MountainSurface(vec3<f32>(0.66,0.73,0.79),normal);
    if cover>0.0 {powder=snow_micro_surface(p,normal,pixel_footprint);}
    surface=mix(surface,powder.color,cover);
    let gradient = vec3<f32>(0.0,fx.z,fx.w) * weights.x
        + vec3<f32>(fy.z,0.0,fy.w) * weights.y
        + vec3<f32>(fz.z,fz.w,0.0) * weights.z;
    let tangent = gradient - normal * dot(normal,gradient);
    let bump = (1.0-cover) * (1.0-smoothstep(0.20,0.85,footprint*1.7));
    var grain_tangent = vec3<f32>(0.0);
    if bump > 0.0 {
        let grain_vector = vec3<f32>(ground_noise(p.yz * 1.7),ground_noise(p.xz * 1.7),
            ground_noise(p.xy * 1.7)) - vec3<f32>(0.5);
        grain_tangent = grain_vector - normal * dot(normal,grain_vector);
    }
    let stone_gradient = vec3<f32>(0.0,stone_x.y,stone_x.z)*weights.x
        + vec3<f32>(stone_y.y,0.0,stone_y.z)*weights.y
        + vec3<f32>(stone_z.y,stone_z.z,0.0)*weights.z;
    let stone_tangent = stone_gradient-normal*dot(normal,stone_gradient);
    let micro_gradient=mix(rock_micro.gradient,mix(meadow_gradient,scree_micro.gradient,talus),meadow_cover)
        *micro_weight*(1.0-cover);
    let micro_tangent=micro_gradient-normal*dot(normal,micro_gradient);
    return MountainSurface(surface,normalize(normal + (powder.normal-normal)*cover - micro_tangent - tangent * 0.23 * bump * fractures * (0.25 + weathered * 0.75)
        + grain_tangent * 0.12 * bump - stone_tangent * 0.018 * (1.0-cover) * (1.0-meadow_cover)
        - (vec3<f32>(tussocks.y,0.0,tussocks.z)
            - normal*dot(normal,vec3<f32>(tussocks.y,0.0,tussocks.z))) * 0.045 * meadow_cover * (1.0-cover)));
}

fn ground_wear(p:vec2<f32>,mottling:f32)->f32 {
    let gate = (1.0 - smoothstep(0.30, 0.85, abs(p.x + (mottling - 0.5) * 0.25)))
        * smoothstep(0.5, 1.2, p.y) * (1.0 - smoothstep(6.0, 8.0, p.y));
    let roots = 1.0 - smoothstep(0.35, 0.80, length(p));
    let ring = (1.0 - smoothstep(0.10, 0.30, abs(length(p) - 1.10))) * 0.35;
    return max(roots, max(gate * 0.62, ring));
}
fn material_color(id: f32, p: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let fields=surface_climate(p.xz);
    return material_color_climate(id,p,normal,climate_snow(p,normal,fields),fields.z,0.125);
}
fn material_color_climate(id: f32, p: vec3<f32>, normal: vec3<f32>, snow: f32, forest: f32, pixel_footprint: f32) -> vec3<f32> {
    if id < 1.5 {
        // Isotropic, overlapping clumps avoid the square lattice showing through
        // the turf. Shared with the distant surface-cache bake.
        let broad = clamp(0.5 + powder_field(p.xz * 0.28).x * 0.65,0.0,1.0);
        let mottling = clamp(0.5 + powder_field(p.xz * 2.3 + broad * 0.7).x * 0.6,0.0,1.0);
        let tufts = clamp(0.5 + powder_field(p.xz * 7.1).x * 0.6,0.0,1.0);
        var grass = mix(vec3<f32>(0.018,0.034,0.008),vec3<f32>(0.060,0.079,0.021),broad);
        grass *= 0.64 + mottling * 0.48 + tufts * 0.28;
        // Short directional fibres and earth granules, filtered at distance to
        // avoid shimmer. These carry the lawn beyond the nearby 3D blades.
        let distance = length(p - uniforms.camera_position.xyz);
        let fine = 1.0 - smoothstep(1.5, 6.0, distance/GROUND_DETAIL_RANGE);
        var fibres = 0.5;
        var grains = 0.5;
        var litter_detail = 0.0;
        if fine > 0.0 {
            fibres = ground_noise(vec2<f32>(p.x * 75.0 + p.z * 21.0, p.z * 48.0 - p.x * 17.0));
            grains = ground_noise(p.xz * 150.0);
            litter_detail = ground_noise(p.xz * 25.0);
        }
        grass *= 1.0 + (fibres - 0.5) * fine * 0.5;
        let outside = smoothstep(5.8, 7.0, max(abs(p.x),abs(p.z)));
        let woodland = forest * outside;
        let litter = mix(vec3<f32>(0.035,0.027,0.014),vec3<f32>(0.10,0.060,0.022),mottling)
            * (0.80 + litter_detail*fine*0.40);
        grass = mix(grass,litter,woodland*0.78);
        let worn = ground_wear(p.xz,mottling);
        let soil = vec3<f32>(0.067, 0.048, 0.025) * (0.8 + mottling * 0.4 + (grains - 0.5) * fine * 0.35);
        let climate = snow;
        if climate <= 0.0 { return mix(grass, soil, worn); }
        let powder=snow_micro_surface(p,normal,pixel_footprint).color;
        let dormant = vec3<f32>(0.078,0.063,0.041) * (0.80+mottling*0.40);
        let turf = mix(grass,dormant,clamp(climate/0.08,0.0,1.0));
        return mix(mix(turf, soil, worn), powder, climate);
    }
    if id < 7.5 {
        let stone_variation = 0.04 * sin(floor(p.x * 1.7) * 2.3 + floor(p.y * 2.0) * 4.1 + floor(p.z * 1.7));
        return vec3<f32>(0.34, 0.30, 0.25) + vec3<f32>(stone_variation);
    }
    return vec3<f32>(0.34, 0.30, 0.25);
}

// Layered meadow floor at three physical scales. The finest layer fades
// independently with pixel footprint; blades and broad leaves carry their own
// oriented fold gradients, so lighting follows the leaf rather than a fixed axis.
struct TurfDetail { color:vec3<f32>, gradient:vec2<f32> }
fn turf_detail(p:vec2<f32>, footprint:f32) -> TurfDetail {
    let earth=clamp(0.5+powder_field(p*83.0).x*0.65,0.0,1.0);
    let fine=1.0-smoothstep(0.002,0.009,ground_detail_footprint(footprint));
    let moss=clamp(0.5+powder_field(p*11.0).x*0.7,0.0,1.0);
    var color=mix(vec3<f32>(0.32,0.29,0.18),vec3<f32>(0.46,0.51,0.29),moss);
    color*=1.0+(earth-0.5)*fine*0.45;
    var gradient=vec2<f32>(0.0);
    for(var layer=0;layer<3;layer++) {
        let scale=select(select(13.0,25.0,layer==1),44.0,layer==2);
        let resolved=1.0-smoothstep(0.22,0.70,ground_detail_footprint(footprint)*scale);
        if resolved<0.01 {continue;}
        let q=p*scale+vec2<f32>(f32(layer)*31.73);
        let cell=floor(q);
        let local=fract(q);
        let aa=clamp(footprint*scale*0.55,0.012,0.48);
        for(var y=-1;y<=1;y++) { for(var x=-1;x<=1;x++) {
            let offset=vec2<f32>(f32(x),f32(y));
            let id=cell+offset;
            let seed=ground_hash(id);
            let variation=ground_hash(id+91.7);
            let angle=seed*6.2831853;
            let axis=vec2<f32>(cos(angle),sin(angle));
            let side=vec2<f32>(-axis.y,axis.x);
            let v=local-offset-vec2<f32>(ground_hash(id+17.3),ground_hash(id+53.9));
            let along=dot(v,axis);
            let half_length=0.36+variation*0.44;
            // Outside this support the longitudinal mask is exactly zero.
            // Rosettes have their own three-leaf support and must still run.
            let rosette=layer==0 && variation>0.79;
            if !rosette && abs(along)>=half_length { continue; }
            let width=0.065+ground_hash(id+27.4)*0.10;
            let bend=(seed-0.5)*0.85;
            let cross=dot(v,side)+along*along*bend;
            let taper=max(0.0,1.0-abs(along)/half_length);
            let edge=width*sqrt(taper);
            var blade=(1.0-smoothstep(edge-aa,edge+aa,abs(cross)))
                *(1.0-smoothstep(half_length-0.12,half_length,abs(along)));
            if !rosette && blade<=0.0 { continue; }
            let dry=smoothstep(0.70,0.95,variation);
            var tint=mix(vec3<f32>(0.47,0.69,0.32),vec3<f32>(0.94,1.08,0.56),seed);
            tint=mix(tint,vec3<f32>(1.18,0.93,0.55),dry*0.78);
            let rib=1.0-smoothstep(0.008,0.018+aa,abs(cross));
            let fold=clamp(cross/max(edge,0.02),-1.0,1.0);
            tint*=0.83+rib*0.21+fold*0.13;
            var leaf_gradient=(side+axis*2.0*along*bend)*fold*0.24;
            // Occasional three-leaf rosettes break up the grass-only silhouette.
            // A shallow crease and pale crescent give each leaflet internal detail.
            if rosette {
                blade=0.0;
                for(var leaf=0;leaf<3;leaf++) {
                    let turn=angle+f32(leaf)*2.0943951;
                    let direction=vec2<f32>(cos(turn),sin(turn));
                    let lateral=vec2<f32>(-direction.y,direction.x);
                    let dv=v-direction*0.19;
                    let uv=vec2<f32>(dot(dv,lateral)/0.18,dot(dv,direction)/0.24);
                    let radius=length(uv);
                    let mask=1.0-smoothstep(0.88-aa*4.0,1.0+aa*4.0,radius);
                    let vein=1.0-smoothstep(0.025,0.10+aa*3.0,abs(uv.x));
                    let crescent=(1.0-smoothstep(0.045,0.11+aa*2.0,abs(uv.y+0.20+abs(uv.x)*0.4)));
                    let leaf_color=mix(vec3<f32>(0.40,0.67,0.34),vec3<f32>(0.79,1.02,0.57),seed)
                        *(0.83+vein*0.17+crescent*0.16-uv.x*0.10);
                    tint=mix(tint,leaf_color,mask);
                    leaf_gradient=mix(leaf_gradient,lateral*uv.x*0.30+direction*uv.y*0.12,mask);
                    blade=max(blade,mask);
                }
            }
            let alpha=blade*resolved*0.94;
            color=mix(color,tint,alpha);
            gradient=mix(gradient,leaf_gradient,alpha);
        }}
    }
    return TurfDetail(color,gradient);
}

// Surface-specific detail shared by the visible pass and static surface baker.
struct GroundMicro { color:vec3<f32>, gradient:vec3<f32> }
fn snow_micro_surface(p:vec3<f32>, normal:vec3<f32>, pixel_footprint:f32)->MountainSurface {
    let footprint=ground_detail_footprint(pixel_footprint);
    let wind=vec2<f32>(p.x*0.82+p.z*0.57,-p.x*0.57+p.z*0.82);
    let drift=surface_band(wind*vec2<f32>(0.36,1.0),1.3,footprint);
    let crust=surface_band(p.xz+vec2<f32>(31.7,-13.9),9.0,footprint);
    let granules=surface_band(p.xz,43.0,footprint);
    let crystals=surface_band(p.xz+vec2<f32>(19.3),137.0,footprint);
    let hollows=smoothstep(0.10,0.65,-drift.x);
    let sparkle=smoothstep(0.38,0.65,crystals.x)*(1.0-smoothstep(0.0015,0.004,pixel_footprint));
    var color=mix(vec3<f32>(0.69,0.76,0.82),vec3<f32>(0.52,0.63,0.73),hollows*0.32);
    color*=1.0+drift.x*0.055+crust.x*0.065+granules.x*0.055;
    color+=vec3<f32>(0.09,0.085,0.065)*sparkle;
    let wind_gradient=vec2<f32>(drift.y*0.36*0.82-drift.z*0.57,drift.y*0.36*0.57+drift.z*0.82);
    let gradient=vec3<f32>(wind_gradient.x,0.0,wind_gradient.y)*0.10
        +vec3<f32>(crust.y,0.0,crust.z)*0.060+vec3<f32>(granules.y,0.0,granules.z)*0.024;
    return MountainSurface(color,normalize(normal-(gradient-normal*dot(normal,gradient))));
}
fn stone_micro_plane(p:vec2<f32>, footprint:f32)->vec4<f32> {
    let resolved=1.0-smoothstep(0.20,0.85,footprint*6.0);
    let weathering=surface_band(p,1.7,footprint);
    let fractured=smoothstep(0.02,0.45,weathering.x);
    var facets=vec4<f32>(0.5,0.0,0.0,0.0);
    if resolved*fractured>0.0 {
        let warp=vec2<f32>(weathering.x,powder_field(p*1.3+vec2<f32>(17.8)).x);
        facets=fracture_plane(p*6.0+warp*0.65);
    }
    let chips=surface_band(p,23.0,footprint);
    let grains=surface_band(p,87.0,footprint);
    // Angular mineral faces and recessed joints, with restrained micro normals.
    // Strong high-frequency noise normals make stone resemble curled fibres.
    let joints=facets.y*resolved*fractured;
    let value=0.91+(facets.x-0.5)*0.10*resolved*fractured+chips.x*0.18+grains.x*0.16-joints*0.30;
    let quartz=smoothstep(0.73,0.92,facets.x)*resolved*(0.4+chips.x*0.25);
    let gradient=facets.zw*0.018*resolved*fractured+chips.yz*0.008+grains.yz*0.003;
    return vec4<f32>(value,gradient,quartz);
}
fn stone_micro_surface(p:vec3<f32>, normal:vec3<f32>, pixel_footprint:f32)->GroundMicro {
    let footprint=ground_detail_footprint(pixel_footprint);
    let w=pow(abs(normal),vec3<f32>(4.0));
    let weights=w/max(dot(w,vec3<f32>(1.0)),0.001);
    var a=vec4<f32>(1.0,0.0,0.0,0.0);
    var b=a;var c=a;
    if weights.x>0.01 {a=stone_micro_plane(p.yz,footprint);}
    if weights.y>0.01 {b=stone_micro_plane(p.xz,footprint);}
    if weights.z>0.01 {c=stone_micro_plane(p.xy,footprint);}
    let grain=a.x*weights.x+b.x*weights.y+c.x*weights.z;
    let mineral=a.w*weights.x+b.w*weights.y+c.w*weights.z;
    let color=mix(vec3<f32>(grain),vec3<f32>(1.27,1.20,1.08),mineral*0.40);
    let gradient=vec3<f32>(0.0,a.y,a.z)*weights.x+vec3<f32>(b.y,0.0,b.z)*weights.y
        +vec3<f32>(c.y,c.z,0.0)*weights.z;
    return GroundMicro(color,gradient);
}
fn litter_micro_surface(p:vec2<f32>, pixel_footprint:f32, woodland:f32)->GroundMicro {
    let footprint=ground_detail_footprint(pixel_footprint);
    let grit=surface_band(p,71.0,footprint);
    let humus=surface_band(p,14.0,footprint);
    var color=vec3<f32>(0.70+humus.x*0.18+grit.x*0.14);
    var gradient=vec2<f32>(humus.y,humus.z)*0.045;
    let scale=mix(12.0,7.0,woodland);
    let q=p*scale;
    let cell=floor(q);let local=fract(q);
    let aa=clamp(pixel_footprint*scale*0.60,0.012,0.22);
    let normal_detail=1.0-smoothstep(0.12,0.60,pixel_footprint*scale);
    for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){
        let offset=vec2<f32>(f32(x),f32(y));let id=cell+offset;
        let seed=climate_lattice(vec2<i32>(id),0x6c656166u);
        let v=local-offset-vec2<f32>(climate_lattice(vec2<i32>(id),0x6c697474u),climate_lattice(vec2<i32>(id),0x74657273u));
        // Both masks are exactly zero outside this circle: the leaf ellipse
        // has a maximum semiaxis of 0.45 and the complete twig fits in 0.60.
        let support=max(0.60,0.45*(1.07+aa*4.0));
        if dot(v,v)>support*support { continue; }
        let angle=seed*6.2831853;let axis=vec2<f32>(cos(angle),sin(angle));
        let side=vec2<f32>(-axis.y,axis.x);
        let uv=vec2<f32>(dot(v,side),dot(v,axis));
        let radius=length(uv/vec2<f32>(0.15+seed*0.12,0.22+seed*0.23));
        let lobes=1.0+woodland*0.07*sin(uv.y*53.0+seed*7.0);
        let leaf=(1.0-smoothstep(lobes-aa*4.0,lobes+aa*4.0,radius));
        let midrib=1.0-smoothstep(0.01,0.025+aa,abs(uv.x));
        let veins=1.0-smoothstep(0.015,0.04+aa,abs(fract((uv.y-abs(uv.x)*0.65)*16.0)-0.5));
        let stone=mix(vec3<f32>(0.78,0.82,0.85),vec3<f32>(1.32,1.16,0.90),seed);
        let leaf_color=mix(vec3<f32>(0.85,0.61,0.34),vec3<f32>(1.65,1.27,0.70),seed);
        var tint=mix(stone,leaf_color,woodland)*(1.02-uv.x*0.40-radius*0.12);
        tint*=1.0+woodland*(midrib*0.12+veins*0.05);
        color=mix(color,tint,leaf*0.92);
        gradient=mix(gradient,(side*uv.x*0.70+axis*uv.y*0.40)*normal_detail,leaf);
        let twig=(1.0-smoothstep(0.008,0.018+aa,abs(uv.x+uv.y*0.25)))
            *(1.0-smoothstep(0.30,0.46,abs(uv.y)))*woodland*step(0.70,seed);
        color=mix(color,vec3<f32>(0.47,0.37,0.24),twig);
    }}
    return GroundMicro(color,vec3<f32>(gradient.x,0.0,gradient.y));
}

// Mipmapped static properties. Color/normal/snow are packed UNORM8; the four
// low-frequency climate fields use FP16 so snowline blends retain precision.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<storage,read> surface_pixels: array<vec2<u32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<storage,read> surface_pages: array<vec4<f32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<storage,read> terrain_samples: array<vec4<f32>>;
struct CachedSurface { color:vec3<f32>, normal:vec3<f32>, snow:f32, weight:f32, climate:vec4<f32>, climate_valid:bool }
fn cache_texel(slot:u32,level:u32,coordinate:vec2<i32>)->mat2x4<f32>{
    let size=256u>>level;let offset=(65536u-size*size)*4u/3u;
    let xy=vec2<u32>(clamp(coordinate,vec2<i32>(0),vec2<i32>(i32(size)-1)));
    let value=surface_pixels[slot*87381u+offset+xy.y*size+xy.x];
    return mat2x4<f32>(unpack4x8unorm(value.x),unpack4x8unorm(value.y));
}
fn cache_bilinear(slot:u32,level:u32,uv:vec2<f32>)->mat2x4<f32>{
    let q=uv*f32(256u>>level)-0.5;let xy=vec2<i32>(floor(q));let f=fract(q);
    let a=cache_texel(slot,level,xy);let b=cache_texel(slot,level,xy+vec2<i32>(1,0));
    let c=cache_texel(slot,level,xy+vec2<i32>(0,1));let d=cache_texel(slot,level,xy+vec2<i32>(1,1));
    return mat2x4<f32>(mix(mix(a[0],b[0],f.x),mix(c[0],d[0],f.x),f.y),mix(mix(a[1],b[1],f.x),mix(c[1],d[1],f.x),f.y));
}
fn climate_texel(slot:u32,coordinate:vec2<i32>)->vec4<f32>{
    let xy=vec2<u32>(clamp(coordinate,vec2<i32>(0),vec2<i32>(255)));
    let value=surface_pixels[48u*87381u+slot*65536u+xy.y*256u+xy.x];
    return vec4<f32>(unpack2x16float(value.x),unpack2x16float(value.y));
}
fn cached_climate(slot:u32,uv:vec2<f32>)->vec4<f32>{
    let q=uv*256.0-0.5;let xy=vec2<i32>(floor(q));let f=fract(q);
    let a=climate_texel(slot,xy);let b=climate_texel(slot,xy+vec2<i32>(1,0));
    let c=climate_texel(slot,xy+vec2<i32>(0,1));let d=climate_texel(slot,xy+vec2<i32>(1,1));
    return mix(mix(a,b,f.x),mix(c,d,f.x),f.y);
}
fn cached_surface(p:vec3<f32>,normal:vec3<f32>,footprint:f32)->CachedSurface{
    // Finished shading needs a resolved footprint; climate does not.
    let weight=smoothstep(0.06,0.15,ground_detail_footprint(footprint));
    var result=CachedSurface(vec3<f32>(0.0),normal,0.0,0.0,vec4<f32>(0.0),false);
    for(var level=0u;level<3u;level++){
        let span=f32(32u<<(level*2u));let cell=vec2<i32>(floor(p.xz/span));
        let wrap=((cell%4)+4)%4;let slot=level*16u+u32(wrap.x+wrap.y*4);
        let page=surface_pages[slot];let uv=(p.xz-page.xy)/span;
        let quality=1.0-smoothstep(1.0,2.0,(span/256.0)/max(footprint,0.0001));
        if page.w>0.0 && page.z==span && all(uv>=vec2<f32>(0.0)) && all(uv<vec2<f32>(1.0)) {
            // Fine pages preserve steep woodland/climate transitions nearby.
            if level==0u && weight*quality<1.0 && all(uv>=vec2<f32>(0.001953125)) && all(uv<=vec2<f32>(0.998046875)) {
                result.climate=cached_climate(slot,uv);
                result.climate_valid=true;
            }
            if weight*quality<=0.0 {if result.climate_valid {return result;} continue;}
            let mip=clamp(log2(max(footprint*256.0/span,1.0)),0.0,8.0);
            let lo=u32(floor(mip));let hi=min(lo+1u,8u);
            let a=cache_bilinear(slot,lo,uv);let b=cache_bilinear(slot,hi,uv);
            let color=mix(a[0],b[0],fract(mip));let n=mix(a[1],b[1],fract(mip));
            result.color=color.xyz;result.normal=normalize(n.xyz*2.0-1.0);
            result.snow=n.w;result.weight=weight*quality;
            return result;
        }
    }
    return result;
}

// Angular sky radiance: the disc and its aureole share the actual shadow light.
// Surface detail is evaluated only close to the sun, and fades below pixel size.
fn solar_radiance(direction: vec3<f32>, sun_direction: vec3<f32>) -> vec3<f32> {
    let distance = length(direction - sun_direction);
    let radius = 0.030;
    let pixel = 1.32 / max(uniforms.resolution.y, 1.0);
    let haze = exp(-distance * distance * 12.0) * 0.065
        + exp(-distance * 20.0) * 0.32;
    var radiance = vec3<f32>(1.0, 0.68, 0.34) * haze;
    if distance > 0.20 { return radiance; }

    // A pole-safe tangent frame keeps the photosphere fixed in the sky.
    let reference = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(sun_direction.y) > 0.98);
    let right = normalize(cross(reference, sun_direction));
    let up = cross(sun_direction, right);
    let uv = vec2<f32>(dot(direction, right), dot(direction, up)) / radius;
    let r = distance / radius;
    let radial = uv / max(length(uv), 0.001);
    // Broad, uneven atmospheric wisps; no screen-space flare or repeating rays.
    let wisps = ground_noise(radial * 4.0 + vec2<f32>(17.3, 9.1));
    let aureole = exp(-max(distance - radius, 0.0) * 75.0)
        * (0.82 + 0.18 * wisps);
    radiance += vec3<f32>(1.0, 0.78, 0.46) * aureole * 0.65;
    let disc = 1.0 - smoothstep(radius - pixel * 0.65, radius + pixel * 0.65, distance);
    if disc > 0.0 {
        // Limb darkening gives the hot center depth without drawing a dark rim.
        let mu = sqrt(max(1.0 - r * r, 0.0));
        let limb = 0.38 + 0.62 * pow(mu, 0.65);
        let broad = ground_noise(uv * 3.5 + vec2<f32>(8.4, 21.7));
        let resolved = 1.0 - smoothstep(0.05, 0.13, pixel / radius);
        let granules = ground_noise(uv * 13.0 + vec2<f32>(32.1, 4.8));
        let surface = 0.82 + 0.28 * broad + resolved * 0.16 * (granules - 0.5);
        let heat = mix(vec3<f32>(1.0, 0.44, 0.12), vec3<f32>(1.0, 0.89, 0.62), pow(mu, 0.45));
        radiance += heat * (3.8 * limb * surface * disc);
    }
    return radiance;
}

fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    if uniforms.camera_forward.w > 0.5 { return vec3<f32>(0.001, 0.002, 0.004); }
    // Cap only physical excavation at the finite draw boundary. An angular
    // projection of a nearby portal can cover enormous regions of outdoor sky.
    let end = uniforms.camera_position.xyz + direction * uniforms.resolution.w;
    for(var i=0u;i<64u;i++) {
        let a=uniforms.goblin_limits[i*2u];
        if a.w==0.0 {break;}
        let b=uniforms.goblin_limits[i*2u+1u];
        var inside=false;
        if a.w<0.0 {
            let q=(end-a.xyz)/b.xyz;
            inside=dot(q,q)<1.0;
        } else {
            let edge=b.xyz-a.xyz;
            let t=clamp(dot(end-a.xyz,edge)/dot(edge,edge),0.0,1.0);
            let q=end-(a.xyz+t*edge);
            inside=dot(q,q)<a.w*a.w;
        }
        if inside && end.y<terrain_height(end.xz)-0.5 {
            return vec3<f32>(0.003,0.005,0.006);
        }
    }
    let horizon = pow(1.0 - abs(direction.y), 4.0);
    let gradient = mix(vec3<f32>(0.012, 0.018, 0.045), vec3<f32>(0.08, 0.15, 0.24), max(direction.y, 0.0));
    var sun_direction = normalize(vec3<f32>(-0.55, 0.78, 0.42));
#ifndef PREPASS_PIPELINE
    if lights.n_directional_lights > 0u { sun_direction = lights.directional_lights[0].direction_to_light; }
#endif
    let clear_sky = gradient + horizon * vec3<f32>(0.08, 0.13, 0.17)
        + solar_radiance(direction, sun_direction);
    let winter_sky = mix(vec3<f32>(0.23, 0.31, 0.40), vec3<f32>(0.10, 0.17, 0.27), max(direction.y, 0.0));
    return mix(clear_sky, winter_sky, uniforms.resolution.z * 0.88);
}

@fragment
fn fragment(mesh: VertexOutput)
#ifdef PREPASS_PIPELINE
#ifdef TERRAIN_MESH
{
#else
-> FragmentOutput {
#endif
#else
-> FragmentOutput {
#endif
#ifdef TERRAIN_MESH
    let footprint = max(length(dpdx(mesh.world_position)),length(dpdy(mesh.world_position)));
    let offset = mesh.world_position - uniforms.camera_position.xyz;
    let ray_direction = normalize(offset);
    let ray_origin = uniforms.camera_position.xyz;
    let hit = surface(length(offset), 1.0);
#ifndef PREPASS_PIPELINE
    if hit.distance > uniforms.resolution.w { discard; }
#else
#ifdef VIEW_PROJECTION_NONSTANDARD
    // The world camera uses FinitePerspective. Its depth pass must have the
    // same radial silhouette as color. Native point/directional shadow views
    // use perspective/orthographic projections and retain offscreen casters.
    if hit.distance > uniforms.resolution.w { discard; }
#endif
#endif
    if inside_orc_hole(mesh.world_position) || inside_goblin_portal(mesh.world_position) { discard; }
    // Hardware depth rejects terrain behind vegetation before surface shading.
#else
    let footprint = 1.0;
    let resolution = max(uniforms.resolution.xy, vec2<f32>(1.0));
    let aspect = resolution.x / resolution.y;
    let screen = mesh.uv * 2.0 - vec2<f32>(1.0);
    let ray_direction = normalize(
        uniforms.camera_forward.xyz +
        uniforms.camera_right.xyz * screen.x * aspect * 0.66 -
        uniforms.camera_up.xyz * screen.y * 0.66
    );
    let ray_origin = uniforms.camera_position.xyz;
    var ray_limit = uniforms.resolution.w;
#ifndef PREPASS_PIPELINE
#ifdef DEPTH_PREPASS
    // The farthest MSAA sample is a conservative occlusion bound for the
    // entire pixel. An uncovered sample prevents early rejection at silhouettes.
    var scene_depth = prepass_depth(mesh.position, 0u);
#ifdef MULTISAMPLED
    for (var sample = 1u; sample < textureNumSamples(view_bindings::depth_prepass_texture); sample += 1u) {
        scene_depth = min(scene_depth,prepass_depth(mesh.position,sample));
    }
#endif
    let scene_travel = 0.05 * uniforms.resolution.w /
        ((scene_depth * (uniforms.resolution.w - 0.05) + 0.05) * dot(ray_direction,uniforms.camera_forward.xyz));
    // Cover the prepass's conservative offset and independent shader rounding.
    ray_limit = min(ray_limit, scene_travel + max(0.002,scene_travel*0.00025));
#endif
#endif
    let hit = raymarch(ray_origin, ray_direction, ray_limit);
    // A depth-writing sky sphere clips ALL mesh materials at the same radial
    // distance as terrain. The projection far plane is only a conservative cap.
    // This also works with MSAA and needs no per-tree material overrides.
    let visible_travel = select(uniforms.resolution.w, min(hit.distance, uniforms.resolution.w), hit.material > 0.0);
    let hit_depth = clamp((0.05 * uniforms.resolution.w / (visible_travel * dot(ray_direction, uniforms.camera_forward.xyz)) - 0.05) / (uniforms.resolution.w - 0.05), 0.0, 1.0);
#ifndef PREPASS_PIPELINE
#ifdef DEPTH_PREPASS
    if scene_depth > hit_depth + 0.0000001 {
        discard;
    }
#endif
#endif
#endif
#ifdef PREPASS_PIPELINE
    // Share the exact silhouette, cave cutouts, radial cutoff and projection
    // with the color pass. No lighting or material shading in the depth pass.
#ifdef TERRAIN_MESH
    // Retain hardware per-sample raster depth, including MSAA coverage.
    // Writing the pixel-center depth here would disagree with the color pass.
    return;
#else
    var out: FragmentOutput;
    // Depth used for culling must be conservative across separately compiled
    // depth/color shaders. Cover roundoff in normalization, dot and projection,
    // plus the ray march's existing hit tolerance. Only the prepass moves back;
    // the color pass retains the exact surface and radial clipping depth.
    let roundoff = visible_travel * (64.0 * 1.1920928955078125e-7);
    let hit_tolerance = select(0.0, SURFACE_EPSILON * max(1.0, visible_travel * 0.12), hit.material > 0.0);
    let conservative_travel = visible_travel + max(roundoff, hit_tolerance);
    out.depth = clamp((0.05 * uniforms.resolution.w / (conservative_travel * dot(ray_direction, uniforms.camera_forward.xyz)) - 0.05) / (uniforms.resolution.w - 0.05), 0.0, 1.0);
    return out;
#endif
#else
    var color = vec3<f32>(0.0);

    if hit.material > 0.0 {
#ifdef TERRAIN_MESH
        let p = mesh.world_position;
        var normal = normalize(mesh.normal);
#else
        let p = ray_origin + ray_direction * hit.distance;
        var normal = vec3<f32>(0.0, 1.0, 0.0);
#endif
        let material_normal = normal;
#ifdef ROCK_MESH
        let detail=max(uniforms.camera_position.w,1.0);
        let rock_weight=smoothstep(detail,detail+max(8.0,detail*0.25),hit.distance/GROUND_DETAIL_RANGE)*smoothstep(0.12,0.25,ground_detail_footprint(footprint))*mesh.static_color.w;
        var cached = cached_surface(p,normal,0.0);
        cached.color=mesh.static_color.xyz;cached.normal=normalize(mesh.static_normal.xyz);
        cached.snow=mesh.static_normal.w;cached.weight=rock_weight;
#else
        var cached=CachedSurface(vec3<f32>(0.0),normal,0.0,0.0,vec4<f32>(0.0),false);
        if hit.material<1.5 {cached=cached_surface(p,normal,footprint);}
#endif
        var mountain = 0.0;
        var fields=cached.climate;
        var snow=cached.snow;
        if cached.weight < 1.0 && hit.material<1.5 {
            if !cached.climate_valid { fields=surface_climate(p.xz); }
            mountain=fields.y;
            snow=climate_snow(p,material_normal,fields);
        }
        var alpine = MountainSurface(vec3<f32>(0.0),normal);
        if hit.material < 1.5 && mountain > 0.0 {
#ifdef ROCK_MESH
            let ground_normal = normalize(mesh.ground_normal);
            let contact_width = mesh.contact_scale * (0.75 + ground_noise(p.xz * 3.7)*0.5);
            let contact = 1.0-smoothstep(0.005,contact_width,p.y-mesh.ground_height);
            if contact < 1.0 { alpine = mountain_surface_snow(p,normal,1.0,footprint,snow); }
            if contact > 0.0 {
                let gp=vec3<f32>(p.x,mesh.ground_height,p.z);
                let gc=cached_surface(gp,ground_normal,footprint);
                var ground=MountainSurface(gc.color,gc.normal);
                if gc.weight<1.0 {
                    let ground_cover=climate_snow(gp,ground_normal,fields);
                    ground=mountain_surface_snow(gp,ground_normal,0.0,footprint,ground_cover);
                    let ground_exposed=(1.0-smoothstep(0.55,0.83,ground_normal.y))*smoothstep(0.02,0.20,mountain);
                    let ground_weight=max(mountain,ground_exposed);
                    if ground_weight<1.0 {
                        ground.color=mix(material_color_climate(1.0,gp,ground_normal,ground_cover,fields.z,footprint),ground.color,ground_weight);
                    }
                    // Cached color already contains the terrain/alpine blend.
                    // Blend it once, after assembling the procedural fallback.
                    ground.color=mix(ground.color,gc.color,gc.weight);
                    ground.normal=normalize(mix(normalize(mix(ground_normal,ground.normal,mountain)),gc.normal,gc.weight));
                }
                alpine.color = mix(alpine.color,ground.color,contact);
                alpine.normal = normalize(mix(alpine.normal,ground.normal,contact));
            }
#else
            alpine = mountain_surface_snow(p,normal,0.0,footprint,snow);
#endif
        }
        if hit.material > 1.5 {
            normal = scene_normal(p);
        } else {
            if mountain > 0.0 {
#ifdef ROCK_MESH
                normal = alpine.normal;
#else
                normal = normalize(mix(normal,alpine.normal,mountain));
#endif
            }

        }
        normal = normalize(mix(normal,cached.normal,cached.weight));
        var base = vec3<f32>(0.0);
        if cached.weight < 1.0 {
        if hit.material < 1.5 && mountain > 0.0 {
#ifdef ROCK_MESH
            base = alpine.color;
#else
            let exposed = (1.0 - smoothstep(0.55,0.83,material_normal.y)) * smoothstep(0.02,0.20,mountain);
            let alpine_weight = max(mountain,exposed);
            if alpine_weight < 1.0 { base = material_color_climate(hit.material, p, material_normal,snow,fields.z,footprint); }
            base = mix(base,alpine.color,alpine_weight);
#endif
        } else {
            base = material_color_climate(hit.material, p, material_normal,snow,fields.z,footprint);
        }

        }
        base = mix(base,cached.color,cached.weight);
#ifdef TERRAIN_MESH
#ifndef ROCK_MESH
        let resolved_ground=ground_detail_footprint(footprint);
        var wear=0.0;
        if max(abs(p.x),abs(p.z))<8.5 {
            let wear_broad=clamp(0.5+powder_field(p.xz*0.28).x*0.65,0.0,1.0);
            let wear_mottle=clamp(0.5+powder_field(p.xz*2.3+wear_broad*0.7).x*0.6,0.0,1.0);
            wear=ground_wear(p.xz,wear_mottle);
        }
        let turf_weight=(1.0-smoothstep(0.008,0.035,resolved_ground))
            *(1.0-snow)*(1.0-mountain)*(1.0-fields.z)*(1.0-wear)
            *smoothstep(0.55,0.85,material_normal.y);
        if hit.material<1.5 && turf_weight>0.001 {
            let turf=turf_detail(p.xz,footprint);
            base*=mix(vec3<f32>(1.0),turf.color*1.38,turf_weight);
            let clump=powder_field(p.xz*7.1);
            let gradient=vec3<f32>(clump.y,0.0,clump.z)*0.065
                +vec3<f32>(turf.gradient.x,0.0,turf.gradient.y);
            normal=normalize(normal-(gradient-material_normal*dot(gradient,material_normal))*turf_weight);
        }
        let litter_weight=(1.0-smoothstep(0.018,0.09,resolved_ground))
            *(1.0-snow)*(1.0-mountain)*max(fields.z,wear);
        if hit.material<1.5 && litter_weight>0.001 {
            let litter=litter_micro_surface(p.xz,footprint,fields.z);
            base*=mix(vec3<f32>(1.0),litter.color,litter_weight);
            normal=normalize(normal-(litter.gradient-material_normal*dot(litter.gradient,material_normal))*litter_weight);
        }
        if hit.material<1.5 && cached.weight<1.0 && snow*(1.0-mountain)>0.001 {
            let powder=snow_micro_surface(p,material_normal,footprint);
            normal=normalize(mix(normal,powder.normal,snow*(1.0-mountain)*(1.0-cached.weight)));
        }
#endif
#endif
        var input = pbr_input_new();
        input.frag_coord = mesh.position;
        input.world_position = vec4<f32>(p, 1.0);
        input.world_normal = material_normal;
        input.N = normal;
        input.V = -ray_direction;
        input.flags = MESH_FLAGS_SHADOW_RECEIVER_BIT;
        input.material.base_color = vec4<f32>(base, 1.0);
        input.material.perceptual_roughness = mix(0.88,0.76,snow);
        input.material.reflectance = vec3<f32>(0.35);
        var surface_occlusion = 1.0;
        if hit.material < 1.5 {
            let snow_cover = mix(snow,cached.snow,cached.weight);
            var tracks = vec2<f32>(0.0);
            if snow_cover > 0.0 { tracks = tracks_shading(p.xz) * snow_cover; }
            // Stronger packed-snow contrast and visible raised powder edges.
            let track_depth = clamp(tracks.x, 0.0, 1.0);
            let track_lip = clamp(tracks.y, 0.0, 1.0);
            input.material.base_color = vec4<f32>(base * (1.0 - track_depth * 0.70 + track_lip * 0.22), 1.0);
            input.material.perceptual_roughness = mix(input.material.perceptual_roughness, 0.65, tracks.x);
            surface_occlusion *= 1.0 - track_depth * 0.30;
            // Ground contact shadow anchors the mesh without coupling its topology to SDFs.
            let footprint = length((p.xz - uniforms.player_position.xz) / vec2<f32>(0.36, 0.26));
            let contact = exp(-footprint * footprint * 0.75);
            surface_occlusion *= 1.0 - contact * 0.35 * exp(-uniforms.player_position.y * 2.0);
        }
        input.diffuse_occlusion = vec3<f32>(surface_occlusion);
        // Heightfield terrain and its surface rocks are exterior receivers.
        // Native cave geometry still uses spatial sky exclusion. Keep ordinary
        // shadow maps active here: only the coarse underground classifier is bypassed.
        color = main_pass_post_lighting_processing(input, hither_pbr_lighting(input, true)).rgb;
        // Do not apply SDF-only fog: mesh grass and trees use clear lighting.
        // Any future atmospheric fog must cover both rendering paths equally.
    } else {
        color = sky_color(ray_direction);
    }

    // Linear HDR is composed with standard materials and hands before tonemapping.
    var out: FragmentOutput;
    out.color = vec4<f32>(color, 1.0);
#ifndef TERRAIN_MESH
    out.depth = hit_depth;
#endif
    return out;
#endif
}
