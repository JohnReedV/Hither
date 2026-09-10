#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#else
#import bevy_pbr::forward_io::{Vertex, VertexOutput}
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> motion: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> detail: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> camera_position: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> tramplers: array<vec4<f32>,16>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> player_steps: array<vec4<f32>,16>;

@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> contact_bounds: vec4<f32>;

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(input.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(input.position, 1.0));
    let root_weight = input.uv.y * input.uv.y;
    // Rotation-invariant distance. A narrow, irregular transition avoids an
    // obvious straight wall of grass without thinning the whole detail range.
    let distance_to_camera = length(world.xyz - camera_position.xyz);
    let edge = detail.x - 1.5 * input.uv.x;
    let fade = 1.0 - smoothstep(edge - 0.5, edge, distance_to_camera);
    let root_height = input.color.a;
    if fade == 0.0 {
        // Preserve the exact sunk triangle, including its original XZ position.
        world.y = root_height - 0.015;
    } else {
        let distance_to_player = length(world.xz - motion.yz);
    let gust = sin(world.x * 0.8 + world.z * 0.55 + motion.x * 1.3)
        + 0.35 * sin(world.z * 2.1 - motion.x * 2.4);
    var away = (world.xz - motion.yz) / max(distance_to_player, 0.05);
    var foot = (1.0 - smoothstep(0.22, 0.58, distance_to_player)) * (1.0 - smoothstep(0.1, 0.6, motion.w));
    // Body-space contacts persist briefly after a step, independently of the
    // camera or avatar visibility. Their CPU envelope provides smooth recovery.
    if all(world.xz >= contact_bounds.xy) && all(world.xz <= contact_bounds.zw) {
    for(var i=0u;i<16u;i+=1u){
        let contact=player_steps[i];
        if contact.w==0.0 {break;}
        let delta=world.xz-contact.xy;
        let squared=dot(delta,delta);
        if squared<contact.z*contact.z {
            let distance=sqrt(squared);
            let press=(1.0-smoothstep(0.22,contact.z,distance))*contact.w;
            if press>foot {foot=press;away=delta/max(distance,0.05);}
        }
    }
    for(var i=0u;i<16u;i+=1u){
        let orc=tramplers[i];
        if orc.w==0.0 {break;}
        let delta=world.xz-orc.xy;
        let squared=dot(delta,delta);
        if squared<orc.z*orc.z {
            let distance=sqrt(squared);
            let press=1.0-smoothstep(0.18,orc.z,distance);
            if press>foot {foot=press;away=delta/max(distance,0.05);}
        }
    }
    }
    world.x += (gust * 0.022 + away.x * foot * 0.13) * root_weight * fade;
    world.z += (gust * 0.013 + away.y * foot * 0.13) * root_weight * fade;
    // Sink culled blades beneath the surface rather than leaving flat,
    // depth-fighting triangles on the terrain.
    world.y = root_height + (world.y - root_height) * fade * (1.0 - foot * 0.75) - 0.015 * (1.0 - fade);
    }
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
#ifndef PREPASS_PIPELINE
    out.world_normal = mesh_functions::mesh_normal_local_to_world(input.normal, input.instance_index);
#endif
#ifdef VERTEX_UVS_A
    out.uv = input.uv;
#endif
#ifdef VERTEX_COLORS
    out.color = vec4<f32>(input.color.rgb, 1.0);
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = input.instance_index;
#endif
    return out;
}
