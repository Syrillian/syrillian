use crate::mesh::UnskinnedVertex3D;
use wgpu::{
    BlendState, ColorTargetState, ColorWrites, TextureFormat, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexStepMode,
};

pub const DEFAULT_VBL: [VertexBufferLayout; 1] = [UnskinnedVertex3D::continuous_descriptor()];
pub const DEFAULT_VBL_STEP_INSTANCE: [VertexBufferLayout; 1] = {
    let mut continuous = UnskinnedVertex3D::continuous_descriptor();
    continuous.step_mode = VertexStepMode::Instance;
    [continuous]
};

pub const DEFAULT_COLOR_TARGETS: &[Option<ColorTargetState>] = &[
    Some(ColorTargetState {
        format: TextureFormat::Rgba8Unorm, // color
        blend: Some(BlendState::ALPHA_BLENDING),
        write_mask: ColorWrites::all(),
    }),
    Some(ColorTargetState {
        format: TextureFormat::Rg16Float, // normal
        blend: Some(BlendState::REPLACE),
        write_mask: ColorWrites::all(),
    }),
    Some(ColorTargetState {
        format: TextureFormat::Bgra8Unorm, // material
        blend: Some(BlendState::REPLACE),
        write_mask: ColorWrites::all(),
    }),
];

pub const ONLY_COLOR_TARGET: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: TextureFormat::Rgba8Unorm,
    blend: Some(BlendState::ALPHA_BLENDING),
    write_mask: ColorWrites::all(),
})];

pub const ONLY_COLOR_TARGET_SRGB: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: TextureFormat::Bgra8UnormSrgb,
    blend: Some(BlendState::ALPHA_BLENDING),
    write_mask: ColorWrites::all(),
})];

pub const DEFAULT_PP_COLOR_TARGETS: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: TextureFormat::Rgba8Unorm,
    blend: None,
    write_mask: ColorWrites::all(),
})];

pub const SURFACE_PP_COLOR_TARGETS: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: TextureFormat::Bgra8UnormSrgb,
    blend: None,
    write_mask: ColorWrites::all(),
})];

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleVertex {
    pub world_pos_alive: [f32; 4],
    pub life_t: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

pub const PARTICLE_VERTEX_LAYOUT: &[VertexBufferLayout] = &[VertexBufferLayout {
    array_stride: size_of::<ParticleVertex>() as u64,
    step_mode: VertexStepMode::Vertex,
    attributes: &[
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: 0,
            shader_location: 0,
        },
        VertexAttribute {
            format: VertexFormat::Float32,
            offset: 16,
            shader_location: 1,
        },
    ],
}];
