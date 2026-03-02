// this module only has tests for the built-in shaders and can be safely ignored
#[cfg(test)]
mod shaders;

pub mod checks;
pub mod defaults;
pub mod immediates;

use self::defaults::{
    DEFAULT_COLOR_TARGETS, DEFAULT_PP_COLOR_TARGETS, DEFAULT_VBL, DEFAULT_VBL_STEP_INSTANCE,
    ONLY_COLOR_TARGET, ONLY_COLOR_TARGET_SRGB, SURFACE_PP_COLOR_TARGETS,
};
use crate::HShader;
use crate::defaults::{PARTICLE_VERTEX_LAYOUT, PICKING_COLOR_TARGET};
use crate::material_inputs::{MaterialInputLayout, MaterialTextureDef};
use crate::shader::immediates::{TextImmediate, UiLineImmediate};
use crate::store::streaming::asset_store::{AssetType, StreamingAssetFile, StreamingAssetPayload};
use crate::store::streaming::decode_helper::{DecodeHelper, MapDecodeHelper, ParseDecode};
use crate::store::streaming::packaged_scene::BuiltPayload;
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{
    AssetKey, AssetRefreshMessage, H, HandleName, Store, StoreDefaults, StoreType,
    StoreTypeFallback, UpdateAssetMessage,
};
use crate::store_add_checked;
use crate::{HBGL, Material};
use bon::Builder;
use crossbeam_channel::Sender;
use snafu::whatever;
use std::error::Error;
use std::fs;
use std::path::Path;
use syrillian_shadergen::function::{PbrShader, PostProcessPassthroughMaterial};
use syrillian_shadergen::generator::MeshPass;
use syrillian_shadergen::generator::{MaterialGroupOverrides, ShaderKind};
use syrillian_shadergen::{MaterialCompiler, PostProcessCompiler, ShaderGenerator};
use syrillian_utils::sizes::{VEC2_SIZE, VEC3_SIZE, VEC4_SIZE, WGPU_VEC4_ALIGN};
use wgpu::{
    ColorTargetState, PolygonMode, PrimitiveTopology, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexStepMode,
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
    Picking,
    Shadow,
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
    #[builder(default = true)]
    opaque: bool,
    #[builder(default = true)]
    depth_enabled: bool,
    shader_type: ShaderType,
    material_layout: Option<MaterialInputLayout>,
    material_groups: Option<MaterialShaderGroups>,
}

impl<S: shader_builder::State> ShaderBuilder<S>
where
    S::Opaque: shader_builder::IsUnset,
{
    pub fn transparent(self) -> ShaderBuilder<shader_builder::SetOpaque<S>> {
        self.opaque(false)
    }
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
    pub const DIM2_PICKER_ID: u32 = 2;
    pub const DIM3_ID: u32 = 3;
    pub const DIM3_PICKER_ID: u32 = 4;
    pub const DIM3_SHADOW_ID: u32 = 5;
    pub const POST_PROCESS_FXAA_ID: u32 = 6;
    pub const POST_PROCESS_ID: u32 = 7;
    pub const SKYBOX_ID: u32 = 8;
    pub const SKYBOX_PROCEDURAL_ID: u32 = 9;
    pub const PARTICLE_SYSTEM_ID: u32 = 10;
    pub const TEXT_2D_ID: u32 = 11;
    pub const TEXT_2D_PICKER_ID: u32 = 12;
    pub const TEXT_3D_ID: u32 = 13;
    pub const TEXT_3D_PICKER_ID: u32 = 14;
    pub const TEXT_3D_SHADOW_ID: u32 = 15;
    pub const LINE_2D_ID: u32 = 16;
    pub const DEBUG_EDGES_ID: u32 = 17;
    pub const DEBUG_MESH_BOUNDS_ID: u32 = 18;
    pub const DEBUG_VERTEX_NORMALS_ID: u32 = 19;
    pub const DEBUG_LINES_ID: u32 = 20;
    pub const DEBUG_TEXT2D_GEOMETRY_ID: u32 = 21;
    pub const DEBUG_TEXT3D_GEOMETRY_ID: u32 = 22;
    pub const DEBUG_LIGHT_ID: u32 = 23;
    pub const MAX_BUILTIN_ID: u32 = 23;

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

    // Default 3D shadow shader.
    pub const DIM3_SHADOW: H<Shader> = H::new(Self::DIM3_SHADOW_ID);

    // Default post-processing shader
    pub const POST_PROCESS: H<Shader> = H::new(Self::POST_PROCESS_ID);
    pub const POST_PROCESS_FXAA: H<Shader> = H::new(Self::POST_PROCESS_FXAA_ID);

    // Default 2D Text shader.
    pub const TEXT_2D: H<Shader> = H::new(Self::TEXT_2D_ID);

    // Default 2D Text picking shader.
    pub const TEXT_2D_PICKING: H<Shader> = H::new(Self::TEXT_2D_PICKER_ID);

    // Default 3D Text shader.
    pub const TEXT_3D: H<Shader> = H::new(Self::TEXT_3D_ID);

    // Default 3D Text picking shader.
    pub const TEXT_3D_PICKING: H<Shader> = H::new(Self::TEXT_3D_PICKER_ID);

    // Default 3D Text shadow shader.
    pub const TEXT_3D_SHADOW: H<Shader> = H::new(Self::TEXT_3D_SHADOW_ID);

    // Shader for drawing single 2D lines.
    pub const LINE_2D: H<Shader> = H::new(Self::LINE_2D_ID);

    // An addon shader ID that is used for drawing debug edges on meshes
    pub const DEBUG_EDGES: H<Shader> = H::new(Self::DEBUG_EDGES_ID);

    // An addon shader ID that is used for drawing debug vertex normals on meshes
    pub const DEBUG_VERTEX_NORMALS: H<Shader> = H::new(Self::DEBUG_VERTEX_NORMALS_ID);

    // An addon shader ID that is used for drawing debug lines
    pub const DEBUG_LINES: H<Shader> = H::new(Self::DEBUG_LINES_ID);
    pub const DEBUG_TEXT2D_GEOMETRY: H<Shader> = H::new(Self::DEBUG_TEXT2D_GEOMETRY_ID);
    pub const DEBUG_TEXT3D_GEOMETRY: H<Shader> = H::new(Self::DEBUG_TEXT3D_GEOMETRY_ID);
    pub const DEBUG_LIGHT: H<Shader> = H::new(Self::DEBUG_LIGHT_ID);
    pub const DEBUG_MESH_BOUNDS: H<Shader> = H::new(Self::DEBUG_MESH_BOUNDS_ID);
    pub const SKYBOX: H<Shader> = H::new(Self::SKYBOX_ID);
    pub const SKYBOX_PROCEDURAL: H<Shader> = H::new(Self::SKYBOX_PROCEDURAL_ID);
    pub const PARTICLE_SYSTEM: H<Shader> = H::new(Self::PARTICLE_SYSTEM_ID);
}

const SHADER_FALLBACK3D: &str = include_str!("shaders/fallback_shader3d.wgsl");
const SHADER_DIM2: &str = include_str!("shaders/shader2d.wgsl");
const SHADER_DIM2_PICKER: &str = include_str!("shaders/picking_ui.wgsl");
const SHADER_TEXT2D: &str = include_str!("shaders/text2d.wgsl");
const SHADER_TEXT2D_PICKER: &str = include_str!("shaders/picking_text2d.wgsl");
const SHADER_TEXT3D: &str = include_str!("shaders/text3d.wgsl");
const SHADER_TEXT3D_PICKER: &str = include_str!("shaders/picking_text3d.wgsl");
const SHADER_TEXT3D_SHADOW: &str = include_str!("shaders/text3d_shadow.wgsl");
const SHADER_LINE2D: &str = include_str!("shaders/line.wgsl");
const SHADER_POST_PROCESS_FXAA: &str = include_str!("shaders/post_process_fxaa.wgsl");
const SHADER_SKYBOX: &str = include_str!("shaders/skybox.wgsl");
const SHADER_SKYBOX_PROCEDURAL: &str = include_str!("shaders/skybox_procedural.wgsl");
const SHADER_PARTICLE_SYSTEM: &str = include_str!("shaders/particle_system_render.wgsl");

const DEBUG_EDGES_SHADER: &str = include_str!("shaders/debug/edges.wgsl");
const DEBUG_MESH_BOUNDS_SHADER: &str = include_str!("shaders/debug/edges.wgsl");
const DEBUG_VERTEX_NORMAL_SHADER: &str = include_str!("shaders/debug/vertex_normals.wgsl");
const DEBUG_LINES_SHADER: &str = include_str!("shaders/debug/lines.wgsl");
const DEBUG_TEXT2D_GEOMETRY: &str = include_str!("shaders/debug/text2d_geometry.wgsl");
const DEBUG_TEXT3D_GEOMETRY: &str = include_str!("shaders/debug/text3d_geometry.wgsl");
const DEBUG_LIGHT_SHADER: &str = include_str!("shaders/debug/light.wgsl");

impl StoreDefaults for Shader {
    fn populate(store: &mut Store<Self>) {
        let post_process_fs =
            PostProcessCompiler::compile_post_process_fragment(&PostProcessPassthroughMaterial, 0);
        let mut pbr = PbrShader::default();
        let mesh3d = MaterialCompiler::compile_mesh(&mut pbr, 0, MeshPass::Base);
        let mesh3d_picking = MaterialCompiler::compile_mesh_picking_with_material(&mut pbr, 0);
        let mesh3d_shadow = MaterialCompiler::compile_mesh(&mut pbr, 0, MeshPass::Shadow);

        let default_layout = Material::default_layout();
        let material_groups = MaterialShaderGroups {
            material: default_layout.wgsl_material_group(),
            material_textures: default_layout.wgsl_material_textures_group(),
        };
        let material_immediates = default_layout.immediate_size();

        let text_atlas_layout = MaterialInputLayout {
            immediates: vec![],
            textures: vec![MaterialTextureDef {
                name: "atlas".to_string(),
                default: H::FALLBACK_DIFFUSE,
            }],
        };

        store_add_checked!(
            store,
            HShader::FALLBACK_ID,
            Shader::builder()
                .shader_type(ShaderType::Default)
                .name("Fallback")
                .code(ShaderCode::Fragment(SHADER_FALLBACK3D.to_string()))
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM2_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("2D Default")
                .code(ShaderCode::Full(SHADER_DIM2.to_string()))
                .vertex_buffers(&DEFAULT_VBL[0..2])
                .color_target(ONLY_COLOR_TARGET_SRGB)
                .immediate_size(material_immediates)
                .material_layout(default_layout.clone())
                .material_groups(material_groups.clone())
                .depth_enabled(false)
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM2_PICKER_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Default 2D Picking Shader")
                .code(ShaderCode::Full(SHADER_DIM2_PICKER.to_string()))
                .immediate_size(VEC4_SIZE as u32)
                .depth_enabled(false)
                .vertex_buffers(&DEFAULT_VBL[0..2])
                .color_target(PICKING_COLOR_TARGET)
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
            HShader::DIM3_PICKER_ID,
            Shader::builder()
                .shader_type(ShaderType::Picking)
                .name("Default 3D Picking Shader")
                .code(ShaderCode::Full(mesh3d_picking))
                .immediate_size(VEC4_SIZE as u32)
                .transparent()
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        store_add_checked!(
            store,
            HShader::DIM3_SHADOW_ID,
            Shader::builder()
                .shader_type(ShaderType::Shadow)
                .name("Default 3D Shadow Shader")
                .code(ShaderCode::Full(mesh3d_shadow))
                .build()
        );

        store_add_checked!(
            store,
            HShader::POST_PROCESS_FXAA_ID,
            Shader::new_post_process_fragment_linear("Post Process FXAA", SHADER_POST_PROCESS_FXAA)
        );

        store_add_checked!(
            store,
            HShader::POST_PROCESS_ID,
            Shader::new_post_process_fragment("Passthrough", post_process_fs)
        );

        store_add_checked!(
            store,
            HShader::SKYBOX_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Skybox Background Shader")
                .code(ShaderCode::Full(SHADER_SKYBOX.to_string()))
                .vertex_buffers(&[])
                .color_target(ONLY_COLOR_TARGET)
                .depth_enabled(false)
                .build()
        );

        store_add_checked!(
            store,
            HShader::SKYBOX_PROCEDURAL_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Skybox Procedural Shader")
                .code(ShaderCode::Full(SHADER_SKYBOX_PROCEDURAL.to_string()))
                .vertex_buffers(&[])
                .color_target(ONLY_COLOR_TARGET)
                .depth_enabled(false)
                .build()
        );

        store_add_checked!(
            store,
            HShader::PARTICLE_SYSTEM_ID,
            Shader::builder()
                .name("Particle System")
                .color_target(DEFAULT_COLOR_TARGETS)
                .shader_type(ShaderType::Custom)
                .vertex_buffers(PARTICLE_VERTEX_LAYOUT)
                .topology(PrimitiveTopology::PointList)
                .code(ShaderCode::Full(SHADER_PARTICLE_SYSTEM.to_string(),))
                .build()
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

        store_add_checked!(
            store,
            HShader::TEXT_2D_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Text 2D Shader")
                .code(ShaderCode::Full(SHADER_TEXT2D.to_string()))
                .color_target(ONLY_COLOR_TARGET_SRGB)
                .vertex_buffers(TEXT_VBL)
                .immediate_size(size_of::<TextImmediate>() as u32)
                .material_layout(text_atlas_layout.clone())
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
                .material_layout(text_atlas_layout.clone())
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
                .transparent()
                .material_layout(text_atlas_layout.clone())
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
                .transparent()
                .material_layout(text_atlas_layout.clone())
                .color_target(PICKING_COLOR_TARGET)
                .build()
        );

        store_add_checked!(
            store,
            HShader::TEXT_3D_SHADOW_ID,
            Shader::builder()
                .shader_type(ShaderType::Shadow)
                .name("Text 3D Shadow Shader")
                .code(ShaderCode::Full(SHADER_TEXT3D_SHADOW.to_string()))
                .vertex_buffers(TEXT_VBL)
                .immediate_size(size_of::<TextImmediate>() as u32)
                .transparent()
                .material_layout(text_atlas_layout.clone())
                .build()
        );

        store_add_checked!(
            store,
            HShader::LINE_2D_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Line 2D Shader")
                .code(ShaderCode::Full(SHADER_LINE2D.to_string()))
                .color_target(ONLY_COLOR_TARGET_SRGB)
                .topology(PrimitiveTopology::TriangleList)
                .vertex_buffers(&[])
                .immediate_size(size_of::<UiLineImmediate>() as u32)
                .depth_enabled(false)
                .build()
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
                .vertex_buffers(&DEFAULT_VBL[0..1])
                .build()
        );

        store_add_checked!(
            store,
            HShader::DEBUG_MESH_BOUNDS_ID,
            Shader::builder()
                .shader_type(ShaderType::Custom)
                .name("Mesh Debug Bounds Shader")
                .code(ShaderCode::Full(DEBUG_MESH_BOUNDS_SHADER.to_string()))
                .topology(PrimitiveTopology::LineList)
                .immediate_size(WGPU_VEC4_ALIGN as u32)
                .vertex_buffers(&DEFAULT_VBL[0..1])
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
    }
}

impl StoreTypeFallback for Shader {
    #[inline]
    fn fallback() -> H<Self> {
        HShader::FALLBACK
    }
}

impl StoreType for Shader {
    const NAME: &str = "Shader";
    const TYPE: AssetType = AssetType::Shader;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        let name = match handle.id() {
            HShader::FALLBACK_ID => "Diffuse Fallback",
            HShader::DIM2_ID => "2 Dimensional Shader",
            HShader::DIM3_ID => "3 Dimensional Shader",
            HShader::DIM3_PICKER_ID => "3 Dimensional Picking Shader",
            HShader::DIM3_SHADOW_ID => "3 Dimensional Shadow Shader",
            HShader::TEXT_2D_ID => "2D Text Shader",
            HShader::TEXT_3D_ID => "3D Text Shader",
            HShader::TEXT_3D_SHADOW_ID => "3D Text Shadow Shader",
            HShader::POST_PROCESS_ID => "Post Process Shader",
            HShader::POST_PROCESS_FXAA_ID => "Post Process FXAA Shader",
            HShader::SKYBOX_ID => "Skybox Background Shader",
            HShader::SKYBOX_PROCEDURAL_ID => "Skybox Procedural Shader",

            HShader::DEBUG_EDGES_ID => "Debug Edges Shader",
            HShader::DEBUG_VERTEX_NORMALS_ID => "Debug Vertex Normals Shader",
            HShader::DEBUG_LINES_ID => "Debug Rays Shader",
            HShader::DEBUG_TEXT2D_GEOMETRY_ID => "Debug Text 2D Geometry Shader",
            HShader::DEBUG_TEXT3D_GEOMETRY_ID => "Debug Text 3D Geometry Shader",
            HShader::DEBUG_LIGHT_ID => "Debug Lights Shader",
            HShader::DEBUG_MESH_BOUNDS_ID => "Debug Mesh Bounds Shader",

            _ => return HandleName::Id(handle),
        };

        HandleName::Static(name)
    }

    fn refresh_dirty(&self, key: AssetKey, assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        assets_tx
            .send(AssetRefreshMessage::updated(
                key,
                UpdateAssetMessage::UpdateShader(self.clone()),
            ))
            .is_ok()
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
            opaque: true,
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
            color_target: SURFACE_PP_COLOR_TARGETS,
            immediate_size: 0,
            opaque: true,
            depth_enabled: false,
            shader_type: ShaderType::PostProcessing,
            material_layout: None,
            material_groups: None,
        }
    }

    pub fn new_post_process_fragment_linear<S, S2>(name: S, code: S2) -> Shader
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
            opaque: true,
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
            opaque: true,
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
        let code = code.into();
        let has_vertex = code.contains("@vertex");
        let has_fragment = code.contains("@fragment");
        let code = match (has_vertex, has_fragment) {
            (true, true) => ShaderCode::Full(code),
            (false, true) => ShaderCode::Fragment(code),
            _ => ShaderCode::Full(code), // TODO: Error, maybe?
        };

        Shader {
            name: name.into(),
            code,
            polygon_mode: PolygonMode::Fill,
            topology: PrimitiveTopology::TriangleList,
            vertex_buffers: &DEFAULT_VBL,
            color_target: DEFAULT_COLOR_TARGETS,
            immediate_size: 0,
            opaque: true,
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
        matches!(
            self.stage(),
            ShaderType::Custom | ShaderType::Picking | ShaderType::Shadow
        )
    }

    pub fn is_post_process(&self) -> bool {
        self.stage() == ShaderType::PostProcessing
    }

    pub fn is_opaque(&self) -> bool {
        self.opaque
    }

    pub fn gen_code(&self) -> String {
        let map = self.bind_group_map();
        self.gen_code_with_map(&map)
    }

    pub fn gen_code_with_map(&self, map: &BindGroupMap) -> String {
        let fragment_only = self.code().is_only_fragment_shader();
        let kind = match self.stage() {
            ShaderType::PostProcessing => ShaderKind::PostProcess,
            ShaderType::Custom | ShaderType::Picking | ShaderType::Shadow => ShaderKind::Custom,
            ShaderType::Default => ShaderKind::Default,
        };
        let material_groups = self
            .material_groups
            .as_ref()
            .map(|groups| MaterialGroupOverrides {
                material: groups.material.as_str(),
                material_textures: groups.material_textures.as_str(),
            });

        let generated = ShaderGenerator::assemble_shader(
            self.code().code(),
            fragment_only,
            kind,
            self.depth_enabled,
            material_groups,
        );

        rewrite_bind_groups(generated, map)
    }

    pub fn needs_bgl(&self, bgl: HBGL) -> bool {
        if self.stage() == ShaderType::Default {
            return match bgl.id() {
                HBGL::RENDER_ID | HBGL::MODEL_ID | HBGL::MATERIAL_ID => true,
                HBGL::LIGHT_ID | HBGL::SHADOW_ID => self.is_depth_enabled(),
                _ => false,
            };
        }

        if self.stage() == ShaderType::PostProcessing {
            return matches!(bgl.id(), HBGL::RENDER_ID | HBGL::POST_PROCESS_ID);
        }

        let use_name = match bgl.id() {
            HBGL::MODEL_ID => "model",
            HBGL::PARTICLE_RENDER_ID => "particle_render",
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

        fn has_group_binding(source: &str, group: u32, binding: u32) -> bool {
            let group_needle = format!("@group({group})");
            let binding_needle = format!("@binding({binding})");
            source.lines().any(|line| {
                let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
                compact.contains(&group_needle) && compact.contains(&binding_needle)
            })
        }

        match bgl.id() {
            HBGL::MODEL_ID => has_group(source, 1),
            HBGL::PARTICLE_RENDER_ID => has_group_binding(source, 1, 1),
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

        if self.needs_bgl(HBGL::MODEL) || self.needs_bgl(HBGL::PARTICLE_RENDER) {
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

impl StreamableAsset for Shader {
    fn encode(&self) -> BuiltPayload {
        todo!()
    }

    fn decode(
        payload: &StreamingAssetPayload,
        _package: &mut StreamingAssetFile,
    ) -> crate::store::streaming::error::Result<Self> {
        let root = payload.data.expect_object("shader metadata")?;

        let name: String = root.required_field("name")?.expect_parse("shader name")?;
        let shader_type = root
            .required_field("shader_type")?
            .expect_parse("shader_type")?;
        let code_obj = root
            .required_field("code")?
            .expect_object("shader code block")?;
        let code_kind = code_obj
            .required_field("kind")?
            .expect_str("shader code kind")?;
        let source = code_obj
            .required_field("source")?
            .expect_parse("shader source")?;
        let code = match code_kind {
            "Full" => ShaderCode::Full(source),
            "Fragment" => ShaderCode::Fragment(source),
            other => whatever!("unsupported shader code kind '{other}'"),
        };

        let polygon_mode = root
            .optional_field("polygon_mode")
            .expect_parse("shader polygon mode")?;
        let topology = root
            .optional_field("topology")
            .expect_parse("shader topology")?;
        let immediate_size = root
            .optional_field("immediate_size")
            .expect_parse("shader immediate_size")?;
        let depth_enabled = root
            .optional_field("depth_enabled")
            .expect_parse("shader depth_enabled")?;
        let opaque = root
            .optional_field("opaque")
            .expect_parse("shader opaque")?;

        Ok(Shader::builder()
            .name(name)
            .code(code)
            .shader_type(shader_type)
            .maybe_polygon_mode(polygon_mode)
            .maybe_topology(topology)
            .maybe_immediate_size(immediate_size)
            .maybe_depth_enabled(depth_enabled)
            .maybe_opaque(opaque)
            .build())
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
