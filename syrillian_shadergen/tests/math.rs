use syrillian_shadergen::chunks::NodeId;
use syrillian_shadergen::function::{MaterialExpressionValue, PostProcessMaterialExpression};
use syrillian_shadergen::generator::PostProcessCompiler;
use syrillian_shadergen::value::MaterialValueType;

struct MathTestMaterial;

impl PostProcessMaterialExpression for MathTestMaterial {
    fn inputs(&self) -> Vec<MaterialExpressionValue> {
        Vec::new()
    }

    fn outputs(&self) -> Vec<MaterialExpressionValue> {
        vec![MaterialExpressionValue {
            name: "color",
            value_type: MaterialValueType::Vec4,
        }]
    }

    fn compile(&self, compiler: &mut PostProcessCompiler, _output_index: u32) -> NodeId {
        let uv = compiler.vertex_uv();
        let (tex, sampler) = compiler.post_surface_input();
        let color = compiler.texture_sample(tex, sampler, uv);
        let factor = compiler.constant_f32(0.5);
        compiler.mul(color, factor)
    }
}

#[test]
fn compiles_math_multiplication() {
    let material = MathTestMaterial;
    let wgsl = PostProcessCompiler::compile_post_process(&material, 0);

    assert!(wgsl.contains("(_pv3 * _pv4)"));
}
