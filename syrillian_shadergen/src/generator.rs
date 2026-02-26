const POST_PROCESS_VERTEX: &str = include_str!("functions/vertex_postprocess_quad.wgsl");
const MESH3D_GROUP: &str = include_str!("groups/mesh3d.wgsl");
const MESH3D_VERTEX: &str = include_str!("functions/vertex_mesh3d.wgsl");
const MATH_HELPERS: &str = include_str!("functions/helpers/math.wgsl");
const MESH3D_PBR: &str = include_str!("functions/pbr_mesh3d.wgsl");

const POST_PROCESS_GROUP: &str = include_str!("groups/post_process.wgsl");
const RENDER_GROUP: &str = include_str!("groups/render.wgsl");
const MODEL_GROUP: &str = include_str!("groups/model.wgsl");
const MATERIAL_GROUP: &str = include_str!("groups/material.wgsl");
const MATERIAL_TEXTURES_GROUP: &str = include_str!("groups/material_textures.wgsl");
const LIGHT_GROUP: &str = include_str!("groups/light.wgsl");

#[derive(Debug, Copy, Clone)]
pub enum MeshPass {
    Base,
    Picking,
    Shadow,
}

pub struct MaterialShaderSetCode {
    pub base: String,
    pub picking: String,
    pub shadow: String,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ShaderKind {
    Default,
    Custom,
    PostProcess,
    Compute,
}

#[derive(Debug, Copy, Clone)]
pub struct MaterialGroupOverrides<'a> {
    pub material: &'a str,
    pub material_textures: &'a str,
}

#[derive(Debug, Clone)]
pub struct ShaderCompilationOutput {
    pub lines: Vec<String>,
    pub result_expr: String,
}

pub struct ShaderGenerator;

impl ShaderGenerator {
    pub fn assemble_shader(
        source: &str,
        fragment_only: bool,
        kind: ShaderKind,
        depth_enabled: bool,
        material_groups: Option<MaterialGroupOverrides<'_>>,
    ) -> String {
        let (material_group, material_textures_group) = material_groups
            .map(|g| (g.material, g.material_textures))
            .unwrap_or((MATERIAL_GROUP, MATERIAL_TEXTURES_GROUP));

        let mut out = String::new();

        out.push_str(MATH_HELPERS);
        out.push('\n');

        match kind {
            ShaderKind::PostProcess => {
                out.push_str(RENDER_GROUP);
                out.push('\n');
                out.push_str(POST_PROCESS_GROUP);
                out.push('\n');

                if fragment_only {
                    out.push_str(POST_PROCESS_VERTEX);
                    out.push('\n');
                }

                out.push_str(source);
                out
            }
            ShaderKind::Default => {
                out.push_str(RENDER_GROUP);
                out.push('\n');
                push_default_mesh_groups(&mut out);
                out.push('\n');
                out.push_str(MODEL_GROUP);
                out.push('\n');
                out.push_str(material_group);
                out.push('\n');
                out.push_str(material_textures_group);
                out.push('\n');

                if depth_enabled {
                    out.push_str(LIGHT_GROUP);
                    out.push('\n');
                }

                if fragment_only {
                    out.push_str(MESH3D_VERTEX);
                    out.push('\n');
                }

                out.push_str(source);
                out
            }
            ShaderKind::Custom => {
                let mut imported = ImportedGroups {
                    render: source.contains("@group(0)"),
                    ..ImportedGroups::default()
                };
                if !imported.render {
                    append_block_once(&mut out, &mut imported.render, RENDER_GROUP);
                    out.push('\n');
                }

                expand_use_directives(
                    &mut out,
                    source,
                    UseDirectiveOptions {
                        material_group,
                        material_textures_group,
                        copy_non_directive_lines: true,
                        allow_default_vertex: true,
                        append_newline_after_directive: true,
                    },
                    &mut imported,
                );

                if fragment_only {
                    out.push_str(MESH3D_VERTEX);
                    out.push('\n');
                }

                out
            }
            ShaderKind::Compute => {
                if has_use_directive(source) {
                    let mut imported = ImportedGroups::default();
                    expand_use_directives(
                        &mut out,
                        source,
                        UseDirectiveOptions {
                            material_group,
                            material_textures_group,
                            copy_non_directive_lines: true,
                            allow_default_vertex: false,
                            append_newline_after_directive: false,
                        },
                        &mut imported,
                    );
                } else {
                    out.push_str(source);
                }

                out.push('\n');

                out
            }
        }
    }

    pub fn build_mesh_shader(compiled: &ShaderCompilationOutput, pass: MeshPass) -> String {
        let mut out = String::new();

        out.push_str(MESH3D_GROUP);
        out.push('\n');

        out.push_str(RENDER_GROUP);
        out.push('\n');
        out.push_str(MODEL_GROUP);
        out.push('\n');

        match pass {
            MeshPass::Base | MeshPass::Shadow => {
                out.push_str("#use material\n");
                out.push_str("#use material_textures\n");
                out.push_str("#use light\n");
            }
            MeshPass::Picking => {}
        }
        out.push('\n');

        if matches!(pass, MeshPass::Base | MeshPass::Shadow) {
            out.push_str(MESH3D_PBR);
            out.push('\n');
        }

        if matches!(pass, MeshPass::Picking) {
            out.push_str("struct PickColor {\n    color: vec4<f32>,\n};\n\n");
            out.push_str("var<immediate> pick: PickColor;\n\n");
        }

        out.push_str(MESH3D_VERTEX);
        out.push('\n');

        let ret = match pass {
            MeshPass::Picking => "@location(0) vec4f",
            MeshPass::Base | MeshPass::Shadow => "FOutput",
        };

        out.push_str("@fragment\nfn fs_main(in: FInput) -> ");
        out.push_str(ret);
        out.push_str(" {\n");
        append_compilation_output(&mut out, compiled);
        out.push_str("}\n");

        out
    }

    pub fn build_post_process_shader(compiled: &ShaderCompilationOutput) -> String {
        let mut out = String::new();
        out.push_str(POST_PROCESS_GROUP);
        out.push('\n');
        out.push_str(POST_PROCESS_VERTEX);
        out.push('\n');
        out.push_str("@fragment\nfn fs_main(in: FInput) -> @location(0) vec4f {\n");
        append_compilation_output(&mut out, compiled);
        out.push_str("}\n");
        out
    }

    pub fn build_post_process_fragment(compiled: &ShaderCompilationOutput) -> String {
        let mut out = String::new();
        out.push_str("@fragment\nfn fs_main(in: FInput) -> @location(0) vec4f {\n");
        append_compilation_output(&mut out, compiled);
        out.push_str("}\n");
        out
    }
}

pub fn assemble_shader(
    source: &str,
    fragment_only: bool,
    kind: ShaderKind,
    depth_enabled: bool,
    material_groups: Option<MaterialGroupOverrides<'_>>,
) -> String {
    ShaderGenerator::assemble_shader(source, fragment_only, kind, depth_enabled, material_groups)
}

pub fn assemble_compute_shader(source: &str) -> String {
    ShaderGenerator::assemble_shader(source, false, ShaderKind::Compute, false, None)
}

fn push_default_mesh_groups(out: &mut String) {
    out.push_str(MESH3D_GROUP);
    out.push('\n');
}

#[derive(Debug, Default)]
struct ImportedGroups {
    render: bool,
    model: bool,
    material: bool,
    material_textures: bool,
    light: bool,
    default_vertex: bool,
    post_process: bool,
}

#[derive(Debug, Copy, Clone)]
struct UseDirectiveOptions<'a> {
    material_group: &'a str,
    material_textures_group: &'a str,
    copy_non_directive_lines: bool,
    allow_default_vertex: bool,
    append_newline_after_directive: bool,
}

fn has_use_directive(source: &str) -> bool {
    source.lines().any(|line| line.contains("#use "))
}

fn expand_use_directives(
    out: &mut String,
    source: &str,
    options: UseDirectiveOptions<'_>,
    imported: &mut ImportedGroups,
) {
    for line in source.lines() {
        let Some(import) = line.find("#use ") else {
            if options.copy_non_directive_lines {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        };

        let group = line[import + 5..].trim();
        append_use_group(
            out,
            imported,
            group,
            options.material_group,
            options.material_textures_group,
            options.allow_default_vertex,
        );

        if options.append_newline_after_directive {
            out.push('\n');
        }
    }
}

fn append_use_group(
    out: &mut String,
    imported: &mut ImportedGroups,
    group: &str,
    material_group: &str,
    material_textures_group: &str,
    allow_default_vertex: bool,
) {
    match group {
        "render" => append_block_once(out, &mut imported.render, RENDER_GROUP),
        "model" => append_block_once(out, &mut imported.model, MODEL_GROUP),
        "material" => append_block_once(out, &mut imported.material, material_group),
        "material_textures" => append_block_once(
            out,
            &mut imported.material_textures,
            material_textures_group,
        ),
        "light" | "shadow" => append_block_once(out, &mut imported.light, LIGHT_GROUP),
        "post_process" => append_block_once(out, &mut imported.post_process, POST_PROCESS_GROUP),
        "default_vertex" if allow_default_vertex => {
            if !imported.default_vertex {
                push_default_mesh_groups(out);
                imported.default_vertex = true;
            }
        }
        _ => {}
    }
}

fn append_block_once(out: &mut String, is_included: &mut bool, block: &str) {
    if *is_included {
        return;
    }

    out.push_str(block);
    *is_included = true;
}

fn append_compilation_output(out: &mut String, compiled: &ShaderCompilationOutput) {
    for stmt in &compiled.lines {
        for line in stmt.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }

    out.push_str("    return ");
    out.push_str(&compiled.result_expr);
    out.push_str(";\n");
}
