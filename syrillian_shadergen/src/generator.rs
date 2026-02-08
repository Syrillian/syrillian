use crate::chunks::{
    ConstantF32Node, EmitCtx, FunctionCallNode, MaterialBaseColorNode, MaterialInputNode,
    MaterialNormalNode, MaterialRoughnessNode, MaterialSamplerNode, MaterialTextureNode, MathNode,
    MathOp, NodeChunk, NodeId, PbrShaderNode, PickColorNode, PostSurfaceSamplerNode,
    PostSurfaceTextureNode, RawChunk, SwizzleNode, TextureSampleNode, VertexUvNode,
};
use crate::function::{MaterialExpression, PostProcessMaterialExpression};
use wgpu::TextureFormat;

pub const PICKING_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

const POST_PROCESS_VERTEX: &str = include_str!("functions/vertex_postprocess_quad.wgsl");
const MESH3D_NO_BONES_GROUP: &str = include_str!("groups/mesh3d_no_bones.wgsl");
const MESH3D_BONES_GROUP: &str = include_str!("groups/mesh3d_bones.wgsl");
const MESH3D_VERTEX_NO_BONES: &str = include_str!("functions/vertex_mesh3d_no_bones.wgsl");
const MESH3D_VERTEX_BONES: &str = include_str!("functions/vertex_mesh3d_bones.wgsl");
const MESH3D_SKINNING: &str = include_str!("functions/helpers/skinning.wgsl");
const MATH_HELPERS: &str = include_str!("functions/helpers/math.wgsl");
const MESH3D_PBR: &str = include_str!("functions/pbr_mesh3d.wgsl");

const POST_PROCESS_GROUP: &str = include_str!("groups/post_process.wgsl");
const RENDER_GROUP: &str = include_str!("groups/render.wgsl");
const MODEL_GROUP: &str = include_str!("groups/model.wgsl");
const MATERIAL_GROUP: &str = include_str!("groups/material.wgsl");
const MATERIAL_TEXTURES_GROUP: &str = include_str!("groups/material_textures.wgsl");
const LIGHT_GROUP: &str = include_str!("groups/light.wgsl");

#[derive(Debug, Copy, Clone)]
pub enum MeshSkinning {
    Skinned,
    Unskinned,
}

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
}

#[derive(Debug, Copy, Clone)]
pub struct MaterialGroupOverrides<'a> {
    pub material: &'a str,
    pub material_textures: &'a str,
}

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
                out.push_str(MESH3D_VERTEX_BONES);
                out.push('\n');
            }

            out.push_str(source);
            out
        }
        ShaderKind::Custom => {
            let uses_directive = source.lines().any(|line| line.contains("#use "));
            let has_render_group = source.contains("@group(0)");
            if !has_render_group {
                out.push_str(RENDER_GROUP);
                out.push('\n');
            }

            for line in source.lines() {
                let Some(import) = line.find("#use ") else {
                    out.push_str(line);
                    out.push('\n');
                    continue;
                };

                let group = line[import + 5..].trim();
                match group {
                    "model" => out.push_str(MODEL_GROUP),
                    "material" => out.push_str(material_group),
                    "material_textures" => out.push_str(material_textures_group),
                    "light" | "shadow" => out.push_str(LIGHT_GROUP),
                    "default_vertex" => push_default_header(&mut out),
                    "post_process" => out.push_str(POST_PROCESS_GROUP),
                    _ => {}
                }
                out.push('\n');
            }

            if fragment_only {
                out.push_str(MESH3D_VERTEX_BONES);
                out.push('\n');
            }

            if !uses_directive && has_render_group && !fragment_only {
                // If we didn't insert anything, just return the original source.
                return source.to_string();
            }

            out
        }
    }
}

fn push_default_header(out: &mut String) {
    out.push_str(MESH3D_BONES_GROUP);
    out.push('\n');
    out.push_str(MATH_HELPERS);
    out.push('\n');
    out.push_str(MESH3D_SKINNING);
    out.push('\n');
}

fn push_default_mesh_groups(out: &mut String) {
    out.push_str(MESH3D_BONES_GROUP);
    out.push('\n');
    out.push_str(MATH_HELPERS);
    out.push('\n');
    out.push_str(MESH3D_SKINNING);
    out.push('\n');
}

#[derive(Default)]
pub struct MaterialCompiler {
    nodes: Vec<RawChunk>,
}

impl MaterialCompiler {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn allocate(&mut self, node: impl NodeChunk + 'static) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(RawChunk {
            node: Box::new(node),
        });
        id
    }

    pub fn vertex_uv(&mut self) -> NodeId {
        self.allocate(VertexUvNode)
    }

    pub fn material_input(&mut self, name: impl Into<String>) -> NodeId {
        self.allocate(MaterialInputNode::new(name))
    }

    pub fn material_texture(&mut self, name: impl Into<String>) -> NodeId {
        self.allocate(MaterialTextureNode::new(name))
    }

    pub fn material_sampler(&mut self, name: impl Into<String>) -> NodeId {
        self.allocate(MaterialSamplerNode::new(name))
    }

    pub fn material_base_color(
        &mut self,
        uv: NodeId,
        color_field: impl Into<String>,
        use_texture_field: impl Into<String>,
        texture_name: impl Into<String>,
    ) -> NodeId {
        self.allocate(MaterialBaseColorNode::new(
            uv,
            color_field,
            use_texture_field,
            texture_name,
        ))
    }

    pub fn material_roughness(
        &mut self,
        uv: NodeId,
        roughness_field: impl Into<String>,
        use_texture_field: impl Into<String>,
        texture_name: impl Into<String>,
    ) -> NodeId {
        self.allocate(MaterialRoughnessNode::new(
            uv,
            roughness_field,
            use_texture_field,
            texture_name,
        ))
    }

    pub fn material_normal(
        &mut self,
        uv: NodeId,
        use_texture_field: impl Into<String>,
        texture_name: impl Into<String>,
    ) -> NodeId {
        self.allocate(MaterialNormalNode::new(uv, use_texture_field, texture_name))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pbr_shader(
        &mut self,
        base_color: NodeId,
        normal: NodeId,
        roughness: NodeId,
        metallic: NodeId,
        alpha: NodeId,
        lit: NodeId,
        cast_shadows: NodeId,
        grayscale: NodeId,
    ) -> NodeId {
        self.allocate(PbrShaderNode::new(
            base_color,
            normal,
            roughness,
            metallic,
            alpha,
            lit,
            cast_shadows,
            grayscale,
        ))
    }

    pub fn pick_color(&mut self) -> NodeId {
        self.allocate(PickColorNode)
    }

    pub fn constant_f32(&mut self, value: f32) -> NodeId {
        self.allocate(ConstantF32Node::new(value))
    }

    pub fn add(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Add))
    }

    pub fn sub(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Sub))
    }

    pub fn mul(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Mul))
    }

    pub fn div(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Div))
    }

    pub fn call(&mut self, name: impl Into<String>, args: Vec<NodeId>) -> NodeId {
        self.allocate(FunctionCallNode::new(name, args))
    }

    pub fn swizzle(&mut self, id: NodeId, component: impl Into<String>) -> NodeId {
        self.allocate(SwizzleNode::new(id, component))
    }

    pub fn compile_shader_set<M: MaterialExpression>(
        material: &M,
        skinning: MeshSkinning,
    ) -> MaterialShaderSetCode {
        let base = Self::compile_mesh(material, 0, skinning, MeshPass::Base);
        let picking = Self::compile_mesh_picking(skinning);
        let shadow = Self::compile_mesh(material, 0, skinning, MeshPass::Shadow);
        MaterialShaderSetCode {
            base,
            picking,
            shadow,
        }
    }

    pub fn compile_mesh<M: MaterialExpression>(
        material: &M,
        output_index: u32,
        skinning: MeshSkinning,
        pass: MeshPass,
    ) -> String {
        let mut compiler = Self::new();
        let output = material.compile(&mut compiler, output_index);
        compiler.build_mesh_shader(output, skinning, pass)
    }

    pub fn compile_mesh_picking(skinning: MeshSkinning) -> String {
        let mut compiler = Self::new();
        let output = compiler.pick_color();
        compiler.build_mesh_shader(output, skinning, MeshPass::Picking)
    }

    fn build_mesh_shader(&self, output: NodeId, skinning: MeshSkinning, pass: MeshPass) -> String {
        let mut out = String::new();

        out.push_str(MATH_HELPERS);

        match skinning {
            MeshSkinning::Skinned => {
                out.push_str(MESH3D_BONES_GROUP);
                out.push('\n');
                out.push('\n');
                out.push_str(MESH3D_SKINNING);
                out.push('\n');
            }
            MeshSkinning::Unskinned => {
                out.push_str(MESH3D_NO_BONES_GROUP);
                out.push('\n');
            }
        }

        out.push_str(RENDER_GROUP);
        out.push('\n');
        out.push_str(MODEL_GROUP);
        out.push('\n');

        match pass {
            MeshPass::Base | MeshPass::Shadow => {
                out.push_str(MATERIAL_GROUP);
                out.push('\n');
                out.push_str(MATERIAL_TEXTURES_GROUP);
                out.push('\n');
                out.push_str(LIGHT_GROUP);
                out.push('\n');
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

        match skinning {
            MeshSkinning::Skinned => out.push_str(MESH3D_VERTEX_BONES),
            MeshSkinning::Unskinned => out.push_str(MESH3D_VERTEX_NO_BONES),
        }
        out.push('\n');

        let ret = match pass {
            MeshPass::Picking => "@location(0) vec4f",
            MeshPass::Base | MeshPass::Shadow => "FOutput",
        };

        out.push_str("@fragment\nfn fs_main(in: FInput) -> ");
        out.push_str(ret);
        out.push_str(" {\n");

        let order = self.topo_order(output);
        let ctx = EmitCtx::new(&self.nodes);

        for id in order {
            let chunk = &self.nodes[id as usize];
            if let Some(stmt) = chunk.node.emit(id, &ctx) {
                for line in stmt.lines() {
                    out.push_str("    ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }

        let expr = ctx.expr(output);
        out.push_str("    return ");
        out.push_str(&expr);
        out.push_str(";\n");
        out.push_str("}\n");

        out
    }

    fn topo_order(&self, output: NodeId) -> Vec<NodeId> {
        let mut visited = vec![false; self.nodes.len()];
        let mut order = Vec::new();

        fn dfs(id: NodeId, nodes: &[RawChunk], visited: &mut [bool], order: &mut Vec<NodeId>) {
            let idx = id as usize;
            let chunk = nodes.get(idx).expect("Invalid NodeId");
            if visited[idx] {
                return;
            }
            visited[idx] = true;
            for dep in chunk.node.deps() {
                dfs(*dep, nodes, visited, order);
            }
            order.push(id);
        }

        dfs(output, &self.nodes, &mut visited, &mut order);
        order
    }
}

#[derive(Default)]
pub struct PostProcessCompiler {
    nodes: Vec<RawChunk>,
}

impl PostProcessCompiler {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn allocate(&mut self, node: impl NodeChunk + 'static) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(RawChunk {
            node: Box::new(node),
        });
        id
    }

    pub fn vertex_uv(&mut self) -> NodeId {
        self.allocate(VertexUvNode)
    }

    pub fn post_surface_input(&mut self) -> (NodeId, NodeId) {
        (
            self.allocate(PostSurfaceTextureNode),
            self.allocate(PostSurfaceSamplerNode),
        )
    }

    pub fn texture_sample(&mut self, texture: NodeId, sampler: NodeId, uv: NodeId) -> NodeId {
        self.allocate(TextureSampleNode::new(texture, sampler, uv))
    }

    #[allow(dead_code)]
    pub fn constant_f32(&mut self, value: f32) -> NodeId {
        self.allocate(ConstantF32Node::new(value))
    }

    pub fn add(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Add))
    }

    pub fn sub(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Sub))
    }

    pub fn mul(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Mul))
    }

    pub fn div(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.allocate(MathNode::new(a, b, MathOp::Div))
    }

    pub fn call(&mut self, name: impl Into<String>, args: Vec<NodeId>) -> NodeId {
        self.allocate(FunctionCallNode::new(name, args))
    }

    pub fn swizzle(&mut self, id: NodeId, component: impl Into<String>) -> NodeId {
        self.allocate(SwizzleNode::new(id, component))
    }

    pub fn compile_post_process<M: PostProcessMaterialExpression>(
        material: &M,
        output_index: u32,
    ) -> String {
        let mut compiler = Self::new();
        let output = material.compile(&mut compiler, output_index);
        compiler.build_post_process_shader(output)
    }

    pub fn compile_post_process_fragment<M: PostProcessMaterialExpression>(
        material: &M,
        output_index: u32,
    ) -> String {
        let mut compiler = Self::new();
        let output = material.compile(&mut compiler, output_index);
        compiler.build_post_process_fragment(output)
    }

    fn build_post_process_shader(&self, output: NodeId) -> String {
        let mut out = String::new();
        out.push_str(POST_PROCESS_GROUP);
        out.push('\n');
        out.push_str(POST_PROCESS_VERTEX);
        out.push('\n');
        out.push_str("@fragment\nfn fs_main(in: FInput) -> @location(0) vec4f {\n");

        let order = self.topo_order(output);
        let ctx = EmitCtx::new(&self.nodes);

        for id in order {
            let chunk = &self.nodes[id as usize];
            if let Some(stmt) = chunk.node.emit(id, &ctx) {
                for line in stmt.lines() {
                    out.push_str("    ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }

        let expr = ctx.expr(output);
        out.push_str("    return ");
        out.push_str(&expr);
        out.push_str(";\n");
        out.push_str("}\n");

        out
    }

    fn build_post_process_fragment(&self, output: NodeId) -> String {
        let mut out = String::new();
        out.push_str("@fragment\nfn fs_main(in: FInput) -> @location(0) vec4f {\n");

        let order = self.topo_order(output);
        let ctx = EmitCtx::new(&self.nodes);

        for id in order {
            let chunk = &self.nodes[id as usize];
            if let Some(stmt) = chunk.node.emit(id, &ctx) {
                for line in stmt.lines() {
                    out.push_str("    ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }

        let expr = ctx.expr(output);
        out.push_str("    return ");
        out.push_str(&expr);
        out.push_str(";\n");
        out.push_str("}\n");

        out
    }

    fn topo_order(&self, output: NodeId) -> Vec<NodeId> {
        let mut visited = vec![false; self.nodes.len()];
        let mut order = Vec::new();

        fn dfs(id: NodeId, nodes: &[RawChunk], visited: &mut [bool], order: &mut Vec<NodeId>) {
            let idx = id as usize;
            let chunk = nodes.get(idx).expect("Invalid NodeId");
            if visited[idx] {
                return;
            }
            visited[idx] = true;
            for dep in chunk.node.deps() {
                dfs(*dep, nodes, visited, order);
            }
            order.push(id);
        }

        dfs(output, &self.nodes, &mut visited, &mut order);
        order
    }
}
