//! A small streamed sky-irradiance field, shared by native and procedural PBR.
//! It stores directional sky radiance and conservative terrain ceilings. This
//! is spatial ambient lighting, not a dynamic multi-bounce GI solver.
use crate::{app::settings::GraphicsSettings, player::camera::CameraView};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageSampler, ImageSamplerDescriptor},
    light::IrradianceVolume,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    shader::Source,
};

const SIDE: usize = 64;
#[derive(Resource, Default)]
struct Field {
    key: Option<(IVec2, u32)>,
    pending: Option<crate::world::streaming::BuildTask<(Image, Transform, Samples)>>,
    samples: Option<Samples>,
    entity: Option<Entity>,
    image: Handle<Image>,
}
#[derive(Clone)]
struct Samples {
    cell: IVec2,
    span: f32,
    values: std::sync::Arc<Vec<(f32, f32)>>,
}
#[derive(Resource)]
struct Libraries {
    pbr: Handle<Shader>,
    irradiance: Handle<Shader>,
    installed: bool,
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Field>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                install,
                stream.in_set(crate::world::streaming::Layer::Contacts),
            ),
        );
}
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Libraries {
        pbr: assets.load("embedded://bevy_pbr/render/pbr_functions.wgsl"),
        irradiance: assets.load("embedded://bevy_pbr/light_probe/irradiance_volume.wgsl"),
        installed: false,
    });
}
// Version-pinned, checked integration at the common lighting boundary. Imports
// and bind groups remain Bevy's own; no per-material copies or additional passes.
fn install(mut library: ResMut<Libraries>, mut shaders: ResMut<Assets<Shader>>) {
    if library.installed
        || shaders.get(&library.pbr).is_none()
        || shaders.get(&library.irradiance).is_none()
    {
        return;
    }
    let mut shader = shaders.get_mut(&library.irradiance).unwrap();
    let source = shader.source.as_str();
    let end = source
        .rfind("#endif")
        .expect("Bevy irradiance library changed");
    shader.source = Source::Wgsl(
        format!(
            "{}\n{}\n{}",
            &source[..end],
            include_str!("../../assets/shaders/sky_visibility.wgsl"),
            &source[end..]
        )
        .into(),
    );
    drop(shader);
    let mut shader = shaders.get_mut(&library.pbr).unwrap();
    let mut source = shader.source.as_str().to_string();
    // Outdoor heightfield surfaces are known exterior receivers. Do not let a
    // low-resolution ambient proxy classify their valleys as underground.
    let entry = "fn apply_pbr_lighting(\n    in: pbr_types::PbrInput,\n) -> vec4<f32> {";
    assert_eq!(
        source.matches(entry).count(),
        1,
        "Bevy PBR entry point changed"
    );
    source = source.replace(entry, "fn apply_pbr_lighting(in: pbr_types::PbrInput) -> vec4<f32> {\n    return hither_pbr_lighting(in, false);\n}\n\nfn hither_pbr_lighting(in: pbr_types::PbrInput, exterior: bool) -> vec4<f32> {");
    let anchor = "    // directional lights (direct)";
    assert_eq!(
        source.matches(anchor).count(),
        1,
        "Bevy PBR library changed"
    );
    source = source.replace(anchor, "    var hither_sky = 1.0;\n#ifdef IRRADIANCE_VOLUME\n    if !exterior { hither_sky = irradiance_volume::hither_sky_visibility(in.world_position.xyz, &clusterable_object_index_ranges); }\n#endif\n    // directional lights (direct)");
    // Restrict substitution to directional light accumulation, leaving points alone.
    let begin = source.find(anchor).unwrap();
    let (before, after) = source.split_at(begin);
    assert!(
        after.contains("direct_light += light_contrib * shadow;"),
        "Bevy directional light accumulation changed"
    );
    source = format!(
        "{}{}",
        before,
        after.replacen(
            "direct_light += light_contrib * shadow;",
            "direct_light += light_contrib * shadow * hither_sky;",
            1
        )
    );
    let irradiance_anchor =
        "indirect_light += irradiance_volume_light * diffuse_color * diffuse_occlusion;";
    assert_eq!(
        source.matches(irradiance_anchor).count(),
        1,
        "Bevy irradiance accumulation changed"
    );
    source = source.replace(irradiance_anchor, "indirect_light += irradiance_volume_light * diffuse_color * diffuse_occlusion * hither_sky;");
    if std::env::var_os("HITHER_PROFILE_FOREST").is_some()
        && std::env::var_os("HITHER_PROFILE_NO_POINT_SAMPLING").is_some()
    {
        let point_sample = "shadows::fetch_point_shadow(light_id, in.world_position, in.world_normal, in.frag_coord.xy)";
        assert_eq!(source.matches(point_sample).count(), 1);
        source = source.replace(point_sample, "1.0");
        println!("PROFILE point shadow sampling disabled; map rendering unchanged");
    }
    shader.source = Source::Wgsl(source.into());
    library.installed = true;
}

fn stream(
    mut commands: Commands,
    mut coordinator: ResMut<crate::world::streaming::Coordinator>,
    view: Res<CameraView>,
    settings: Res<GraphicsSettings>,
    mut field: ResMut<Field>,
    mut images: ResMut<Assets<Image>>,
) {
    let span = (settings.render_distance * 2.0 + 64.0).max(160.0);
    let cell = field_cell(
        view.position.xz(),
        span,
        settings.render_distance,
        field.key,
    );
    let wanted = (cell, span.to_bits());
    if let Some(task) = field.pending.as_mut()
        && task.is_ready()
        && let Some(_scope) = coordinator.install_entities(
            crate::world::streaming::Layer::Contacts,
            1,
            SIDE * SIDE * 6 * 8,
            1,
        )
    {
        let (image, transform, samples) = task.take();
        field.pending = None;
        // Never install results for a region we already left (teleport/settings).
        if samples.span == span
            && field_covers(
                view.position.xz(),
                samples.cell,
                span,
                settings.render_distance,
            )
        {
            field.samples = Some(samples);
            if let Some(entity) = field.entity {
                if let Some(mut existing) = images.get_mut(&field.image) {
                    *existing = image;
                }
                commands.entity(entity).insert(transform);
            } else {
                field.image = images.add(image);
                field.entity = Some(
                    commands
                        .spawn((
                            Name::new("Streamed sky irradiance"),
                            bevy::camera::visibility::RenderLayers::from_layers(&[0, 2]),
                            IrradianceVolume {
                                voxels: field.image.clone(),
                                intensity: 1.0,
                                ..default()
                            },
                            transform,
                        ))
                        .id(),
                );
            }
        } else {
            field.key = None;
        }
    }
    if field.pending.is_none()
        && field.key != Some(wanted)
        && let Some(permit) = coordinator.worker(crate::world::streaming::Layer::Contacts)
    {
        field.key = Some(wanted);
        let previous = field.samples.clone();
        field.pending = Some(crate::world::streaming::BuildTask::new(permit, move || {
            build(cell, span, previous)
        }));
    }
}
// Keep half the spare coverage for asynchronous construction. Hysteresis avoids
// rebuilding a 192 KiB image at every texel boundary or when pacing across one.
fn field_cell(position: Vec2, span: f32, distance: f32, previous: Option<(IVec2, u32)>) -> IVec2 {
    let step = span / SIDE as f32;
    let margin = (span * 0.5 - distance - step * 0.5).max(0.0);
    if let Some((cell, bits)) = previous
        && bits == span.to_bits()
        && (position - (cell.as_vec2() + Vec2::splat(0.5)) * step)
            .abs()
            .max_element()
            <= (margin * 0.5).min(16.0)
    {
        return cell;
    }
    (position / step).floor().as_ivec2()
}
fn field_covers(position: Vec2, cell: IVec2, span: f32, distance: f32) -> bool {
    let step = span / SIDE as f32;
    (position - (cell.as_vec2() + Vec2::splat(0.5)) * step)
        .abs()
        .max_element()
        + distance
        + step * 0.5
        <= span * 0.5
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    #[test]
    fn retained_fields_cover_the_entire_view_and_resist_boundary_jitter() {
        for distance in [48.0f32, 260.0, 512.0] {
            let span = (distance * 2.0 + 64.0).max(160.0);
            let mut previous = None;
            for x in -2000..2000 {
                let p = Vec2::new(x as f32 * 0.1, -13.0);
                let cell = field_cell(p, span, distance, previous);
                assert!(field_covers(p, cell, span, distance));
                previous = Some((cell, span.to_bits()));
            }
            let cell = field_cell(Vec2::ZERO, span, distance, None);
            assert_eq!(
                field_cell(
                    Vec2::splat(-0.01),
                    span,
                    distance,
                    Some((cell, span.to_bits()))
                ),
                cell
            );
            assert!(!field_covers(Vec2::splat(1000.0), cell, span, distance));
        }
    }
}

fn build(cell: IVec2, span: f32, previous: Option<Samples>) -> (Image, Transform, Samples) {
    let step = span / SIDE as f32;
    let center = (cell.as_vec2() + Vec2::splat(0.5)) * step;
    let mut values = Vec::with_capacity(SIDE * SIDE);
    let mut data = vec![0u8; SIDE * 2 * SIDE * 3 * 8];
    for z in 0..SIDE {
        for x in 0..SIDE {
            // Address samples by their world-grid index so overlap is bitwise
            // stable even when span is not a power of two.
            let p = (cell.as_vec2()
                + Vec2::new(
                    x as f32 + 1.0 - SIDE as f32 * 0.5,
                    z as f32 + 1.0 - SIDE as f32 * 0.5,
                ))
                * step;
            let old = previous
                .as_ref()
                .filter(|old| old.span == span)
                .and_then(|old| {
                    let q = IVec2::new(x as i32, z as i32) + cell - old.cell;
                    ((0..SIDE as i32).contains(&q.x) && (0..SIDE as i32).contains(&q.y))
                        .then(|| old.values[q.y as usize * SIDE + q.x as usize])
                });
            let (ceiling, snow) = old.unwrap_or_else(|| {
                let mut ceiling = crate::world::terrain::height(p);
                for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                    ceiling = ceiling.min(crate::world::terrain::height(p + d * step));
                }
                (ceiling, crate::world::biome::snow_amount(p))
            });
            values.push((ceiling, snow));
            for axis in 0..3 {
                for sign in 0..2 {
                    // Native irradiance_volume.wgsl samples positive normals
                    // from the first half and negative normals from the second.
                    let sky = if axis == 1 && sign == 0 {
                        Vec3::new(290., 335., 410.)
                    } else if axis == 1 {
                        Vec3::new(100., 110., 100.).lerp(Vec3::splat(190.), snow)
                    } else {
                        Vec3::new(195., 225., 275.)
                    };
                    let scale = if super::lighting::validation_dark() {
                        0.0
                    } else {
                        3.0
                    };
                    let value = [sky.x * scale, sky.y * scale, sky.z * scale, ceiling];
                    let index = ((z + axis * SIDE) * 2 * SIDE + sign * SIDE + x) * 8;
                    for (channel, value) in value.into_iter().enumerate() {
                        let bits = half::f16::from_f32(value).to_le_bytes();
                        data[index + channel * 2..index + channel * 2 + 2].copy_from_slice(&bits);
                    }
                }
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIDE as u32,
            height: 2,
            depth_or_array_layers: (SIDE * 3) as u32,
        },
        TextureDimension::D3,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    (
        image,
        Transform::from_xyz(center.x, 0.0, center.y).with_scale(Vec3::new(span, 8192.0, span)),
        Samples {
            cell,
            span,
            values: std::sync::Arc::new(values),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rolling_field_matches_full_rebuild_across_negative_cells_and_resize() {
        let (_, _, old) = build(IVec2::new(-11, 4), 512.0, None);
        let (a, _, shifted) = build(IVec2::new(-10, 3), 512.0, Some(old));
        let (b, _, _) = build(IVec2::new(-10, 3), 512.0, None);
        assert_eq!(a.data, b.data);
        let (a, _, _) = build(IVec2::new(300, -200), 256.0, Some(shifted));
        let (b, _, _) = build(IVec2::new(300, -200), 256.0, None);
        assert_eq!(a.data, b.data);
    }
}
