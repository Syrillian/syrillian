mod shader_gen;
pub(crate) use shader_gen::ShaderGen;

// this module only has tests for the built-in shaders and can be safely ignored
#[cfg(test)]
mod shaders;

use crate::assets::HBGL;
use crate::engine::assets::generic_store::{HandleName, Store, StoreDefaults, StoreType};
use crate::engine::assets::{H, HShader, StoreTypeFallback, StoreTypeName};
use crate::rendering::proxies::text_proxy::TextImmediates;
use crate::rendering::{
    AssetCache, DEFAULT_COLOR_TARGETS, DEFAULT_PP_COLOR_TARGETS, DEFAULT_VBL,
    PICKING_TEXTURE_FORMAT,
};
use crate::utils::sizes::{VEC2_SIZE, VEC4_SIZE};
use crate::{store_add_checked, store_add_checked_many};
use bon::Builder;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use wgpu::{
    BindGroupLayout, ColorTargetState, ColorWrites, Device, PipelineLayout,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveTopology, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexStepMode,
};

#[cfg(debug_assertions)]
use crate::rendering::DEFAULT_VBL_STEP_INSTANCE;

#[derive(Debug, Clone)]
pub enum ShaderCode {
    Full(String),
    Fragment(String),
}

impl ShaderCode {
    pub fn is_only_fragment_shader(&self) -> bool {
        matches!(self, ShaderCode::Fragment(_))
    }

    pub fn code(&self) -> &str {
        match self {
            ShaderCode::Full(code) => code,
            ShaderCode::Fragment(code) => code,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ShaderType {
    Default,
    Custom,
    PostProcessing,
}

#[derive(Debug, Clone, Builder)]
pub struct Shader {
    #[builder(into)]
    name: String,
    code: ShaderCode,
    #[builder(default = PolygonMode::Fill)]
    polygon_mode: PolygonMode,
    #[builder(default = PrimitiveTopology::TriangleList)]
    topology: PrimitiveTopology,
    #[builder(default = &DEFAULT_VBL)]
    vertex_buffers: &'static [VertexBufferLayout<'static>],
    #[builder(default = DEFAULT_COLOR_TARGETS)]
    color_target: &'static [Option<ColorTargetState>],
    #[builder(default = 0)]
    immediate_size: u32,
    #[builder(default = false)]
    shadow_transparency: bool,
    #[builder(default = true)]
    depth_enabled: bool,
    shader_type: ShaderType,
}

#[derive(Debug, Clone, Default)]
pub struct BindGroupMap {
    pub render: u32,
    pub model: Option<u32>,
    pub material: Option<u32>,
    pub light: Option<u32>,
    pub shadow: Option<u32>,
    pub post_process: Option<u32>,
}

impl BindGroupMap {
    pub fn new(render: u32) -> Self {
        BindGroupMap {
            render,
            ..Self::default()
        }
    }
}

impl H<Shader> {
    pub const FALLBACK_ID: u32 = 0;
    pub const DIM2_ID: u32 = 1;
    pub const DIM3_ID: u32 = 2;
    pub const POST_PROCESS_ID: u32 = 3;
    pub const DIM2_PICKER_ID: u32 = 4;
    pub const DIM3_PICKER_ID: u32 = 5;
    pub const TEXT_2D_ID: u32 = 6;
    pub const TEXT_2D_PICKER_ID: u32 = 7;
    pub const TEXT_3D_ID: u32 = 8;
    pub const TEXT_3D_PICKER_ID: u32 = 9;

    pub const DEBUG_EDGES_ID: u32 = 10;
    pub const DEBUG_VERTEX_NORMALS_ID: u32 = 11;
    pub const DEBUG_LINES_ID: u32 = 12;
    pub const DEBUG_TEXT2D_GEOMETRY_ID: u32 = 13;
    pub const DEBUG_TEXT3D_GEOMETRY_ID: u32 = 14;
    pub const DEBUG_LIGHT_ID: u32 = 15;
    pub const MAX_BUILTIN_ID: u32 = 15;

    // The fallback shader if a pipeline fails
    pub const FALLBACK: H<Shader> = H::new(Self::FALLBACK_ID);

    // The default 2D shader.
    pub const DIM2: H<Shader> = H::new(Self::DIM2_ID);

    // The default 3D shader.
    pub const DIM3: H<Shader> = H::new(Self::DIM3_ID);

    // The default 2D picking shader.
    pub const DIM2_PICKING: H<Shader> = H::new(Self::DIM2_PICKER_ID);

    // The default 3D picking shader.
    pub const DIM3_PICKING: H<Shader> = H::new(Self::DIM3_PICKER_ID);

    // Default post-processing shader
    pub const POST_PROCESS: H<Shader> = H::new(Self::POST_PROCESS_ID);

    // Default 2D Text shader.
    pub const TEXT_2D: H<Shader> = H::new(Self::TEXT_2D_ID);

    // Default 2D Text picking shader.
    pub const TEXT_2D_PICKING: H<Shader> = H::new(Self::TEXT_2D_PICKER_ID);

    // Default 3D Text shader.
    pub const TEXT_3D: H<Shader> = H::new(Self::TEXT_3D_ID);

    // Default 3D Text picking shader.
    pub const TEXT_3D_PICKING: H<Shader> = H::new(Self::TEXT_3D_PICKER_ID);

    // An addon shader ID that is used for drawing debug edges on meshes
    pub const DEBUG_EDGES: H<Shader> = H::new(Self::DEBUG_EDGES_ID);

    // An addon shader ID that is used for drawing debug vertex normals on meshes
    pub const DEBUG_VERTEX_NORMALS: H<Shader> = H::new(Self::DEBUG_VERTEX_NORMALS_ID);

    // An addon shader ID that is used for drawing debug lines
    pub const DEBUG_LINES: H<Shader> = H::new(Self::DEBUG_LINES_ID);
    pub const DEBUG_TEXT2D_GEOMETRY: H<Shader> = H::new(Self::DEBUG_TEXT2D_GEOMETRY_ID);
    pub const DEBUG_TEXT3D_GEOMETRY: H<Shader> = H::new(Self::DEBUG_TEXT3D_GEOMETRY_ID);
    pub const DEBUG_LIGHT: H<Shader> = H::new(Self::DEBUG_LIGHT_ID);
}

const SHADER_FALLBACK3D: &str = include_str!("shaders/fallback_shader3d.wgsl");
const SHADER_DIM2: &str = include_str!("shaders/shader2d.wgsl");
const SHADER_DIM3: &str = include_str!("shaders/shader3d.wgsl");
const SHADER_DIM2_PICKER: &str = include_str!("shaders/picking_ui.wgsl");
const SHADER_DIM3_PICKER: &str = include_str!("shaders/picking_mesh.wgsl");
const SHADER_TEXT2D: &str = include_str!("shaders/text2d.wgsl");
const SHADER_TEXT2D_PICKER: &str = include_str!("shaders/picking_text2d.wgsl");
const SHADER_TEXT3D: &str = include_str!("shaders/text3d.wgsl");
const SHADER_TEXT3D_PICKER: &str = include_str!("shaders/picking_text3d.wgsl");
const SHADER_FS_COPY: &str = include_str!("shaders/fullscreen_passthrough.wgsl");

#[cfg(debug_assertions)]
const DEBUG_EDGES_SHADER: &str = include_str!("shaders/debug/edges.wgsl");
#[cfg(debug_assertions)]
const DEBUG_VERTEX_NORMAL_SHADER: &str = include_str!("shaders/debug/vertex_normals.wgsl");
#[cfg(debug_assertions)]
const DEBUG_LINES_SHADER: &str = include_str!("shaders/debug/lines.wgsl");
#[cfg(debug_assertions)]
const DEBUG_TEXT2D_GEOMETRY: &str = include_str!("shaders/debug/text2d_geometry.wgsl");
#[cfg(debug_assertions)]
const DEBUG_TEXT3D_GEOMETRY: &str = include_str!("shaders/debug/text3d_geometry.wgsl");
#[cfg(debug_assertions)]
const DEBUG_LIGHT_SHADER: &str = include_str!("shaders/debug/light.wgsl");

impl StoreDefaults for Shader {
    fn populate(store: &mut Store<Self>) {
        store_add_checked_many!(store,
            HShader::FALLBACK_ID => Shader::new_default("Fallback", SHADER_FALLBACK3D),
            HShader::DIM2_ID => Shader::new_default("2D Default", SHADER_DIM2)
                .with_depth_enabled(false),
            HShader::DIM3_ID => Shader::new_fragment("3D Default", SHADER_DIM3),
            HShader::POST_PROCESS_ID => Shader::new_post_process("Post Process", SHADER_FS_COPY),
        );

        const TEXT_VBL: &[VertexBufferLayout] = &[VertexBufferLayout {
            array_stride: VEC2_SIZE * 2,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: VEC2_SIZE,
                    shader_location: 1,
                },
            ],
        }];

        const PICKING_COLOR_TARGET: &[Option<ColorTargetState>] = &[Some(ColorTargetState {
            format: PICKING_TEXTURE_FORMAT,
            blend: None,
            write_mask: ColorWrites::all(),
        })];

        store_add_checked!(
            store,
            HShader::DIM2_PICKER_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Default 2D Picking Shader")
                .code(ShaderCode::Full(SHADER_DIM2_PICKER.to_string()))
                .immediate_size(VEC4_SIZE as u32)
                .depth_enabled(false)
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM3_PICKER_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Default 3D Picking Shader")
                .code(ShaderCode::Fragment(SHADER_DIM3_PICKER.to_string()))
                .immediate_size(VEC4_SIZE as u32)
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        store_add_checked!(
            store,
            HShader::TEXT_2D_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Text 2D Shader")
                .code(ShaderCode::Full(SHADER_TEXT2D.to_string()))
                .vertex_buffers(TEXT_VBL)
                .immediate_size(size_of::<TextImmediates>() as u32)
                .depth_enabled(false)
                .build()
        );

        store_add_checked!(
            store,
            HShader::TEXT_2D_PICKER_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Text 2D Picking Shader")
                .code(ShaderCode::Full(SHADER_TEXT2D_PICKER.to_string()))
                .vertex_buffers(TEXT_VBL)
                .immediate_size(size_of::<TextImmediates>() as u32)
                .depth_enabled(false)
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        store_add_checked!(
            store,
            HShader::TEXT_3D_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Text 3D Shader")
                .code(ShaderCode::Full(SHADER_TEXT3D.to_string()))
                .vertex_buffers(TEXT_VBL)
                .immediate_size(size_of::<TextImmediates>() as u32)
                .shadow_transparency(true)
                .build()
        );

        store_add_checked!(
            store,
            HShader::TEXT_3D_PICKER_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Text 3D Picking Shader")
                .code(ShaderCode::Full(SHADER_TEXT3D_PICKER.to_string()))
                .vertex_buffers(TEXT_VBL)
                .immediate_size(size_of::<TextImmediates>() as u32)
                .shadow_transparency(true)
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        #[cfg(debug_assertions)]
        {
            use crate::utils::sizes::{VEC3_SIZE, WGPU_VEC4_ALIGN};
            use wgpu::{VertexAttribute, VertexFormat, VertexStepMode};

            store_add_checked!(
                store,
                HShader::DEBUG_EDGES_ID,
                Shader::builder()
                    .shader_type(ShaderType::Custom)
                    .name("Mesh Debug Edges Shader")
                    .code(ShaderCode::Full(DEBUG_EDGES_SHADER.to_string()))
                    .polygon_mode(PolygonMode::Line)
                    .immediate_size(WGPU_VEC4_ALIGN as u32)
                    .build()
            );

            store_add_checked!(
                store,
                HShader::DEBUG_VERTEX_NORMALS_ID,
                Shader::builder()
                    .shader_type(ShaderType::Custom)
                    .name("Mesh Debug Vertices Shader")
                    .code(ShaderCode::Full(DEBUG_VERTEX_NORMAL_SHADER.to_string()))
                    .topology(PrimitiveTopology::LineList)
                    .polygon_mode(PolygonMode::Line)
                    .vertex_buffers(&DEFAULT_VBL_STEP_INSTANCE)
                    .build()
            );

            const DEBUG_LINE_VBL: &[VertexBufferLayout] = &[VertexBufferLayout {
                array_stride: VEC3_SIZE + VEC4_SIZE,
                step_mode: VertexStepMode::Vertex,
                attributes: &[
                    VertexAttribute {
                        format: VertexFormat::Float32x3, // position
                        offset: 0,
                        shader_location: 0,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4, // color
                        offset: VEC3_SIZE,
                        shader_location: 1,
                    },
                ],
            }];

            store_add_checked!(
                store,
                HShader::DEBUG_LINES_ID,
                Shader::builder()
                    .shader_type(ShaderType::Custom)
                    .name("Line Debug")
                    .code(ShaderCode::Full(DEBUG_LINES_SHADER.to_string()))
                    .topology(PrimitiveTopology::LineList)
                    .polygon_mode(PolygonMode::Line)
                    .vertex_buffers(DEBUG_LINE_VBL)
                    .build()
            );

            const DEBUG_TEXT: &[VertexBufferLayout] = &[VertexBufferLayout {
                array_stride: VEC2_SIZE * 2,
                step_mode: VertexStepMode::Vertex,
                attributes: &[VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                }], // dont need atlas uv
            }];

            store_add_checked!(
                store,
                HShader::DEBUG_TEXT2D_GEOMETRY_ID,
                Shader::builder()
                    .shader_type(ShaderType::Custom)
                    .name("Debug 2D Text Geometry Shader")
                    .code(ShaderCode::Full(DEBUG_TEXT2D_GEOMETRY.to_string()))
                    .polygon_mode(PolygonMode::Line)
                    .vertex_buffers(DEBUG_TEXT)
                    .immediate_size(size_of::<TextImmediates>() as u32)
                    .depth_enabled(false)
                    .build()
            );

            store_add_checked!(
                store,
                HShader::DEBUG_TEXT3D_GEOMETRY_ID,
                Shader::builder()
                    .shader_type(ShaderType::Custom)
                    .name("Debug 3D Text Geometry Shader")
                    .code(ShaderCode::Full(DEBUG_TEXT3D_GEOMETRY.to_string()))
                    .polygon_mode(PolygonMode::Line)
                    .vertex_buffers(DEBUG_TEXT)
                    .immediate_size(size_of::<TextImmediates>() as u32)
                    .build()
            );

            store_add_checked!(
                store,
                HShader::DEBUG_LIGHT_ID,
                Shader::builder()
                    .shader_type(ShaderType::Custom)
                    .name("Light Debug")
                    .code(ShaderCode::Full(DEBUG_LIGHT_SHADER.to_string()))
                    .topology(PrimitiveTopology::LineList)
                    .polygon_mode(PolygonMode::Line)
                    .vertex_buffers(&[])
                    .immediate_size(4)
                    .build()
            );
        }
    }
}

impl StoreTypeFallback for Shader {
    #[inline]
    fn fallback() -> H<Self> {
        HShader::FALLBACK
    }
}

impl StoreTypeName for Shader {
    #[inline]
    fn name(&self) -> &str {
        self.name()
    }
}

impl StoreType for Shader {
    #[inline]
    fn name() -> &'static str {
        "Shader"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        let name = match handle.id() {
            HShader::FALLBACK_ID => "Diffuse Fallback",
            HShader::DIM2_ID => "2 Dimensional Shader",
            HShader::DIM3_ID => "3 Dimensional Shader",
            HShader::TEXT_2D_ID => "2D Text Shader",
            HShader::TEXT_3D_ID => "3D Text Shader",
            HShader::POST_PROCESS_ID => "Post Process Shader",

            #[cfg(debug_assertions)]
            HShader::DEBUG_EDGES_ID => "Debug Edges Shader",
            #[cfg(debug_assertions)]
            HShader::DEBUG_VERTEX_NORMALS_ID => "Debug Vertex Normals Shader",
            #[cfg(debug_assertions)]
            HShader::DEBUG_LINES_ID => "Debug Rays Shader",
            #[cfg(debug_assertions)]
            HShader::DEBUG_TEXT2D_GEOMETRY_ID => "Debug Text 2D Geometry Shader",
            #[cfg(debug_assertions)]
            HShader::DEBUG_TEXT3D_GEOMETRY_ID => "Debug Text 3D Geometry Shader",
            #[cfg(debug_assertions)]
            HShader::DEBUG_LIGHT_ID => "Debug Lights Shader",

            _ => return HandleName::Id(handle),
        };

        HandleName::Static(name)
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::MAX_BUILTIN_ID
    }
}

impl Shader {
    pub fn load_default<S, T>(name: S, path: T) -> Result<Shader, Box<dyn Error>>
    where
        S: Into<String>,
        T: AsRef<Path>,
    {
        let content = fs::read_to_string(path)?;
        Ok(Self::new_default(name, content))
    }

    pub fn load_fragment<S, T>(name: S, path: T) -> Result<Shader, Box<dyn Error>>
    where
        S: Into<String>,
        T: AsRef<Path>,
    {
        let code = fs::read_to_string(path)?;
        Ok(Self::new_fragment(name, code))
    }

    pub fn new_post_process<S, S2>(name: S, code: S2) -> Shader
    where
        S: Into<String>,
        S2: Into<String>,
    {
        Shader {
            name: name.into(),
            code: ShaderCode::Full(code.into()),
            polygon_mode: PolygonMode::Fill,
            topology: PrimitiveTopology::TriangleList,
            vertex_buffers: &DEFAULT_VBL,
            color_target: DEFAULT_PP_COLOR_TARGETS,
            immediate_size: 0,
            shadow_transparency: false,
            depth_enabled: false,
            shader_type: ShaderType::PostProcessing,
        }
    }

    pub fn new_fragment<S, S2>(name: S, code: S2) -> Shader
    where
        S: Into<String>,
        S2: Into<String>,
    {
        Shader {
            name: name.into(),
            code: ShaderCode::Fragment(code.into()),
            polygon_mode: PolygonMode::Fill,
            topology: PrimitiveTopology::TriangleList,
            vertex_buffers: &DEFAULT_VBL,
            color_target: DEFAULT_COLOR_TARGETS,
            immediate_size: 0,
            shadow_transparency: false,
            depth_enabled: true,
            shader_type: ShaderType::Default,
        }
    }

    pub fn new_default<S, S2>(name: S, code: S2) -> Shader
    where
        S: Into<String>,
        S2: Into<String>,
    {
        Shader {
            name: name.into(),
            code: ShaderCode::Full(code.into()),
            polygon_mode: PolygonMode::Fill,
            topology: PrimitiveTopology::TriangleList,
            vertex_buffers: &DEFAULT_VBL,
            color_target: DEFAULT_COLOR_TARGETS,
            immediate_size: 0,
            shadow_transparency: false,
            depth_enabled: true,
            shader_type: ShaderType::Default,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn polygon_mode(&self) -> PolygonMode {
        self.polygon_mode
    }

    pub fn topology(&self) -> PrimitiveTopology {
        self.topology
    }

    pub fn set_code(&mut self, source: String) {
        self.code = ShaderCode::Full(source);
    }

    pub fn code(&self) -> &ShaderCode {
        &self.code
    }

    pub fn set_fragment_code(&mut self, source: String) {
        self.code = ShaderCode::Fragment(source);
    }

    pub fn stage(&self) -> ShaderType {
        self.shader_type
    }

    pub fn immediate_size(&self) -> u32 {
        self.immediate_size
    }

    pub fn is_depth_enabled(&self) -> bool {
        self.depth_enabled
    }

    pub fn color_target(&self) -> &'static [Option<ColorTargetState>] {
        self.color_target
    }

    pub fn vertex_buffers(&self) -> &'static [VertexBufferLayout<'static>] {
        self.vertex_buffers
    }

    pub fn with_depth_enabled(mut self, enabled: bool) -> Self {
        if self.stage() == ShaderType::PostProcessing {
            return self;
        }

        self.depth_enabled = enabled;
        self
    }

    pub fn is_custom(&self) -> bool {
        self.stage() == ShaderType::Custom
    }

    pub fn is_post_process(&self) -> bool {
        self.stage() == ShaderType::PostProcessing
    }

    pub fn has_shadow_transparency(&self) -> bool {
        self.shadow_transparency
    }

    pub fn gen_code(&self) -> String {
        let map = self.bind_group_map();
        ShaderGen::new(self, &map).generate()
    }

    pub fn needs_bgl(&self, bgl: HBGL) -> bool {
        if !self.is_custom() {
            if bgl == HBGL::LIGHT || bgl == HBGL::SHADOW {
                return self.is_depth_enabled();
            }

            return true;
        }

        let use_name = match bgl.id() {
            HBGL::MODEL_ID => "model",
            HBGL::MATERIAL_ID => "material",
            HBGL::LIGHT_ID => "light",
            HBGL::SHADOW_ID => "shadow",

            HBGL::RENDER_ID => return true,
            _ => return false,
        };
        let source = self.code().code();

        for line in source.lines() {
            let Some(i) = line.find("#use ") else {
                continue;
            };

            if line[i + 5..].trim() == use_name {
                return true;
            }
        }

        false
    }

    pub fn bind_group_map(&self) -> BindGroupMap {
        let mut map = BindGroupMap::new(0);
        let mut idx = 1;

        if self.is_post_process() {
            map.post_process = Some(idx);
            return map;
        }

        if self.needs_bgl(HBGL::MODEL) {
            map.model = Some(idx);
            idx += 1;
        }
        if self.needs_bgl(HBGL::MATERIAL) {
            map.material = Some(idx);
            idx += 1;
        }
        if self.needs_bgl(HBGL::LIGHT) {
            map.light = Some(idx);
            idx += 1;
        }
        if self.needs_bgl(HBGL::SHADOW) {
            map.shadow = Some(idx);
        }

        map
    }

    fn required_bgls(&self) -> Vec<HBGL> {
        let mut out = Vec::new();
        out.push(HBGL::RENDER);

        if self.is_post_process() {
            out.push(HBGL::POST_PROCESS);
            return out;
        }

        if self.needs_bgl(HBGL::MODEL) {
            out.push(HBGL::MODEL);
        }
        if self.needs_bgl(HBGL::MATERIAL) {
            out.push(HBGL::MATERIAL);
        }
        if self.needs_bgl(HBGL::LIGHT) {
            out.push(HBGL::LIGHT);
        }
        if self.needs_bgl(HBGL::SHADOW) {
            out.push(HBGL::SHADOW);
        }

        out
    }

    pub(crate) fn solid_layout(&self, device: &Device, cache: &AssetCache) -> PipelineLayout {
        let layout_name = format!("{} Pipeline Layout", self.name());
        let bgls = self.required_bgls();
        let layouts: Vec<Arc<BindGroupLayout>> = bgls
            .iter()
            .map(|handle| {
                cache
                    .bgl(*handle)
                    .expect("required bind group layout should exist")
            })
            .collect();
        let refs: Vec<&BindGroupLayout> = layouts.iter().map(|l| l.as_ref()).collect();

        self.layout_with(device, &layout_name, &refs)
    }

    pub(crate) fn shadow_layout(
        &self,
        device: &Device,
        cache: &AssetCache,
    ) -> Option<PipelineLayout> {
        if self.is_post_process() {
            return None;
        }

        let layout_name = format!("{} Shadow Pipeline Layout", self.name());
        let bgls = self.required_bgls();
        let layouts: Vec<Arc<BindGroupLayout>> = bgls
            .iter()
            .map(|handle| {
                cache
                    .bgl(*handle)
                    .expect("required bind group layout should exist")
            })
            .collect();
        let refs: Vec<&BindGroupLayout> = layouts.iter().map(|l| l.as_ref()).collect();

        Some(self.layout_with(device, &layout_name, &refs))
    }

    fn layout_with(
        &self,
        device: &Device,
        layout_name: &str,
        fixed_bgls: &[&BindGroupLayout],
    ) -> PipelineLayout {
        let desc = PipelineLayoutDescriptor {
            label: Some(layout_name),
            bind_group_layouts: fixed_bgls,
            immediate_size: self.immediate_size(),
        };
        device.create_pipeline_layout(&desc)
    }
}
