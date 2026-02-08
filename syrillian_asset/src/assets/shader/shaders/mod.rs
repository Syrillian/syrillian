macro_rules! test_shader {
    ($fn_name:ident, $name:literal => $path:literal) => {
        #[test]
        fn $fn_name() {
            use crate::Shader;
            use crate::shader::checks::validate_wgsl_source;

            let shader = Shader::new_default($name, include_str!($path)).gen_code();
            validate_wgsl_source(&shader)
                .inspect_err(|e| e.emit_to_stderr_with_path(&shader, $path))
                .unwrap();
        }
    };
}

macro_rules! test_post_shader {
    ($fn_name:ident, $name:literal => $path:literal) => {
        #[test]
        fn $fn_name() {
            use crate::Shader;
            use crate::shader::checks::validate_wgsl_source;

            let shader = Shader::new_post_process($name, include_str!($path)).gen_code();

            validate_wgsl_source(&shader)
                .inspect_err(|e| e.emit_to_stderr_with_path(&shader, $path))
                .unwrap();
        }
    };
}

macro_rules! test_custom_shader {
    ($fn_name:ident, $name:literal => $path:literal) => {
        #[test]
        fn $fn_name() {
            use crate::Shader;
            use crate::shader::checks::validate_wgsl_source;
            use crate::shader::{ShaderCode, ShaderType};

            let shader = Shader::builder()
                .shader_type(ShaderType::Custom)
                .name($name)
                .code(ShaderCode::Full(include_str!($path).to_string()))
                .build()
                .gen_code();

            validate_wgsl_source(&shader)
                .inspect_err(|e| e.emit_to_stderr_with_path(&shader, $path))
                .unwrap();
        }
    };
}

// Fundamental Shaders
test_shader!(shader_2d, "Shader 2D" => "shader2d.wgsl");
test_shader!(fallback_shader3d, "Fallback Shader 3D" => "fallback_shader3d.wgsl");
test_custom_shader!(picking_text_2d, "Text 2D Picking Shader" => "picking_text2d.wgsl");
test_custom_shader!(picking_text_3d, "Text 3D Picking Shader" => "picking_text3d.wgsl");
test_custom_shader!(picking_mesh, "Mesh Picking Shader" => "picking_mesh.wgsl");
test_custom_shader!(picking_ui, "UI Picking Shader" => "picking_ui.wgsl");
test_custom_shader!(text2d, "Text 2D Shader" => "text2d.wgsl");
test_custom_shader!(text3d, "Text 3D Shader" => "text3d.wgsl");
test_custom_shader!(debug_line2d, "Debug Line 2D" => "line.wgsl");

// Debug shaders
test_custom_shader!(debug_edges, "Debug Edges Shader" => "debug/edges.wgsl");
test_custom_shader!(debug_lines, "Debug Lines Shader" => "debug/lines.wgsl");
test_custom_shader!(debug_vertex_normals, "Debug Vertex Normals" => "debug/vertex_normals.wgsl");
test_custom_shader!(debug_text2d, "Debug Text 2D Geometry Shader" => "debug/text2d_geometry.wgsl");
test_custom_shader!(debug_text3d, "Debug Text 3D Geometry Shader" => "debug/text3d_geometry.wgsl");
test_custom_shader!(debug_light, "Debug Light Geometry Shader" => "debug/light.wgsl");

// Post-Processing Shaders
test_post_shader!(ssr_post_process, "SSR Post Process Shader" => "ssr_post_process.wgsl");

#[test]
fn fullscreen_passthrough() {
    use crate::Shader;
    use crate::shader::checks::validate_wgsl_source;
    use syrillian_shadergen::function::PostProcessPassthroughMaterial;
    use syrillian_shadergen::generator::PostProcessCompiler;

    let material = PostProcessPassthroughMaterial;
    let fs = PostProcessCompiler::compile_post_process_fragment(&material, 0);
    let shader = Shader::new_post_process_fragment("Fullscreen Passthrough Shader", fs).gen_code();

    validate_wgsl_source(&shader)
        .inspect_err(|e| e.emit_to_stderr_with_path(&shader, "shadergen/fullscreen_passthrough"))
        .unwrap();
}

#[test]
fn shadergen_mesh3d() {
    use crate::Shader;
    use crate::shader::checks::validate_wgsl_source;
    use crate::shader::{ShaderCode, ShaderType};
    use syrillian_shadergen::function::PbrShader;
    use syrillian_shadergen::generator::{MaterialCompiler, MeshPass, MeshSkinning};

    let pbr = PbrShader;
    let code = MaterialCompiler::compile_mesh(&pbr, 0, MeshSkinning::Unskinned, MeshPass::Base);
    let shader = Shader::builder()
        .shader_type(ShaderType::Custom)
        .name("Shadergen Mesh3D")
        .code(ShaderCode::Full(code))
        .build()
        .gen_code();

    validate_wgsl_source(&shader)
        .inspect_err(|e| e.emit_to_stderr_with_path(&shader, "shadergen/mesh3d"))
        .unwrap();
}
