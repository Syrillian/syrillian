use crate::ObjectHash;
use crate::rendering::viewport::ViewportId;
use syrillian_shadergen::generator::PICKING_TEXTURE_FORMAT;
use wgpu::{
    Device, Extent3d, SurfaceConfiguration, Texture, TextureDescriptor, TextureDimension,
    TextureUsages, TextureView, TextureViewDescriptor,
};

#[derive(Debug, Clone)]
pub struct PickRequest {
    pub id: u64,
    pub target: ViewportId,
    pub position: (u32, u32),
}

#[derive(Debug, Clone, Copy)]
pub struct PickResult {
    pub id: u64,
    pub target: ViewportId,
    pub hash: Option<ObjectHash>,
}

pub struct PickingSurface {
    texture: Texture,
    view: TextureView,
}

impl PickingSurface {
    pub fn new(device: &Device, config: &SurfaceConfiguration) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Picking Texture"),
            size: Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: PICKING_TEXTURE_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());

        Self { texture, view }
    }

    pub fn recreate(&mut self, device: &Device, config: &SurfaceConfiguration) {
        *self = Self::new(device, config);
    }

    pub fn view(&self) -> &TextureView {
        &self.view
    }

    pub fn texture(&self) -> &Texture {
        &self.texture
    }
}

pub fn hash_to_rgba_bytes(hash: ObjectHash) -> [u8; 4] {
    [
        (hash & 0xff) as u8,
        ((hash >> 8) & 0xff) as u8,
        ((hash >> 16) & 0xff) as u8,
        ((hash >> 24) & 0xff) as u8,
    ]
}

pub fn hash_to_rgba(hash: ObjectHash) -> [f32; 4] {
    let bytes = hash_to_rgba_bytes(hash);
    [
        bytes[0] as f32 / 255.0,
        bytes[1] as f32 / 255.0,
        bytes[2] as f32 / 255.0,
        bytes[3] as f32 / 255.0,
    ]
}

pub fn color_bytes_to_hash(bytes: [u8; 4]) -> Option<ObjectHash> {
    let hash = (bytes[0] as ObjectHash)
        | ((bytes[1] as ObjectHash) << 8)
        | ((bytes[2] as ObjectHash) << 16)
        | ((bytes[3] as ObjectHash) << 24);
    (hash != 0).then_some(hash)
}
