#import bevy_pbr::{mesh_functions, forward_io::Vertex, view_transformations::position_world_to_clip, mesh_view_bindings::view}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> motion: vec4<f32>;
struct FlameVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec3<f32>,
    @location(1) @interpolate(flat) origin: vec3<f32>,
}
@vertex
fn vertex(v: Vertex) -> FlameVertex {
    let model = mesh_functions::get_world_from_local(v.instance_index);
    var out: FlameVertex;
    out.position = position_world_to_clip((model * vec4<f32>(v.position, 1.0)).xyz);
    out.local = v.position;
    out.origin = model[3].xyz;
    return out;
}
fn hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(hash(i), hash(i + vec3<f32>(1.,0.,0.)), u.x),
        mix(hash(i + vec3<f32>(0.,1.,0.)), hash(i + vec3<f32>(1.,1.,0.)), u.x), u.y),
        mix(mix(hash(i + vec3<f32>(0.,0.,1.)), hash(i + vec3<f32>(1.,0.,1.)), u.x),
        mix(hash(i + vec3<f32>(0.,1.,1.)), hash(i + vec3<f32>(1.,1.,1.)), u.x), u.y), u.z);
}
fn turbulence(p: vec3<f32>) -> f32 {
    return noise(p) * 0.57 + noise(p * 2.07 + 11.4) * 0.28 + noise(p * 4.13 + 7.8) * 0.15;
}
// Identical to the CPU light trajectory, with h running from root to tip.
fn wisp(t: f32, i: f32, h: f32) -> vec3<f32> {
    let phase = t + i * 1.71;
    let root = vec2<f32>(cos(i * 2.39996), sin(i * 2.39996)) * (0.025 + i * 0.009);
    let bend = vec2<f32>(sin(phase * 3.1 - h * 5.), cos(phase * 2.3 - h * 6.)) * (0.022 + 0.12 * h * h);
    return vec3<f32>(root.x + bend.x, 0., root.y + bend.y);
}
fn flame(p: vec3<f32>, t: f32) -> vec4<f32> {
    // The ignition envelope is zero below -18 mm, and every ember is
    // above 60 mm with at most 18.75 mm vertical support.
    if p.y < -0.018 { return vec4<f32>(0.); }
    var ember = 0.;
    for (var i = 0; i < 7; i += 1) {
        let fi = f32(i);
        let age = fract(t * (0.38 + fi * 0.023) + fi * 0.173);
        let point = vec3<f32>(sin(fi * 7.1 + age * 4.) * (0.04 + age * 0.16),
            0.06 + age * 1.13, cos(fi * 3.9 + age * 5.) * (0.035 + age * 0.10));
        let d = length((p - point) * vec3<f32>(1., 0.48, 1.));
        ember += (1. - smoothstep(0.001, 0.009, d)) * (1. - age) * smoothstep(0., 0.10, age);
    }
    // All five wisps end below (0.56 + 0.15 + 0.08) * 1.05 metres.
    // Above that bound only embers contribute; the zero-density gold is
    // independent of noise here. Preserve the same ember color/absorption.
    if p.y > 0.8296 {
        let color = mix(vec3<f32>(1., 0.16, 0.003), vec3<f32>(1., 0.39, 0.025), clamp(ember, 0., 1.));
        return vec4<f32>(color, ember * 180.);
    }
    let flow = vec3<f32>(p.x * 12., p.y * 7. - t * 2.8, p.z * 12.);
    let n = turbulence(flow);
    let detail = noise(flow * 3.7 + vec3<f32>(t, -t * 1.7, 0.));
    var density = 0.;
    for (var i = 0; i < 5; i += 1) {
        let fi = f32(i);
        let phase = t + fi * 1.71;
        let height = 0.56 + 0.15 * sin(phase * 2.7) + 0.08 * sin(phase * 6.1);
        let h = p.y / height;
        let center = wisp(t, fi, h);
        // Broad combustion zone peels into slender, independently curling tips.
        let radius = (0.052 + 0.043 * sin(clamp(h, 0., 1.) * 3.14159))
            * pow(max(1. - h, 0.), 0.65);
        let ragged = (n - 0.46) * 0.075 * (0.2 + h) + (detail - 0.5) * 0.013;
        let distance = length(p.xz - center.xz) + ragged;
        let skin = 1. - smoothstep(radius * 0.70, radius + 0.005, distance);
        let hollow = 1. - smoothstep(radius * 0.12, radius * 0.68, distance);
        let tears = smoothstep(0.22, 0.53, n + 0.14 * detail + (1. - h) * 0.20);
        let body = skin * (1. - hollow * 0.58) * tears;
        let ignition = 0.026 * noise(vec3<f32>(p.x * 53., t * 3., p.z * 53.));
        density += body * smoothstep(-0.018, 0.027, p.y - ignition) * (1. - smoothstep(0.86, 1.05, h));
    }
    density = min(density, 2.6);
    let root_lace = 0.55 + 0.45 * noise(vec3<f32>(p.x * 75., p.y * 24. - t * 6., p.z * 75.));
    density *= mix(root_lace, 1., smoothstep(0.02, 0.12, p.y));
    // Cobalt at the fuel surface, cyan transition, then luminous gold and amber.
    let blue = vec3<f32>(0.018, 0.13, 0.85);
    let gold = mix(vec3<f32>(1., 0.16, 0.003), vec3<f32>(1., 0.76, 0.12), clamp(density * 0.43, 0., 1.));
    var color = mix(blue, gold, smoothstep(0.008, 0.065, p.y + (n - 0.5) * 0.16 + (detail - 0.5) * 0.02));
    color = mix(color, vec3<f32>(1., 0.96, 0.64), smoothstep(1.6, 2.6, density) * smoothstep(0.08, 0.18, p.y) * 0.45);
    color = mix(color, vec3<f32>(1., 0.39, 0.025), clamp(ember, 0., 1.));
    return vec4<f32>(color, density * 20. + ember * 180.);
}
@fragment
fn fragment(in: FlameVertex) -> @location(0) vec4<f32> {
    // Torch instances are translated, never scaled or rotated: metres stay metres.
    let camera = view.world_position - in.origin;
    let direction = normalize(in.local - camera);
    let safe_direction = sign(direction + vec3<f32>(0.000001)) * max(abs(direction), vec3<f32>(0.000001));
    let a = (vec3<f32>(-0.45, -0.08, -0.45) - camera) / safe_direction;
    let b = (vec3<f32>(0.45, 1.28, 0.45) - camera) / safe_direction;
    let entry = min(a,b);
    let leave = max(a,b);
    let near = max(max(entry.x, entry.y), max(entry.z, 0.));
    let far = min(min(leave.x, leave.y), leave.z);
    if far <= near { discard; }
    let phase = fract((in.origin.x * 0.73 + in.origin.z * 0.37) / 6.2831853) * 6.2831853;
    let t = motion.x + phase;
    let step = (far - near) / 72.;
    var rgb = vec3<f32>(0.);
    var alpha = 0.;
    // Front-to-back absorption gives a continuous 3D flame from every angle.
    for (var i = 0; i < 72; i += 1) {
        let p = camera + direction * (near + (f32(i) + 0.5) * step);
        let sample = flame(p, t);
        let opacity = 1. - exp(-sample.a * step);
        rgb += (1. - alpha) * sample.rgb * opacity;
        alpha += (1. - alpha) * opacity;
        if alpha > 0.985 { break; }
    }
    if alpha < 0.003 { discard; }
    // Return straight alpha for Bevy's Blend pipeline; retain warm color detail.
    return vec4<f32>(rgb / max(alpha, 0.001) * 1.20, alpha);
}
