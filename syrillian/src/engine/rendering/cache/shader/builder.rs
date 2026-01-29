use crate::assets::{HBGL, Shader, ShaderType};
use crate::core::Vertex3D;
use wgpu::{
    BlendState, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState,
    Device, Face, FragmentState, MultisampleState, PipelineCompilationOptions, PipelineLayout,
    PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor,
    ShaderModule, StencilFaceState, StencilState, TextureFormat, VertexBufferLayout, VertexState,
    VertexStepMode,
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
        format: TextureFormat::Bgra8Unorm,
        blend: Some(BlendState::REPLACE),
        write_mask: ColorWrites::all(),
    }),
    Some(ColorTargetState {
        format: TextureFormat::Bgra8Unorm,
        blend: Some(BlendState::REPLACE),
        write_mask: ColorWrites::all(),
    }),
];

pub const DEFAULT_PP_COLOR_TARGETS: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
    format: TextureFormat::Bgra8UnormSrgb,
    blend: None,
    write_mask: ColorWrites::all(),
})];

const DEFAULT_DEPTH_STENCIL: DepthStencilState = DepthStencilState {
    format: TextureFormat::Depth32Float,
    depth_write_enabled: true,
    depth_compare: CompareFunction::LessEqual,
    stencil: StencilState {
        front: StencilFaceState::IGNORE,
        back: StencilFaceState::IGNORE,
        read_mask: 0,
        write_mask: 0,
    },
    bias: DepthBiasState {
        constant: 0,
        slope_scale: 0.0,
        clamp: 0.0,
    },
};

const SHADOW_DEPTH_STENCIL: DepthStencilState = DepthStencilState {
    format: TextureFormat::Depth32Float,
    depth_write_enabled: true,
    depth_compare: CompareFunction::LessEqual,
    stencil: StencilState {
        front: StencilFaceState::IGNORE,
        back: StencilFaceState::IGNORE,
        read_mask: 0,
        write_mask: 0,
    },
    bias: DepthBiasState {
        constant: 2,
        slope_scale: 1.5,
        clamp: 0.0,
    },
};

// TODO: this is all very assumptious and static. I feel like, the shader should actually be a builder
//       abstraction over the wgpu RenderPipelineDescriptor and the Shader and stuff.
pub struct RenderPipelineBuilder<'a> {
    pub label: String,
    pub layout: &'a PipelineLayout,
    pub module: &'a ShaderModule,
    pub is_post_process: bool,
    pub has_depth: bool,
    pub polygon_mode: PolygonMode,
    pub topology: PrimitiveTopology,
    pub vertex_buffers: &'a [VertexBufferLayout<'a>],
    pub is_custom: bool,
    pub has_shadow_transparency: bool,
    pub color_target: &'a [Option<ColorTargetState>],
}

impl<'a> RenderPipelineBuilder<'a> {
    pub fn build(&'a self, device: &Device) -> RenderPipeline {
        device.create_render_pipeline(&self.desc())
    }

    pub fn build_shadow(&'a self, device: &Device) -> Option<RenderPipeline> {
        Some(device.create_render_pipeline(&self.shadow_desc()?))
    }

    pub fn cull_mode(&self) -> Option<Face> {
        (!self.is_custom && !self.is_post_process).then_some(Face::Back)
    }

    pub fn desc(&'a self) -> RenderPipelineDescriptor<'a> {
        let depth_stencil =
            (!self.is_post_process && self.has_depth).then_some(DEFAULT_DEPTH_STENCIL);
        let cull_mode = (!self.is_custom && !self.is_post_process).then_some(Face::Back);

        RenderPipelineDescriptor {
            label: Some(&self.label),
            layout: Some(self.layout),

            vertex: VertexState {
                module: self.module,
                entry_point: None,
                compilation_options: PipelineCompilationOptions::default(),
                buffers: self.vertex_buffers,
            },
            primitive: PrimitiveState {
                topology: self.topology,
                cull_mode,
                polygon_mode: self.polygon_mode,
                ..PrimitiveState::default()
            },
            depth_stencil,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: self.module,
                entry_point: None,
                compilation_options: PipelineCompilationOptions::default(),
                targets: self.color_target,
            }),
            multiview_mask: None,
            cache: None,
        }
    }

    pub fn shadow_desc(&'a self) -> Option<RenderPipelineDescriptor<'a>> {
        if self.is_post_process || !self.has_depth {
            return None;
        }

        let cull_mode = self.cull_mode();

        let fragment = self.has_shadow_transparency.then(|| FragmentState {
            module: self.module,
            entry_point: None,
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[],
        });

        Some(RenderPipelineDescriptor {
            label: Some(&self.label),
            layout: Some(self.layout),

            vertex: VertexState {
                module: self.module,
                entry_point: None,
                compilation_options: PipelineCompilationOptions::default(),
                buffers: self.vertex_buffers,
            },
            primitive: PrimitiveState {
                topology: self.topology,
                cull_mode,
                polygon_mode: self.polygon_mode,
                ..PrimitiveState::default()
            },
            depth_stencil: Some(SHADOW_DEPTH_STENCIL),
            multisample: MultisampleState::default(),
            fragment,
            multiview_mask: None,
            cache: None,
        })
    }

    pub fn builder(
        shader: &Shader,
        layout: &'a PipelineLayout,
        module: &'a ShaderModule,
    ) -> RenderPipelineBuilder<'a> {
        let name = shader.name();
        let polygon_mode = shader.polygon_mode();
        let topology = shader.topology();
        let is_post_process = shader.is_post_process();
        let is_custom = shader.is_custom();
        let has_shadow_transparency = shader.has_shadow_transparency();
        let has_depth = shader.is_depth_enabled();
        let color_target = shader.color_target();

        debug_assert!(
            !has_shadow_transparency || !shader.needs_bgl(HBGL::SHADOW),
            "Cannot render shadow transparency while relying on shadows for rendering"
        );

        let label = match shader.stage() {
            ShaderType::Default => format!("{name} Pipeline"),
            ShaderType::PostProcessing => format!("{name} Post Process Pipeline"),
            ShaderType::Custom => format!("{name} Custom Pipeline"),
        };

        let vertex_buffers = match shader.stage() {
            ShaderType::Default => &DEFAULT_VBL,
            ShaderType::Custom => shader.vertex_buffers(),
            ShaderType::PostProcessing => &[],
        };

        RenderPipelineBuilder {
            label,
            layout,
            module,
            is_post_process,
            has_depth,
            is_custom,
            has_shadow_transparency,
            polygon_mode,
            topology,
            vertex_buffers,
            color_target,
        }
    }
}
