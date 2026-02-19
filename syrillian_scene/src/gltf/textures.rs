use crate::GltfScene;
use block_compression::CompressionVariant;
use block_compression::encode::compress_rgba8;
use gltf::image::Format;
use std::collections::HashMap;
use syrillian::assets::Texture2D;
use syrillian::rendering::rendering::TextureFormat;
use syrillian_utils::debug_panic;

// gonna be used later
#[derive(Copy, Clone)]
pub enum TextureTypeInfo {
    BaseColor,
    Emissive,
    Normal,
    MetallicRoughness,
    Occlusion,
}

#[derive(Copy, Clone)]
pub struct TextureUsageInfo {
    srgb: bool,
    _texture_type: TextureTypeInfo,
}

impl TextureUsageInfo {
    pub fn color_srgb() -> Self {
        Self {
            srgb: true,
            _texture_type: TextureTypeInfo::BaseColor,
        }
    }

    pub fn color() -> Self {
        Self {
            srgb: false,
            _texture_type: TextureTypeInfo::BaseColor,
        }
    }

    pub fn normal() -> Self {
        Self {
            srgb: false,
            _texture_type: TextureTypeInfo::Normal,
        }
    }

    pub fn metallic_roughness() -> Self {
        Self {
            srgb: false,
            _texture_type: TextureTypeInfo::MetallicRoughness,
        }
    }
}

impl GltfScene {
    pub fn decode_texture(
        &self,
        texture: &gltf::Texture,
        usage: TextureUsageInfo,
    ) -> Option<Texture2D> {
        let image = texture.source();
        let index = image.index();
        let image_data = &self.images[index];

        let (width, height) = (image_data.width, image_data.height);
        let original_format = image_data.format;

        let mut format = match original_format {
            Format::R8 => TextureFormat::R8Unorm,
            Format::R8G8 => TextureFormat::Rg8Unorm,
            Format::R8G8B8 => TextureFormat::Bc1RgbaUnorm,
            Format::R8G8B8A8 => TextureFormat::Bc3RgbaUnorm,
            Format::R16 => TextureFormat::R16Unorm,
            Format::R16G16 => TextureFormat::Rg16Snorm,
            Format::R16G16B16 => {
                debug_panic!("Cannot use RGB16 (no alpha) Texture");
                return None;
            }
            Format::R16G16B16A16 => TextureFormat::Rgba16Unorm,
            Format::R32G32B32FLOAT => {
                debug_panic!("Cannot use RGB32 (no alpha) Texture");
                return None;
            }
            Format::R32G32B32A32FLOAT => TextureFormat::Rgba32Float,
        };

        if usage.srgb {
            format = format.add_srgb_suffix();
        }

        let pixels = &image_data.pixels[..];
        let data = if original_format == Format::R8G8B8 {
            let mut data = Vec::with_capacity(pixels.len() / 3 * 4);
            for rgb in pixels.chunks(3) {
                data.extend(rgb);
                data.push(255);
            }

            // TODO: Switch to Compute GPU Compression
            let mut new_data = vec![0u8; CompressionVariant::BC1.blocks_byte_size(width, height)];
            compress_rgba8(
                CompressionVariant::BC1,
                &data,
                &mut new_data,
                width,
                height,
                width * 4,
            );
            new_data
        } else if original_format == Format::R8G8B8A8 {
            // TODO: Switch to Compute GPU Compression
            let mut new_data = vec![0u8; CompressionVariant::BC3.blocks_byte_size(width, height)];
            compress_rgba8(
                CompressionVariant::BC3,
                pixels,
                &mut new_data,
                width,
                height,
                width * 4,
            );
            new_data
        } else {
            pixels.to_vec()
        };

        debug_assert_eq!(
            data.len(),
            width as usize * height as usize
                / (format.block_dimensions().0 * format.block_dimensions().1) as usize
                * format.block_copy_size(None).unwrap() as usize,
            "Data size of a {width} x {height} texture in format {format:?} did not match expectations. Original was: {original_format:?}",
        );

        Some(Texture2D::load_pixels(data, width, height, format))
    }
}

pub(super) fn collect_material_texture_usage(
    material: gltf::Material,
    usage: &mut HashMap<usize, TextureUsageInfo>,
) {
    let pbr = material.pbr_metallic_roughness();

    if let Some(info) = pbr.base_color_texture() {
        usage
            .entry(info.texture().index())
            .or_insert_with(|| TextureUsageInfo {
                srgb: true,
                _texture_type: TextureTypeInfo::BaseColor,
            });
    }

    if let Some(info) = material.emissive_texture() {
        usage
            .entry(info.texture().index())
            .or_insert_with(|| TextureUsageInfo {
                srgb: true,
                _texture_type: TextureTypeInfo::Emissive,
            });
    }

    if let Some(info) = material.normal_texture() {
        usage
            .entry(info.texture().index())
            .or_insert_with(|| TextureUsageInfo {
                srgb: false,
                _texture_type: TextureTypeInfo::Normal,
            });
    }

    if let Some(info) = pbr.metallic_roughness_texture() {
        usage
            .entry(info.texture().index())
            .or_insert_with(|| TextureUsageInfo {
                srgb: false,
                _texture_type: TextureTypeInfo::MetallicRoughness,
            });
    }

    if let Some(info) = material.occlusion_texture() {
        usage
            .entry(info.texture().index())
            .or_insert_with(|| TextureUsageInfo {
                srgb: false,
                _texture_type: TextureTypeInfo::Occlusion,
            });
    }
}
