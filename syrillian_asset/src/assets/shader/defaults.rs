use syrillian_utils::sizes::{VEC2_SIZE, VEC3_SIZE};
use wgpu::{
    BlendState, ColorTargetState, ColorWrites, TextureFormat, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexStepMode,
};

pub const PICKING_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

pub const DEFAULT_VBL: [VertexBufferLayout; 4] = [
    VertexBufferLayout {
        array_stride: VEC3_SIZE,
        step_mode: VertexStepMode::Vertex,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }],
    },
    VertexBufferLayout {
        array_stride: VEC2_SIZE,
        step_mode: VertexStepMode::Vertex,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: 0,
            shader_location: 1,
        }],
    },
    VertexBufferLayout {
        array_stride: VEC3_SIZE,
        step_mode: VertexStepMode::Vertex,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 2,
        }],
    },
    VertexBufferLayout {
        array_stride: VEC3_SIZE,
        step_mode: VertexStepMode::Vertex,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 3,
        }],
    },
];

pub const DEFAULT_VBL_STEP_INSTANCE: [VertexBufferLayout; 4] = [
    VertexBufferLayout {
        array_stride: VEC3_SIZE,
        step_mode: VertexStepMode::Instance,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }],
    },
    VertexBufferLayout {
        array_stride: VEC2_SIZE,
        step_mode: VertexStepMode::Instance,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: 0,
            shader_location: 1,
        }],
    },
    VertexBufferLayout {
        array_stride: VEC3_SIZE,
        step_mode: VertexStepMode::Instance,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 2,
        }],
    },
    VertexBufferLayout {
        array_stride: VEC3_SIZE,
        step_mode: VertexStepMode::Instance,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 3,
        }],
    },
];

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

pub const PICKING_COLOR_TARGET: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: PICKING_TEXTURE_FORMAT,
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
