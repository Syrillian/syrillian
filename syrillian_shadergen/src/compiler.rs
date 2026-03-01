use crate::chunks::{
    ConstantF32Node, EmitCtx, FunctionCallNode, MaterialBaseColorNode, MaterialInputNode,
    MaterialMetallicNode, MaterialNormalNode, MaterialRoughnessNode, MaterialSamplerNode,
    MaterialTextureNode, MathNode, MathOp, NodeChunk, NodeExpressionInput, PbrShaderNode,
    PickColorNode, PostSurfaceSamplerNode, PostSurfaceTextureNode, RawChunk, SwizzleNode,
    TextureSampleNode, VertexUvNode,
};
use crate::function::{
    ExpressionInput, ExpressionTexture, MaterialExpression, MaterialPinType,
    PostProcessMaterialExpression,
};
use crate::generator::{MaterialShaderSetCode, MeshPass, ShaderCompilationOutput};
use crate::{NodeId, ShaderGenerator};
use glamx::Vec3;
use syrillian_utils::debug_panic;

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

    pub fn input_value(&mut self, name: &'static str) -> NodeId {
        self.allocate(MaterialInputNode::new(name))
    }

    pub fn bind_texture(&mut self, name: &'static str) -> NodeId {
        self.allocate(MaterialTextureNode::new(name))
    }

    pub fn bind_sampler(&mut self, name: &'static str) -> NodeId {
        self.allocate(MaterialSamplerNode::new(name))
    }

    pub fn bind_expression_input<T: MaterialPinType>(&mut self, input: &mut ExpressionInput<T>) {
        if input.is_unbound() {
            debug_panic!(
                "unbound ExpressionInput: initialize with ExpressionInput::material(...) or ExpressionInput::bound(...)"
            );
            return;
        }
        let Some(name) = input.material_name() else {
            return;
        };
        let node = self.input_value(name);
        input.set_bound(node, 0);
    }

    pub fn bind_expression_texture(&mut self, texture: &mut ExpressionTexture) {
        if texture.is_unbound() {
            debug_panic!(
                "unbound ExpressionTexture: initialize with ExpressionTexture::material(...) or ExpressionTexture::bound(...)"
            );
            return;
        }
        let Some(name) = texture.material_name() else {
            return;
        };
        let texture_node = self.bind_texture(name);
        let sampler_node = self.bind_sampler(name);
        texture.set_bound(texture_node, 0, sampler_node, 0);
    }

    pub fn base_color(
        &mut self,
        uv: NodeId,
        color: &ExpressionInput<Vec3>,
        use_texture: &ExpressionInput<bool>,
        texture: &ExpressionTexture,
    ) -> NodeId {
        self.allocate(MaterialBaseColorNode::new(
            NodeExpressionInput::new(uv, 0),
            color.as_chunk_input(),
            use_texture.as_chunk_input(),
            texture.texture_input(),
            texture.sampler_input(),
        ))
    }

    pub fn roughness(
        &mut self,
        uv: NodeId,
        roughness: &ExpressionInput<f32>,
        use_texture: &ExpressionInput<bool>,
        texture: &ExpressionTexture,
    ) -> NodeId {
        self.allocate(MaterialRoughnessNode::new(
            NodeExpressionInput::new(uv, 0),
            roughness.as_chunk_input(),
            use_texture.as_chunk_input(),
            texture.texture_input(),
            texture.sampler_input(),
        ))
    }

    pub fn metallic(
        &mut self,
        uv: NodeId,
        metallic: &ExpressionInput<f32>,
        use_texture: &ExpressionInput<bool>,
        texture: &ExpressionTexture,
    ) -> NodeId {
        self.allocate(MaterialMetallicNode::new(
            NodeExpressionInput::new(uv, 0),
            metallic.as_chunk_input(),
            use_texture.as_chunk_input(),
            texture.texture_input(),
            texture.sampler_input(),
        ))
    }

    pub fn normal(
        &mut self,
        uv: NodeId,
        use_texture: &ExpressionInput<bool>,
        texture: &ExpressionTexture,
    ) -> NodeId {
        self.allocate(MaterialNormalNode::new(
            NodeExpressionInput::new(uv, 0),
            use_texture.as_chunk_input(),
            texture.texture_input(),
            texture.sampler_input(),
        ))
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

    pub fn compile_shader_set<M: MaterialExpression>(material: &mut M) -> MaterialShaderSetCode {
        let base = Self::compile_mesh(material, 0, MeshPass::Base);
        let picking = Self::compile_mesh_picking_with_material(material, 0);
        let shadow = Self::compile_mesh(material, 0, MeshPass::Shadow);
        MaterialShaderSetCode {
            base,
            picking,
            shadow,
        }
    }

    pub fn compile_mesh<M: MaterialExpression>(
        material: &mut M,
        output_index: u32,
        pass: MeshPass,
    ) -> String {
        let mut compiler = Self::new();
        material.bind_inputs(&mut compiler);
        let output = material.compile(&mut compiler, output_index);
        let compiled = compiler.compile_output(output);
        let position_only = pass != MeshPass::Base;
        ShaderGenerator::build_mesh_shader(&compiled, pass, position_only)
    }

    pub fn compile_mesh_picking() -> String {
        let mut compiler = Self::new();
        let output = compiler.pick_color();
        let compiled = compiler.compile_output(output);
        ShaderGenerator::build_mesh_shader(&compiled, MeshPass::Picking, true)
    }

    pub fn compile_mesh_picking_with_material<M: MaterialExpression>(
        material: &mut M,
        output_index: u32,
    ) -> String {
        let mut compiler = Self::new();
        material.bind_inputs(&mut compiler);

        let material_output = material.compile(&mut compiler, output_index);
        let pick_output = compiler.pick_color();

        let compiled_material = compiler.compile_output(material_output);
        let compiled_pick = compiler.compile_output(pick_output);

        ShaderGenerator::build_mesh_picking_shader(&compiled_pick, &compiled_material, false)
    }

    fn compile_output(&self, output: NodeId) -> ShaderCompilationOutput {
        compile_output_from_nodes(&self.nodes, output)
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
        let compiled = compiler.compile_output(output);
        ShaderGenerator::build_post_process_shader(&compiled)
    }

    pub fn compile_post_process_fragment<M: PostProcessMaterialExpression>(
        material: &M,
        output_index: u32,
    ) -> String {
        let mut compiler = Self::new();
        let output = material.compile(&mut compiler, output_index);
        let compiled = compiler.compile_output(output);
        ShaderGenerator::build_post_process_fragment(&compiled)
    }

    fn compile_output(&self, output: NodeId) -> ShaderCompilationOutput {
        compile_output_from_nodes(&self.nodes, output)
    }
}

fn compile_output_from_nodes(nodes: &[RawChunk], output: NodeId) -> ShaderCompilationOutput {
    let order = topo_order_from_nodes(nodes, output);
    let ctx = EmitCtx::new(nodes);
    let mut lines = Vec::new();

    for id in order {
        let chunk = &nodes[id as usize];
        if let Some(stmt) = chunk.node.emit(id, &ctx) {
            lines.push(stmt);
        }
    }

    ShaderCompilationOutput {
        lines,
        result_expr: ctx.expr(output),
    }
}

fn topo_order_from_nodes(nodes: &[RawChunk], output: NodeId) -> Vec<NodeId> {
    let mut visited = vec![false; nodes.len()];
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

    dfs(output, nodes, &mut visited, &mut order);
    order
}
