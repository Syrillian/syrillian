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
        let can_use_block_compression = width % 4 == 0 && height % 4 == 0;
        let original_format = image_data.format;

        let mut format = match original_format {
            Format::R8 => TextureFormat::R8Unorm,
            Format::R8G8 => TextureFormat::Rg8Unorm,
            Format::R8G8B8 => {
                if can_use_block_compression {
                    TextureFormat::Bc1RgbaUnorm
                } else {
                    TextureFormat::Rgba8Unorm
                }
            }
            Format::R8G8B8A8 => {
                if can_use_block_compression {
                    TextureFormat::Bc3RgbaUnorm
                } else {
                    TextureFormat::Rgba8Unorm
                }
            }
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

            if can_use_block_compression {
                // TODO: Switch to Compute GPU Compression
                compress_rgba_blocks(CompressionVariant::BC1, &data, width, height)
            } else {
                data
            }
        } else if original_format == Format::R8G8B8A8 {
            if can_use_block_compression {
                // TODO: Switch to Compute GPU Compression
                compress_rgba_blocks(CompressionVariant::BC3, pixels, width, height)
            } else {
                pixels.to_vec()
            }
        } else {
            pixels.to_vec()
        };

        let (block_width, block_height) = format.block_dimensions();
        let block_size = format.block_copy_size(None).unwrap() as usize;
        let blocks_x = width.div_ceil(block_width) as usize;
        let blocks_y = height.div_ceil(block_height) as usize;
        let expected_size = blocks_x * blocks_y * block_size;

        debug_assert_eq!(
            data.len(),
            expected_size,
            "Data size of a {width} x {height} texture in format {format:?} did not match expectations. Original was: {original_format:?}",
        );

        Some(Texture2D::load_pixels(data, width, height, format))
    }
}

fn compress_rgba_blocks(
    variant: CompressionVariant,
    rgba_pixels: &[u8],
    width: u32,
    height: u32,
) -> Vec<u8> {
    debug_assert_eq!(width % 4, 0, "BC compression requires width multiple of 4");
    debug_assert_eq!(
        height % 4,
        0,
        "BC compression requires height multiple of 4"
    );

    let mut compressed = vec![0u8; variant.blocks_byte_size(width, height)];
    compress_rgba8(
        variant,
        rgba_pixels,
        &mut compressed,
        width,
        height,
        width * 4,
    );
    compressed
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
