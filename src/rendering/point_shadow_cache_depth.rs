//! Copy-capable point-shadow atlas, bound before Bevy prepares view bindings.
//! Directional and spot shadow targets retain their native resources.
use bevy::{
    pbr::{GlobalClusterableObjectMeta, LightEntity, ShadowView, ViewShadowBindings},
    prelude::*,
    render::{render_resource::*, renderer::RenderDevice, texture::DepthAttachment},
};
#[derive(Component, Clone)]
pub(super) struct Target {
    pub texture: Texture,
    pub layer: u32,
}
#[derive(Resource)]
pub(super) struct Atlas {
    native: TextureId,
    texture: Texture,
    view: TextureView,
    faces: Vec<TextureView>,
}
pub(super) fn prepare(
    mut commands: Commands,
    device: Res<RenderDevice>,
    inputs: Res<super::Inputs>,
    meta: Res<GlobalClusterableObjectMeta>,
    mut atlas: Option<ResMut<Atlas>>,
    mut bindings: Query<&mut ViewShadowBindings>,
    mut views: Query<(Entity, &LightEntity, &mut ShadowView)>,
) {
    // Scenes without opted-in geometry use Bevy's atlas directly. Release the
    // replacement when leaving a den; it must not consume memory outdoors.
    if inputs.0.values().all(|face| face.casters.is_empty()) {
        for (entity, light, _) in &mut views {
            if matches!(light, LightEntity::Point { .. }) {
                commands.entity(entity).remove::<Target>();
            }
        }
        commands.remove_resource::<Atlas>();
        return;
    }
    if !views
        .iter()
        .any(|(_, light, _)| matches!(light, LightEntity::Point { .. }))
    {
        return;
    }
    let Some(first) = bindings.iter().next() else {
        return;
    };
    let native = first.point_light_depth_texture.clone();
    if !bindings
        .iter()
        .all(|b| b.point_light_depth_texture.id() == native.id())
    {
        return;
    }
    let mut replacement = None;
    if atlas.as_ref().is_none_or(|a| a.native != native.id()) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Copy-capable point shadow atlas"),
            size: native.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: native.format(),
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::CubeArray),
            ..default()
        });
        let faces = (0..native.depth_or_array_layers())
            .map(|layer| {
                texture.create_view(&TextureViewDescriptor {
                    dimension: Some(TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..default()
                })
            })
            .collect();
        replacement = Some(Atlas {
            native: native.id(),
            texture,
            view,
            faces,
        });
    }
    let current = replacement.as_ref().or(atlas.as_deref()).unwrap();
    for mut binding in &mut bindings {
        binding.point_light_depth_texture = current.texture.clone();
        binding.point_light_depth_texture_view = current.view.clone();
    }
    for (entity, light, mut shadow) in &mut views {
        let LightEntity::Point {
            light_entity,
            face_index,
        } = light
        else {
            continue;
        };
        let Some(index) = meta.entity_to_index.get(light_entity) else {
            continue;
        };
        let layer = (*index * 6 + face_index) as u32;
        let Some(view) = current.faces.get(layer as usize) else {
            continue;
        };
        shadow.depth_attachment = DepthAttachment::new(view.clone(), Some(0.0));
        commands.entity(entity).insert(Target {
            texture: current.texture.clone(),
            layer,
        });
    }
    if let Some(replacement) = replacement {
        if let Some(atlas) = atlas.as_mut() {
            **atlas = replacement;
        } else {
            commands.insert_resource(replacement);
        }
    }
}
pub(super) struct DepthImage {
    pub texture: Texture,
    pub view: TextureView,
}
impl DepthImage {
    pub fn new(device: &RenderDevice, size: u32) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Static point-shadow face"),
            size: Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&default());
        Self { texture, view }
    }
    pub fn restore(&self, encoder: &mut CommandEncoder, target: &Target) {
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::DepthOnly,
            },
            TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: target.layer,
                },
                aspect: TextureAspect::DepthOnly,
            },
            self.texture.size(),
        );
    }
}
