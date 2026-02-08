// this module only has tests for the built-in shaders and can be safely ignored
#[cfg(test)]
mod shaders;

pub mod checks;
pub mod defaults;
pub mod immediates;

use self::defaults::{
    DEFAULT_COLOR_TARGETS, DEFAULT_PP_COLOR_TARGETS, DEFAULT_VBL, DEFAULT_VBL_STEP_INSTANCE,
    ONLY_COLOR_TARGET,
};
use crate::HShader;
use crate::material_inputs::MaterialInputLayout;
use crate::shader::immediates::{TextImmediate, UiLineImmediate};
use crate::store::{
    H, HandleName, Store, StoreDefaults, StoreType, StoreTypeFallback, StoreTypeName,
};
use crate::store_add_checked;
use crate::{HBGL, Material};
use bon::Builder;
use std::error::Error;
use std::fs;
use std::path::Path;
use syrillian_shadergen::function::{PbrShader, PostProcessPassthroughMaterial};
use syrillian_shadergen::generator::{
    MaterialCompiler, MaterialGroupOverrides, PostProcessCompiler, ShaderKind, assemble_shader,
};
use syrillian_shadergen::generator::{MeshPass, MeshSkinning, PICKING_TEXTURE_FORMAT};
use syrillian_utils::sizes::{VEC2_SIZE, VEC3_SIZE, VEC4_SIZE, WGPU_VEC4_ALIGN};
use tracing::debug;
use wgpu::{
    ColorTargetState, ColorWrites, PolygonMode, PrimitiveTopology, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexStepMode,
};

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

#[derive(Debug, Copy, Clone)]
pub struct MaterialShaderSet {
    pub base: HShader,
    pub picking: HShader,
    pub shadow: HShader,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ShaderType {
    Default,
    Custom,
    PostProcessing,
}

#[derive(Debug, Clone)]
pub struct MaterialShaderGroups {
    pub material: String,
    pub material_textures: String,
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
    material_layout: Option<MaterialInputLayout>,
    material_groups: Option<MaterialShaderGroups>,
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
    pub const LINE_2D_ID: u32 = 10;
    pub const POST_PROCESS_SSR_ID: u32 = 11;

    pub const DEBUG_EDGES_ID: u32 = 12;
    pub const DEBUG_VERTEX_NORMALS_ID: u32 = 13;
    pub const DEBUG_LINES_ID: u32 = 14;
    pub const DEBUG_TEXT2D_GEOMETRY_ID: u32 = 15;
    pub const DEBUG_TEXT3D_GEOMETRY_ID: u32 = 16;
    pub const DEBUG_LIGHT_ID: u32 = 17;

    pub const DIM3_SKINNED_ID: u32 = 18;
    pub const DIM3_PICKER_SKINNED_ID: u32 = 19;
    pub const DIM3_SHADOW_ID: u32 = 20;
    pub const DIM3_SHADOW_SKINNED_ID: u32 = 21;
    pub const MAX_BUILTIN_ID: u32 = 21;

    // The fallback shader if a pipeline fails
    pub const FALLBACK: H<Shader> = H::new(Self::FALLBACK_ID);

    // The default 2D shader.
    pub const DIM2: H<Shader> = H::new(Self::DIM2_ID);

    // The default 2D picking shader.
    pub const DIM2_PICKING: H<Shader> = H::new(Self::DIM2_PICKER_ID);

    // The default 3D shader.
    pub const DIM3: H<Shader> = H::new(Self::DIM3_ID);

    // The default 3D picking shader.
    pub const DIM3_PICKING: H<Shader> = H::new(Self::DIM3_PICKER_ID);

    // Default 3D skinned shader.
    pub const DIM3_SKINNED: H<Shader> = H::new(Self::DIM3_SKINNED_ID);

    // Default 3D skinned picking shader.
    pub const DIM3_PICKING_SKINNED: H<Shader> = H::new(Self::DIM3_PICKER_SKINNED_ID);

    // Default 3D shadow shader.
    pub const DIM3_SHADOW: H<Shader> = H::new(Self::DIM3_SHADOW_ID);

    // Default 3D skinned shadow shader.
    pub const DIM3_SHADOW_SKINNED: H<Shader> = H::new(Self::DIM3_SHADOW_SKINNED_ID);

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

    // Shader for drawing single 2D lines.
    pub const LINE_2D: H<Shader> = H::new(Self::LINE_2D_ID);

    // Post processing shader for screen space reflection
    pub const POST_PROCESS_SSR: H<Shader> = H::new(Self::POST_PROCESS_SSR_ID);

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
const SHADER_DIM2_PICKER: &str = include_str!("shaders/picking_ui.wgsl");
const SHADER_TEXT2D: &str = include_str!("shaders/text2d.wgsl");
const SHADER_TEXT2D_PICKER: &str = include_str!("shaders/picking_text2d.wgsl");
const SHADER_TEXT3D: &str = include_str!("shaders/text3d.wgsl");
const SHADER_TEXT3D_PICKER: &str = include_str!("shaders/picking_text3d.wgsl");
const SHADER_LINE2D: &str = include_str!("shaders/line.wgsl");
const SHADER_POST_PROCESS_SSR: &str = include_str!("shaders/ssr_post_process.wgsl");

const DEBUG_EDGES_SHADER: &str = include_str!("shaders/debug/edges.wgsl");
const DEBUG_VERTEX_NORMAL_SHADER: &str = include_str!("shaders/debug/vertex_normals.wgsl");
const DEBUG_LINES_SHADER: &str = include_str!("shaders/debug/lines.wgsl");
const DEBUG_TEXT2D_GEOMETRY: &str = include_str!("shaders/debug/text2d_geometry.wgsl");
const DEBUG_TEXT3D_GEOMETRY: &str = include_str!("shaders/debug/text3d_geometry.wgsl");
const DEBUG_LIGHT_SHADER: &str = include_str!("shaders/debug/light.wgsl");

impl StoreDefaults for Shader {
    fn populate(store: &mut Store<Self>) {
        let post_process_fs =
            PostProcessCompiler::compile_post_process_fragment(&PostProcessPassthroughMaterial, 0);
        let pbr = PbrShader;
        let mesh3d =
            MaterialCompiler::compile_mesh(&pbr, 0, MeshSkinning::Unskinned, MeshPass::Base);

        debug!("{mesh3d}");
        let mesh3d_skinned =
            MaterialCompiler::compile_mesh(&pbr, 0, MeshSkinning::Skinned, MeshPass::Base);
        let mesh3d_picking = MaterialCompiler::compile_mesh_picking(MeshSkinning::Unskinned);
        let mesh3d_picking_skinned = MaterialCompiler::compile_mesh_picking(MeshSkinning::Skinned);
        let mesh3d_shadow =
            MaterialCompiler::compile_mesh(&pbr, 0, MeshSkinning::Unskinned, MeshPass::Shadow);
        let mesh3d_shadow_skinned =
            MaterialCompiler::compile_mesh(&pbr, 0, MeshSkinning::Skinned, MeshPass::Shadow);

        let default_layout = Material::default_layout();
        let material_groups = MaterialShaderGroups {
            material: default_layout.wgsl_material_group(),
            material_textures: default_layout.wgsl_material_textures_group(),
        };
        let material_immediates = default_layout.immediate_size();

        store_add_checked!(
            store,
            HShader::FALLBACK_ID,
            Shader::new_default("Fallback", SHADER_FALLBACK3D)
        );

        store_add_checked!(
            store,
            HShader::DIM2_ID,
            Shader::builder()
                .shader_type(ShaderType::Default)
                .name("2D Default")
                .code(ShaderCode::Full(SHADER_DIM2.to_string()))
                .color_target(ONLY_COLOR_TARGET)
                .immediate_size(material_immediates)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
                .depth_enabled(false)
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM3_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("3D Default")
                .code(ShaderCode::Full(mesh3d))
                .immediate_size(material_immediates)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
                .build()
        );

        store_add_checked!(
            store,
            HShader::POST_PROCESS_ID,
            Shader::new_post_process_fragment("Post Process", post_process_fs)
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
                .code(ShaderCode::Full(mesh3d_picking))
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
                .color_target(ONLY_COLOR_TARGET)
                .vertex_buffers(TEXT_VBL)
                .immediate_size(size_of::<TextImmediate>() as u32)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
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
                .immediate_size(size_of::<TextImmediate>() as u32)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
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
                .immediate_size(size_of::<TextImmediate>() as u32)
                .shadow_transparency(true)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
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
                .immediate_size(size_of::<TextImmediate>() as u32)
                .shadow_transparency(true)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        store_add_checked!(
            store,
            HShader::LINE_2D_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Line 2D Shader")
                .code(ShaderCode::Full(SHADER_LINE2D.to_string()))
                .color_target(ONLY_COLOR_TARGET)
                .topology(PrimitiveTopology::TriangleList)
                .vertex_buffers(&[])
                .immediate_size(size_of::<UiLineImmediate>() as u32)
                .depth_enabled(false)
                .build()
        );

        store_add_checked!(
            store,
            HShader::POST_PROCESS_SSR_ID,
            Shader::new_post_process("SSR Post Process", SHADER_POST_PROCESS_SSR)
        );

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
                .color_target(ONLY_COLOR_TARGET)
                .immediate_size(size_of::<TextImmediate>() as u32)
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
                .immediate_size(size_of::<TextImmediate>() as u32)
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

        store_add_checked!(
            store,
            HShader::DIM3_SKINNED_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("3D Skinned")
                .code(ShaderCode::Full(mesh3d_skinned))
                .immediate_size(material_immediates)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM3_PICKER_SKINNED_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Default 3D Skinned Picking Shader")
                .code(ShaderCode::Full(mesh3d_picking_skinned))
                .immediate_size(VEC4_SIZE as u32)
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM3_SHADOW_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Default 3D Shadow Shader")
                .code(ShaderCode::Full(mesh3d_shadow))
                .immediate_size(material_immediates)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM3_SHADOW_SKINNED_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Default 3D Skinned Shadow Shader")
                .code(ShaderCode::Full(mesh3d_shadow_skinned))
                .immediate_size(material_immediates)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
                .build()
        );
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
            HShader::DIM3_SKINNED_ID => "3 Dimensional Skinned Shader",
            HShader::DIM3_PICKER_ID => "3 Dimensional Picking Shader",
            HShader::DIM3_PICKER_SKINNED_ID => "3 Dimensional Skinned Picking Shader",
            HShader::DIM3_SHADOW_ID => "3 Dimensional Shadow Shader",
            HShader::DIM3_SHADOW_SKINNED_ID => "3 Dimensional Skinned Shadow Shader",
            HShader::TEXT_2D_ID => "2D Text Shader",
            HShader::TEXT_3D_ID => "3D Text Shader",
            HShader::POST_PROCESS_ID => "Post Process Shader",
            HShader::POST_PROCESS_SSR_ID => "SSR Post Process Shader",

            HShader::DEBUG_EDGES_ID => "Debug Edges Shader",
            HShader::DEBUG_VERTEX_NORMALS_ID => "Debug Vertex Normals Shader",
            HShader::DEBUG_LINES_ID => "Debug Rays Shader",
            HShader::DEBUG_TEXT2D_GEOMETRY_ID => "Debug Text 2D Geometry Shader",
            HShader::DEBUG_TEXT3D_GEOMETRY_ID => "Debug Text 3D Geometry Shader",
            HShader::DEBUG_LIGHT_ID => "Debug Lights Shader",

            _ => return HandleName::Id(handle),
        };

        HandleName::Static(name)
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= HShader::MAX_BUILTIN_ID
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
            material_layout: None,
            material_groups: None,
        }
    }

    pub fn new_post_process_fragment<S, S2>(name: S, code: S2) -> Shader
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
            color_target: DEFAULT_PP_COLOR_TARGETS,
            immediate_size: 0,
            shadow_transparency: false,
            depth_enabled: false,
            shader_type: ShaderType::PostProcessing,
            material_layout: None,
            material_groups: None,
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
            material_layout: None,
            material_groups: None,
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
            material_layout: None,
            material_groups: None,
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

    pub fn color_only(mut self) -> Self {
        self.color_target = ONLY_COLOR_TARGET;
        self
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
        self.gen_code_with_map(&map)
    }

    pub fn gen_code_with_map(&self, map: &BindGroupMap) -> String {
        let fragment_only = self.code().is_only_fragment_shader();
        let kind = match self.stage() {
            ShaderType::PostProcessing => ShaderKind::PostProcess,
            ShaderType::Custom => ShaderKind::Custom,
            ShaderType::Default => ShaderKind::Default,
        };
        let material_groups = self
            .material_groups
            .as_ref()
            .map(|groups| MaterialGroupOverrides {
                material: groups.material.as_str(),
                material_textures: groups.material_textures.as_str(),
            });

        let generated = assemble_shader(
            self.code().code(),
            fragment_only,
            kind,
            self.depth_enabled,
            material_groups,
        );

        debug!("Generated shader {:?}: {generated}", self.name());

        rewrite_bind_groups(generated, map)
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

            let target = line[i + 5..].trim();

            if bgl == HBGL::MATERIAL && (target == "material" || target == "material_textures") {
                return true;
            }

            if bgl == HBGL::SHADOW && (target == "shadow" || target == "light") {
                return true;
            }

            if target == use_name {
                return true;
            }
        }

        fn has_group(source: &str, group: u32) -> bool {
            let needle = format!("@group({group})");
            source.contains(&needle)
        }

        match bgl.id() {
            HBGL::MODEL_ID => has_group(source, 1),
            HBGL::MATERIAL_ID => has_group(source, 2),
            HBGL::LIGHT_ID => has_group(source, 3),
            HBGL::SHADOW_ID => has_group(source, 4),
            _ => false,
        }
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

    pub fn material_layout(&self) -> Option<&MaterialInputLayout> {
        self.material_layout.as_ref()
    }

    pub fn material_groups(&self) -> Option<&MaterialShaderGroups> {
        self.material_groups.as_ref()
    }
}

fn rewrite_bind_groups(source: String, map: &BindGroupMap) -> String {
    let mut out = source;

    let mut replace = |orig: u32, new_idx: u32| {
        let needle = format!("@group({orig})");
        let repl = format!("@group({new_idx})");
        out = out.replace(&needle, &repl);
    };

    replace(0, map.render);
    if let Some(idx) = map.model {
        replace(1, idx);
    }
    if let Some(idx) = map.material {
        replace(2, idx);
    }
    if let Some(idx) = map.light {
        replace(3, idx);
    }
    if let Some(idx) = map.shadow {
        replace(4, idx);
    }
    if let Some(idx) = map.post_process {
        replace(1, idx);
    }

    out
}
