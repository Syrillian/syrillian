use crate::math::Vec3;
use crossbeam_channel::bounded;
use half::f16;
use image::{ColorType, ImageFormat};
use rapier3d::prelude::Vec2;
use snafu::{OptionExt, Snafu};
use std::path::Path;
use syrillian_utils::debug_panic;
use wgpu::{
    BufferDescriptor, BufferUsages, COPY_BYTES_PER_ROW_ALIGNMENT, Device, Extent3d, MapMode,
    Origin3d, PollType, Queue, TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo,
    Texture, TextureAspect, TextureFormat,
};

#[derive(Debug, Snafu)]
pub enum TextureExportError {
    #[snafu(display("Unsupported texture format {:?} for export", format))]
    UnsupportedFormat { format: TextureFormat },

    #[snafu(display("Cannot export empty texture: {width}x{height}"))]
    InvalidDimensions { width: u32, height: u32 },

    #[snafu(display("Failed to map export buffer: {source:?}"))]
    Map { source: wgpu::BufferAsyncError },

    #[snafu(display("Failed to map export buffer: channel closed"))]
    MapChannelClosed,

    #[snafu(display("Failed to write image: {source}"))]
    Image { source: image::ImageError },

    #[snafu(display("Export source unavailable: {reason}"))]
    Unavailable { reason: &'static str },
}

fn is_supported(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::Rgba8Unorm
            | TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Bgra8Unorm
            | TextureFormat::Bgra8UnormSrgb
            | TextureFormat::Rg16Float
            | TextureFormat::Depth32Float
    )
}

fn rg16f_to_oct(bytes: [u8; 4]) -> Vec2 {
    let u = f16::from_le_bytes([bytes[0], bytes[1]]);
    let v = f16::from_le_bytes([bytes[2], bytes[3]]);
    Vec2::new(u.to_f32(), v.to_f32())
}

fn oct_decode(mut e: Vec2) -> Vec3 {
    e = e.clamp(Vec2::splat(-1.0), Vec2::splat(1.0));

    let mut v = Vec3::new(e.x, e.y, 1.0 - e.x.abs() - e.y.abs());

    if v.z < 0.0 {
        let x = v.x;
        let y = v.y;
        v.x = (1.0 - y.abs()) * if x >= 0.0 { 1.0 } else { -1.0 };
        v.y = (1.0 - x.abs()) * if y >= 0.0 { 1.0 } else { -1.0 };
    }

    v.normalize()
}

fn normal_from_rg16float_oct(bytes: [u8; 4]) -> Vec3 {
    let e = rg16f_to_oct(bytes);
    oct_decode(e)
}

fn linearize_depth_01(z: f32, near: f32, far: f32) -> f32 {
    (near * far) / (far - z * (far - near))
}

/// Reads a texture into an RGBA8 buffer (no gamma conversion) and strips row padding.
pub fn read_texture_as_rgba(
    device: &Device,
    queue: &Queue,
    texture: &Texture,
) -> Result<Vec<u8>, TextureExportError> {
    let height = texture.height();
    let width = texture.width();
    let format = texture.format();

    if width == 0 || height == 0 {
        return Err(TextureExportError::InvalidDimensions { width, height });
    }

    if !is_supported(format) {
        return Err(TextureExportError::UnsupportedFormat { format });
    }

    let bytes_per_pixel: u32 = format
        .block_copy_size(None)
        .with_context(|| UnsupportedFormatSnafu { format })?;

    let bytes_per_row = bytes_per_pixel * width;
    let padded_bytes_per_row =
        bytes_per_row.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;

    let buffer_size = padded_bytes_per_row as u64 * height as u64;

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Texture Export Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Texture Export Encoder"),
    });

    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let slice = buffer.slice(..);
    let (tx, rx) = bounded(1);
    slice.map_async(MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    let _ = device.poll(PollType::wait_indefinitely());

    match rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(source)) => return Err(TextureExportError::Map { source }),
        Err(_) => return Err(TextureExportError::MapChannelClosed),
    }

    let data = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);

    const NEAR: f32 = 0.1;
    const FAR: f32 = 500.0;

    for row in 0..height as usize {
        let start = row * padded_bytes_per_row as usize;
        let end = start + bytes_per_row as usize;

        let row_data = &data[start..end];
        if format == TextureFormat::Rg16Float {
            let (chunks, leftover) = row_data.as_chunks::<4>();
            debug_assert!(leftover.is_empty());

            for chunk in chunks {
                let normal = ((normal_from_rg16float_oct(*chunk) + 1.0) / 2.0)
                    .clamp(Vec3::splat(0.0), Vec3::splat(1.0));
                let mapped = [
                    (normal.x * 255.0) as u8,
                    (normal.y * 255.0) as u8,
                    (normal.z * 255.0) as u8,
                    u8::MAX,
                ];
                pixels.extend_from_slice(&mapped)
            }
        } else if bytes_per_pixel == 4 && format.has_color_aspect() {
            pixels.extend_from_slice(&data[start..end]);
        } else if format == TextureFormat::Depth32Float {
            let (chunks, leftover) = row_data.as_chunks::<4>();
            debug_assert!(leftover.is_empty());

            for chunk in chunks {
                let z = f32::from_le_bytes(*chunk); // 0..1 depth buffer value
                let lin = linearize_depth_01(z, NEAR, FAR); // view-space depth (positive)
                let t = ((lin - NEAR) / (FAR - NEAR)).clamp(0.0, 1.0);
                let g = ((1.0 - t) * 255.0).round() as u8;
                pixels.extend_from_slice(&[g, g, g, 255]);
            }
        } else {
            debug_panic!("Set format {format:?} as supported, but not actually supported.");
            return Err(TextureExportError::UnsupportedFormat { format });
        }
    }

    drop(data);
    buffer.unmap();

    if matches!(
        format,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb
    ) {
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
    }

    Ok(pixels)
}

pub fn save_texture_to_png(
    device: &Device,
    queue: &Queue,
    texture: &Texture,
    path: impl AsRef<Path>,
) -> Result<(), TextureExportError> {
    let pixels = read_texture_as_rgba(device, queue, texture)?;

    let height = texture.height();
    let width = texture.width();

    image::save_buffer_with_format(
        path,
        &pixels,
        width,
        height,
        ColorType::Rgba8,
        ImageFormat::Png,
    )
    .map_err(|source| TextureExportError::Image { source })
}
