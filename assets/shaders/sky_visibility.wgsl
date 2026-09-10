// Appended to Bevy's irradiance-volume library. RGB stores incident diffuse
// radiance; alpha stores a conservative terrain ceiling in world metres.
// Negative sentinel alpha belongs to ordinary local bounce volumes.
fn hither_sky_visibility(
    world_position: vec3<f32>,
    ranges: ptr<function, ClusterableObjectIndexRanges>,
) -> f32 {
    var iterator = light_probe_iterator_new(world_position, true, ranges);
    var visibility = 1.0;
    while (true) {
        let probe = light_probe_iterator_next(&iterator);
        if probe.texture_index < 0 { break; }
#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY
        let voxels = irradiance_volumes[probe.texture_index];
#else
        let voxels = irradiance_volume;
#endif
        let atlas = vec3<f32>(textureDimensions(voxels));
        let resolution = atlas / vec3<f32>(1.0, 2.0, 3.0);
        let local = (probe.light_from_world * vec4<f32>(world_position, 1.0)).xyz;
        let coordinate = clamp((local + 0.5) * resolution, vec3<f32>(0.5), resolution - 0.5) / atlas;
        let ceiling = textureSampleLevel(voxels, irradiance_volume_sampler, coordinate, 0.0).a;
        if ceiling > -60000.0 {
            visibility = min(visibility, smoothstep(-2.0, -0.15, world_position.y - ceiling));
        }
    }
    return visibility;
}
