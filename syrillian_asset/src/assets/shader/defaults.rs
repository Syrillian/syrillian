use crate::mesh::Vertex3D;
use wgpu::{
    BlendState, ColorTargetState, ColorWrites, TextureFormat, VertexBufferLayout, VertexStepMode,
};

pub const DEFAULT_VBL: [VertexBufferLayout; 1] = [Vertex3D::continuous_descriptor()];
pub const DEFAULT_VBL_STEP_INSTANCE: [VertexBufferLayout; 1] = {
    let mut continuous = Vertex3D::continuous_descriptor();
    continuous.step_mode = VertexStepMode::Instance;
    [continuous]
};

pub const DEFAULT_COLOR_TARGETS: &[Option<ColorTargetState>] = &[
    Some(ColorTargetState {
        format: TextureFormat::Bgra8UnormSrgb,
        blend: Some(BlendState::ALPHA_BLENDING),
        write_mask: ColorWrites::all(),
    }),
    Some(ColorTargetState {
        format: TextureFormat::Rg16Float,
        blend: Some(BlendState::REPLACE),
        write_mask: ColorWrites::all(),
    }),
    Some(ColorTargetState {
        format: TextureFormat::Bgra8Unorm,
        blend: Some(BlendState::REPLACE),
        write_mask: ColorWrites::all(),
    }),
];

pub const ONLY_COLOR_TARGET: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: TextureFormat::Bgra8UnormSrgb,
    blend: Some(BlendState::ALPHA_BLENDING),
    write_mask: ColorWrites::all(),
})];

pub const DEFAULT_PP_COLOR_TARGETS: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: TextureFormat::Bgra8UnormSrgb,
    blend: None,
    write_mask: ColorWrites::all(),
})];
