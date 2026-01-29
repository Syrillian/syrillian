use crate::engine::rendering::uniform::ShaderUniform;
use syrillian_macros::UniformIndex;
use wgpu::{
    AddressMode, BindGroupLayout, Device, FilterMode, MipmapFilterMode, SamplerDescriptor,
    TextureView,
};

#[repr(u8)]
#[derive(Debug, Copy, Clone, UniformIndex)]
pub enum PostProcessUniformIndex {
    Color = 0,
    Sampler = 1,
    Depth = 2,
    GNormal = 3,
    GMaterial = 4,
}

pub struct PostProcessData {
    pub(crate) uniform: ShaderUniform<PostProcessUniformIndex>,
}

impl PostProcessData {
    pub(crate) fn new(
        device: &Device,
        layout: BindGroupLayout,
        color_view: TextureView,
        depth_view: TextureView,
        g_normal_view: TextureView,
        g_material_view: TextureView,
    ) -> Self {
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("PostProcess Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..SamplerDescriptor::default()
        });

        let uniform = ShaderUniform::<PostProcessUniformIndex>::builder(layout)
            .with_texture(color_view.clone())
            .with_sampler(sampler)
            .with_texture(depth_view)
            .with_texture(g_normal_view)
            .with_texture(g_material_view)
            .build(device);

        Self { uniform }
    }
}
