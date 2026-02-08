use crate::chunks::NodeId;
use crate::generator::{MaterialCompiler, PostProcessCompiler};
use crate::value::MaterialValueType;

#[derive(Clone, Copy, Debug)]
pub struct MaterialExpressionValue {
    pub name: &'static str,
    pub value_type: MaterialValueType,
}

pub trait MaterialExpression {
    fn inputs(&self) -> Vec<MaterialExpressionValue>;
    fn outputs(&self) -> Vec<MaterialExpressionValue>;
    fn compile(&self, compiler: &mut MaterialCompiler, output_index: u32) -> NodeId;
}

pub trait PostProcessMaterialExpression {
    fn inputs(&self) -> Vec<MaterialExpressionValue>;
    fn outputs(&self) -> Vec<MaterialExpressionValue>;
    fn compile(&self, compiler: &mut PostProcessCompiler, output_index: u32) -> NodeId;
}

pub struct PbrShader;

impl MaterialExpression for PbrShader {
    fn inputs(&self) -> Vec<MaterialExpressionValue> {
        vec![
            MaterialExpressionValue {
                name: "base_color",
                value_type: MaterialValueType::Vec4,
            },
            MaterialExpressionValue {
                name: "normal",
                value_type: MaterialValueType::Vec3,
            },
            MaterialExpressionValue {
                name: "roughness",
                value_type: MaterialValueType::F32,
            },
            MaterialExpressionValue {
                name: "metallic",
                value_type: MaterialValueType::F32,
            },
            MaterialExpressionValue {
                name: "alpha",
                value_type: MaterialValueType::F32,
            },
            MaterialExpressionValue {
                name: "lit",
                value_type: MaterialValueType::Bool,
            },
            MaterialExpressionValue {
                name: "cast_shadows",
                value_type: MaterialValueType::Bool,
            },
            MaterialExpressionValue {
                name: "grayscale_diffuse",
                value_type: MaterialValueType::Bool,
            },
        ]
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
        let base_color =
            compiler.material_base_color(uv, "diffuse", "use_diffuse_texture", "diffuse");
        let roughness =
            compiler.material_roughness(uv, "roughness", "use_roughness_texture", "roughness");
        let normal = compiler.material_normal(uv, "use_normal_texture", "normal");
        let metallic = compiler.material_input("metallic");
        let alpha = compiler.material_input("alpha");
        let lit = compiler.material_input("lit");
        let cast_shadows = compiler.material_input("cast_shadows");
        let grayscale = compiler.material_input("grayscale_diffuse");
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
    fn inputs(&self) -> Vec<MaterialExpressionValue> {
        Vec::new()
    }

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
