use syrillian_shadergen::function::PostProcessPassthroughMaterial;
use syrillian_shadergen::generator::PostProcessCompiler;

#[test]
fn compiles_post_process_passthrough() {
    let material = PostProcessPassthroughMaterial;
    let wgsl = PostProcessCompiler::compile_post_process(&material, 0);

    assert!(wgsl.contains("@vertex"));
    assert!(wgsl.contains("@fragment"));
    assert!(wgsl.contains("fn fs_main(in: FInput) -> @location(0) vec4f"));
    assert!(wgsl.contains("textureSample(postTexture, postSampler, in.uv)"));
}
