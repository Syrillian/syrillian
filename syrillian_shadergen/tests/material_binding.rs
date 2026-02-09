use syrillian_shadergen::function::PbrShader;
use syrillian_shadergen::generator::{MaterialCompiler, MeshPass, MeshSkinning};

#[test]
fn recompiles_with_rebound_inputs() {
    let mut pbr = PbrShader::default();

    let unskinned =
        MaterialCompiler::compile_mesh(&mut pbr, 0, MeshSkinning::Unskinned, MeshPass::Base);
    let skinned =
        MaterialCompiler::compile_mesh(&mut pbr, 0, MeshSkinning::Skinned, MeshPass::Base);

    assert!(!unskinned.is_empty());
    assert!(!skinned.is_empty());
}
