// Adapted from Bevy 0.19.1 shadows.wgsl (MIT OR Apache-2.0).
fn fetch_directional_shadow(
    light_id: u32,
    frag_position: vec4<f32>,
    surface_normal: vec3<f32>,
    _view_z: f32,
    frag_coord_xy: vec2<f32>,
) -> f32 {
    // HITHER_RADIAL_SHADOWS: camera rotation cannot move the coverage boundary.
    let radius = distance(frag_position.xyz, view_bindings::view.world_position);
    let view_z = -radius;
    let light = &view_bindings::lights.directional_lights[light_id];
    let cascade_index = get_cascade_index(light_id, view_z);

    if (cascade_index >= (*light).num_cascades) {
        return 1.0;
    }

    var shadow = sample_directional_cascade(
        light_id,
        cascade_index,
        frag_position,
        surface_normal,
        frag_coord_xy,
    );

    // Blend with the next cascade, if there is one.
    let next_cascade_index = cascade_index + 1u;
    if (next_cascade_index < (*light).num_cascades) {
        let this_far_bound = (*light).cascades[cascade_index].far_bound;
        let next_near_bound = 0.8 * this_far_bound;
        if (-view_z >= next_near_bound) {
            let next_shadow = sample_directional_cascade(
                light_id,
                next_cascade_index,
                frag_position,
                surface_normal,
                frag_coord_xy,
            );
            shadow = mix(shadow, next_shadow, (-view_z - next_near_bound) / (this_far_bound - next_near_bound));
        }
    }
    let far_bound = (*light).cascades[(*light).num_cascades - 1u].far_bound;
    return mix(shadow, 1.0, smoothstep(far_bound * 0.85, far_bound, radius));
}

