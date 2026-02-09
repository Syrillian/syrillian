use crate::chunks::{ExpressionInput as ChunkInput, NodeId};
use crate::generator::{MaterialCompiler, PostProcessCompiler};
use crate::value::MaterialValueType;
use glamx::{Vec2, Vec3, Vec4};
use std::marker::PhantomData;
use syrillian_utils::debug_panic;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialExpressionValue {
    pub name: &'static str,
    pub value_type: MaterialValueType,
}

pub trait MaterialPinType {
    const VALUE_TYPE: MaterialValueType;
}

impl MaterialPinType for f32 {
    const VALUE_TYPE: MaterialValueType = MaterialValueType::F32;
}

impl MaterialPinType for u32 {
    const VALUE_TYPE: MaterialValueType = MaterialValueType::U32;
}

impl MaterialPinType for bool {
    const VALUE_TYPE: MaterialValueType = MaterialValueType::Bool;
}

impl MaterialPinType for Vec2 {
    const VALUE_TYPE: MaterialValueType = MaterialValueType::Vec2;
}

impl MaterialPinType for Vec3 {
    const VALUE_TYPE: MaterialValueType = MaterialValueType::Vec3;
}

impl MaterialPinType for Vec4 {
    const VALUE_TYPE: MaterialValueType = MaterialValueType::Vec4;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpressionInputSource {
    Unbound,
    Parameter(&'static str),
    Node {
        parameter: Option<&'static str>,
        node: NodeId,
        output_index: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpressionInput<T: MaterialPinType> {
    source: ExpressionInputSource,
    marker: PhantomData<T>,
}

impl<T: MaterialPinType> ExpressionInput<T> {
    pub const fn unbound() -> Self {
        Self {
            source: ExpressionInputSource::Unbound,
            marker: PhantomData,
        }
    }

    pub const fn material(name: &'static str) -> Self {
        Self {
            source: ExpressionInputSource::Parameter(name),
            marker: PhantomData,
        }
    }

    pub const fn bound(node: NodeId, output_index: u32) -> Self {
        Self {
            source: ExpressionInputSource::Node {
                parameter: None,
                node,
                output_index,
            },
            marker: PhantomData,
        }
    }

    pub fn bind(&mut self, compiler: &mut MaterialCompiler) {
        compiler.bind_expression_input(self);
    }

    pub(crate) fn material_name(&self) -> Option<&'static str> {
        match self.source {
            ExpressionInputSource::Parameter(name) => Some(name),
            ExpressionInputSource::Node { parameter, .. } => parameter,
            ExpressionInputSource::Unbound => None,
        }
    }

    pub(crate) fn is_unbound(&self) -> bool {
        matches!(self.source, ExpressionInputSource::Unbound)
    }

    pub(crate) fn set_bound(&mut self, node: NodeId, output_index: u32) {
        let parameter = match self.source {
            ExpressionInputSource::Parameter(name) => Some(name),
            ExpressionInputSource::Node { parameter, .. } => parameter,
            ExpressionInputSource::Unbound => None,
        };
        self.source = ExpressionInputSource::Node {
            parameter,
            node,
            output_index,
        };
    }

    pub fn node(&self) -> NodeId {
        match self.source {
            ExpressionInputSource::Node { node, .. } => node,
            ExpressionInputSource::Unbound | ExpressionInputSource::Parameter(_) => {
                debug_panic!("expression input is unbound");
                0
            }
        }
    }

    pub const fn output_index(&self) -> u32 {
        match self.source {
            ExpressionInputSource::Node { output_index, .. } => output_index,
            _ => 0,
        }
    }

    pub(crate) fn as_chunk_input(&self) -> ChunkInput {
        ChunkInput::new(self.node(), self.output_index())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpressionTextureSource {
    Unbound,
    Material(&'static str),
    Bound {
        material: Option<&'static str>,
        texture_node: NodeId,
        texture_output_index: u32,
        sampler_node: NodeId,
        sampler_output_index: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpressionTexture {
    source: ExpressionTextureSource,
}

impl ExpressionTexture {
    pub const fn unbound() -> Self {
        Self {
            source: ExpressionTextureSource::Unbound,
        }
    }

    pub const fn material(name: &'static str) -> Self {
        Self {
            source: ExpressionTextureSource::Material(name),
        }
    }

    pub const fn bound(
        texture_node: NodeId,
        texture_output_index: u32,
        sampler_node: NodeId,
        sampler_output_index: u32,
    ) -> Self {
        Self {
            source: ExpressionTextureSource::Bound {
                material: None,
                texture_node,
                texture_output_index,
                sampler_node,
                sampler_output_index,
            },
        }
    }

    pub fn bind(&mut self, compiler: &mut MaterialCompiler) {
        compiler.bind_expression_texture(self);
    }

    pub(crate) fn material_name(&self) -> Option<&'static str> {
        match self.source {
            ExpressionTextureSource::Material(name) => Some(name),
            ExpressionTextureSource::Bound { material, .. } => material,
            ExpressionTextureSource::Unbound => None,
        }
    }

    pub(crate) fn is_unbound(&self) -> bool {
        matches!(self.source, ExpressionTextureSource::Unbound)
    }

    pub(crate) fn set_bound(
        &mut self,
        texture_node: NodeId,
        texture_output_index: u32,
        sampler_node: NodeId,
        sampler_output_index: u32,
    ) {
        let material = match self.source {
            ExpressionTextureSource::Material(name) => Some(name),
            ExpressionTextureSource::Bound { material, .. } => material,
            ExpressionTextureSource::Unbound => None,
        };
        self.source = ExpressionTextureSource::Bound {
            material,
            texture_node,
            texture_output_index,
            sampler_node,
            sampler_output_index,
        };
    }

    pub fn texture_node(&self) -> NodeId {
        match self.source {
            ExpressionTextureSource::Bound { texture_node, .. } => texture_node,
            ExpressionTextureSource::Unbound | ExpressionTextureSource::Material(_) => {
                debug_panic!("texture input is unbound");
                0
            }
        }
    }

    pub const fn texture_output_index(&self) -> u32 {
        match self.source {
            ExpressionTextureSource::Bound {
                texture_output_index,
                ..
            } => texture_output_index,
            _ => 0,
        }
    }

    pub fn sampler_node(&self) -> NodeId {
        match self.source {
            ExpressionTextureSource::Bound { sampler_node, .. } => sampler_node,
            ExpressionTextureSource::Unbound | ExpressionTextureSource::Material(_) => {
                debug_panic!("sampler input is unbound");
                0
            }
        }
    }

    pub const fn sampler_output_index(&self) -> u32 {
        match self.source {
            ExpressionTextureSource::Bound {
                sampler_output_index,
                ..
            } => sampler_output_index,
            _ => 0,
        }
    }

    pub(crate) fn texture_input(&self) -> ChunkInput {
        ChunkInput::new(self.texture_node(), self.texture_output_index())
    }

    pub(crate) fn sampler_input(&self) -> ChunkInput {
        ChunkInput::new(self.sampler_node(), self.sampler_output_index())
    }
}

pub trait MaterialExpression {
    fn bind_inputs(&mut self, _compiler: &mut MaterialCompiler) {}
    fn outputs(&self) -> Vec<MaterialExpressionValue>;
    fn compile(&self, compiler: &mut MaterialCompiler, output_index: u32) -> NodeId;
}

pub trait PostProcessMaterialExpression {
    fn outputs(&self) -> Vec<MaterialExpressionValue>;
    fn compile(&self, compiler: &mut PostProcessCompiler, output_index: u32) -> NodeId;
}

#[derive(Clone, Copy, Debug)]
pub struct PbrShader {
    pub diffuse: ExpressionInput<Vec3>,
    pub use_diffuse_texture: ExpressionInput<bool>,
    pub diffuse_texture: ExpressionTexture,
    pub use_normal_texture: ExpressionInput<bool>,
    pub normal_texture: ExpressionTexture,
    pub roughness: ExpressionInput<f32>,
    pub use_roughness_texture: ExpressionInput<bool>,
    pub roughness_texture: ExpressionTexture,
    pub metallic: ExpressionInput<f32>,
    pub alpha: ExpressionInput<f32>,
    pub lit: ExpressionInput<bool>,
    pub cast_shadows: ExpressionInput<bool>,
    pub grayscale_diffuse: ExpressionInput<bool>,
}

impl Default for PbrShader {
    fn default() -> Self {
        Self {
            diffuse: ExpressionInput::material("diffuse"),
            use_diffuse_texture: ExpressionInput::material("use_diffuse_texture"),
            diffuse_texture: ExpressionTexture::material("diffuse"),
            use_normal_texture: ExpressionInput::material("use_normal_texture"),
            normal_texture: ExpressionTexture::material("normal"),
            roughness: ExpressionInput::material("roughness"),
            use_roughness_texture: ExpressionInput::material("use_roughness_texture"),
            roughness_texture: ExpressionTexture::material("roughness"),
            metallic: ExpressionInput::material("metallic"),
            alpha: ExpressionInput::material("alpha"),
            lit: ExpressionInput::material("lit"),
            cast_shadows: ExpressionInput::material("cast_shadows"),
            grayscale_diffuse: ExpressionInput::material("grayscale_diffuse"),
        }
    }
}

impl MaterialExpression for PbrShader {
    fn bind_inputs(&mut self, compiler: &mut MaterialCompiler) {
        self.diffuse.bind(compiler);
        self.use_diffuse_texture.bind(compiler);
        self.diffuse_texture.bind(compiler);
        self.use_normal_texture.bind(compiler);
        self.normal_texture.bind(compiler);
        self.roughness.bind(compiler);
        self.use_roughness_texture.bind(compiler);
        self.roughness_texture.bind(compiler);
        self.metallic.bind(compiler);
        self.alpha.bind(compiler);
        self.lit.bind(compiler);
        self.cast_shadows.bind(compiler);
        self.grayscale_diffuse.bind(compiler);
    }

    fn outputs(&self) -> Vec<MaterialExpressionValue> {
        vec![MaterialExpressionValue {
            name: "out",
            value_type: MaterialValueType::Vec4,
        }]
    }

    fn compile(&self, compiler: &mut MaterialCompiler, output_index: u32) -> NodeId {
        debug_assert_eq!(output_index, 0, "output_index must be 0 for PBR shader");

        let uv = compiler.vertex_uv();
        let base_color = compiler.base_color(
            uv,
            &self.diffuse,
            &self.use_diffuse_texture,
            &self.diffuse_texture,
        );
        let roughness = compiler.roughness(
            uv,
            &self.roughness,
            &self.use_roughness_texture,
            &self.roughness_texture,
        );
        let normal = compiler.normal(uv, &self.use_normal_texture, &self.normal_texture);
        let metallic = self.metallic.node();
        let alpha = self.alpha.node();
        let lit = self.lit.node();
        let cast_shadows = self.cast_shadows.node();
        let grayscale = self.grayscale_diffuse.node();
        compiler.pbr_shader(
            base_color,
            normal,
            roughness,
            metallic,
            alpha,
            lit,
            cast_shadows,
            grayscale,
        )
    }
}

pub struct PostProcessPassthroughMaterial;

impl PostProcessMaterialExpression for PostProcessPassthroughMaterial {
    fn outputs(&self) -> Vec<MaterialExpressionValue> {
        vec![MaterialExpressionValue {
            name: "color",
            value_type: MaterialValueType::Vec4,
        }]
    }

    fn compile(&self, compiler: &mut PostProcessCompiler, output_index: u32) -> NodeId {
        debug_assert_eq!(output_index, 0, "output_index must be 0 for passthrough");
        let uv = compiler.vertex_uv();
        let (tex, sampler) = compiler.post_surface_input();
        compiler.texture_sample(tex, sampler, uv)
    }
}
