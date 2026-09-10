//! CPU mip chains for procedural RGBA8 textures. One 2D layer, largest first.
use bevy::prelude::*;

#[derive(Clone, Copy)]
pub(crate) enum Filter {
    Color,
    Normal,
    Linear,
}
fn decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn encode(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}
pub(crate) fn generate(image: &mut Image, filter: Filter) {
    let (mut w, mut h) = (image.width(), image.height());
    let data = image
        .data
        .as_mut()
        .expect("procedural image has CPU pixels");
    data.truncate((w * h * 4) as usize);
    let mut offset = 0;
    let mut levels = 1;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let next = data.len();
        for y in 0..nh {
            for x in 0..nw {
                let mut sum = Vec4::ZERO;
                let mut count = 0.;
                for sy in y * h / nh..(y + 1) * h / nh {
                    for sx in x * w / nw..(x + 1) * w / nw {
                        let i = offset + ((sy * w + sx) * 4) as usize;
                        let mut p =
                            Vec4::from_array(std::array::from_fn(|c| data[i + c] as f32 / 255.));
                        match filter {
                            Filter::Color => {
                                p.x = decode(p.x);
                                p.y = decode(p.y);
                                p.z = decode(p.z);
                            }
                            Filter::Normal => {
                                p = (p.truncate() * 2. - Vec3::ONE).extend(p.w);
                            }
                            Filter::Linear => {}
                        }
                        sum += p;
                        count += 1.;
                    }
                }
                sum /= count;
                match filter {
                    Filter::Color => {
                        sum.x = encode(sum.x);
                        sum.y = encode(sum.y);
                        sum.z = encode(sum.z);
                    }
                    Filter::Normal => {
                        sum = (sum.truncate().try_normalize().unwrap_or(Vec3::Z) * 0.5
                            + Vec3::splat(0.5))
                        .extend(sum.w);
                    }
                    Filter::Linear => {}
                }
                data.extend(
                    sum.to_array()
                        .map(|v| (v.clamp(0., 1.) * 255.).round() as u8),
                );
            }
        }
        offset = next;
        w = nw;
        h = nh;
        levels += 1;
    }
    image.texture_descriptor.mip_level_count = levels;
}
/// Select an existing correctly filtered level and retain its complete tail.
pub(crate) fn reduce(image: &mut Image, levels: u32) {
    let levels = levels.min(image.texture_descriptor.mip_level_count - 1);
    let (mut w, mut h) = (image.width(), image.height());
    let mut offset = 0;
    for _ in 0..levels {
        offset += (w * h * 4) as usize;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    image.data.as_mut().unwrap().drain(..offset);
    image.texture_descriptor.size.width = w;
    image.texture_descriptor.size.height = h;
    image.texture_descriptor.mip_level_count -= levels;
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    fn image(data: Vec<u8>, w: u32, h: u32) -> Image {
        Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        )
    }
    #[test]
    fn color_is_filtered_in_linear_space_and_reduction_preserves_tail() {
        let mut im = image(vec![0, 0, 0, 255, 255, 255, 255, 255], 2, 1);
        generate(&mut im, Filter::Color);
        assert_eq!(&im.data.as_ref().unwrap()[8..], &[188, 188, 188, 255]);
        reduce(&mut im, 2);
        assert_eq!(
            (
                im.width(),
                im.height(),
                im.texture_descriptor.mip_level_count
            ),
            (1, 1, 1)
        );
        assert_eq!(im.data.unwrap(), [188, 188, 188, 255]);
    }
    #[test]
    fn normals_remain_unit_length_and_odd_dimensions_reach_one() {
        let mut im = image(
            [255, 128, 128, 255, 128, 255, 128, 255, 128, 128, 255, 255].repeat(5),
            3,
            5,
        );
        generate(&mut im, Filter::Normal);
        assert_eq!(im.texture_descriptor.mip_level_count, 3);
        let data = im.data.unwrap();
        assert_eq!(data.len(), (15 + 2 + 1) * 4);
        for p in data[60..].chunks_exact(4) {
            let n = Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 127.5 - Vec3::ONE;
            assert!((n.length() - 1.).abs() < 0.01);
        }
    }
}
