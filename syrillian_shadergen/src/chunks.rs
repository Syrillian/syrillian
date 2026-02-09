pub type NodeId = u32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpressionInput {
    node: NodeId,
    output_index: u32,
}

impl ExpressionInput {
    pub const fn new(node: NodeId, output_index: u32) -> Self {
        Self { node, output_index }
    }

    pub const fn node(self) -> NodeId {
        self.node
    }

    pub fn expr(self, ctx: &EmitCtx) -> String {
        let value = ctx.expr(self.node);
        debug_assert_eq!(
            self.output_index, 0,
            "output_index {} is not yet implemented for WGSL node emission",
            self.output_index
        );
        value
    }
}

pub struct RawChunk {
    pub(crate) node: Box<dyn NodeChunk>,
}

pub struct EmitCtx<'a> {
    nodes: &'a [RawChunk],
}

impl<'a> EmitCtx<'a> {
    pub fn new(nodes: &'a [RawChunk]) -> Self {
        Self { nodes }
    }

    pub fn expr(&self, id: NodeId) -> String {
        let idx = id as usize;
        let chunk = self.nodes.get(idx).expect("Invalid NodeId");
        chunk.node.expr(id, self)
    }
}

pub trait NodeChunk {
    fn deps(&self) -> &[NodeId];
    fn emit(&self, id: NodeId, ctx: &EmitCtx) -> Option<String>;
    fn expr(&self, id: NodeId, ctx: &EmitCtx) -> String;
}

static EMPTY_DEPS: [NodeId; 0] = [];

pub(crate) struct VertexUvNode;

impl NodeChunk for VertexUvNode {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, _ctx: &EmitCtx) -> String {
        "in.uv".to_string()
    }
}

pub(crate) struct MaterialInputNode {
    name: String,
}

impl MaterialInputNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl NodeChunk for MaterialInputNode {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, _ctx: &EmitCtx) -> String {
        format!("material.{}", self.name)
    }
}

pub(crate) struct MaterialTextureNode {
    name: String,
}

impl MaterialTextureNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl NodeChunk for MaterialTextureNode {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, _ctx: &EmitCtx) -> String {
        format!("t_{}", self.name)
    }
}

pub(crate) struct MaterialSamplerNode {
    name: String,
}

impl MaterialSamplerNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl NodeChunk for MaterialSamplerNode {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, _ctx: &EmitCtx) -> String {
        format!("s_{}", self.name)
    }
}

pub(crate) struct MaterialBaseColorNode {
    uv: ExpressionInput,
    color: ExpressionInput,
    use_texture: ExpressionInput,
    texture: ExpressionInput,
    sampler: ExpressionInput,
    deps: [NodeId; 5],
}

impl MaterialBaseColorNode {
    pub fn new(
        uv: ExpressionInput,
        color: ExpressionInput,
        use_texture: ExpressionInput,
        texture: ExpressionInput,
        sampler: ExpressionInput,
    ) -> Self {
        Self {
            uv,
            color,
            use_texture,
            texture,
            sampler,
            deps: [
                uv.node(),
                color.node(),
                use_texture.node(),
                texture.node(),
                sampler.node(),
            ],
        }
    }
}

impl NodeChunk for MaterialBaseColorNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, id: NodeId, ctx: &EmitCtx) -> Option<String> {
        let uv = self.uv.expr(ctx);
        let color = self.color.expr(ctx);
        let use_tex = self.use_texture.expr(ctx);
        let tex = self.texture.expr(ctx);
        let sampler = self.sampler.expr(ctx);
        Some(format!(
            "var _pv{id}: vec4f;\n    if ({use_tex} != 0) {{\n        _pv{id} = textureSample({tex}, {sampler}, {uv});\n    }} else {{\n        _pv{id} = vec4f({color}, 1.0);\n    }}",
            tex = tex,
            sampler = sampler,
            uv = uv,
            color = color,
            use_tex = use_tex,
        ))
    }

    fn expr(&self, id: NodeId, _ctx: &EmitCtx) -> String {
        format!("_pv{id}")
    }
}

pub(crate) struct MaterialRoughnessNode {
    uv: ExpressionInput,
    roughness: ExpressionInput,
    use_texture: ExpressionInput,
    texture: ExpressionInput,
    sampler: ExpressionInput,
    deps: [NodeId; 5],
}

impl MaterialRoughnessNode {
    pub fn new(
        uv: ExpressionInput,
        roughness: ExpressionInput,
        use_texture: ExpressionInput,
        texture: ExpressionInput,
        sampler: ExpressionInput,
    ) -> Self {
        Self {
            uv,
            roughness,
            use_texture,
            texture,
            sampler,
            deps: [
                uv.node(),
                roughness.node(),
                use_texture.node(),
                texture.node(),
                sampler.node(),
            ],
        }
    }
}

impl NodeChunk for MaterialRoughnessNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, id: NodeId, ctx: &EmitCtx) -> Option<String> {
        let uv = self.uv.expr(ctx);
        let roughness = self.roughness.expr(ctx);
        let use_tex = self.use_texture.expr(ctx);
        let tex = self.texture.expr(ctx);
        let sampler = self.sampler.expr(ctx);
        Some(format!(
            "var _pv{id}: f32;\n    if ({use_tex} != 0) {{\n        _pv{id} = textureSample({tex}, {sampler}, {uv}).g;\n    }} else {{\n        _pv{id} = {roughness};\n    }}",
            tex = tex,
            sampler = sampler,
            uv = uv,
            roughness = roughness,
            use_tex = use_tex,
        ))
    }

    fn expr(&self, id: NodeId, _ctx: &EmitCtx) -> String {
        format!("_pv{id}")
    }
}

pub(crate) struct MaterialNormalNode {
    uv: ExpressionInput,
    use_texture: ExpressionInput,
    texture: ExpressionInput,
    sampler: ExpressionInput,
    deps: [NodeId; 4],
}

impl MaterialNormalNode {
    pub fn new(
        uv: ExpressionInput,
        use_texture: ExpressionInput,
        texture: ExpressionInput,
        sampler: ExpressionInput,
    ) -> Self {
        Self {
            uv,
            use_texture,
            texture,
            sampler,
            deps: [
                uv.node(),
                use_texture.node(),
                texture.node(),
                sampler.node(),
            ],
        }
    }
}

impl NodeChunk for MaterialNormalNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, id: NodeId, ctx: &EmitCtx) -> Option<String> {
        let uv = self.uv.expr(ctx);
        let use_tex = self.use_texture.expr(ctx);
        let tex = self.texture.expr(ctx);
        let sampler = self.sampler.expr(ctx);
        Some(format!(
            "var _pv{id}: vec3f;\n    if ({use_tex} != 0) {{\n        _pv{id} = normal_from_map({tex}, {sampler}, {uv}, in.normal, in.tangent, in.bitangent);\n    }} else {{\n        _pv{id} = safe_normalize(in.normal);\n    }}",
            tex = tex,
            sampler = sampler,
            uv = uv,
            use_tex = use_tex,
        ))
    }

    fn expr(&self, id: NodeId, _ctx: &EmitCtx) -> String {
        format!("_pv{id}")
    }
}

pub(crate) struct PostSurfaceTextureNode;

impl NodeChunk for PostSurfaceTextureNode {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, _ctx: &EmitCtx) -> String {
        "postTexture".to_string()
    }
}

pub(crate) struct PostSurfaceSamplerNode;

impl NodeChunk for PostSurfaceSamplerNode {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, _ctx: &EmitCtx) -> String {
        "postSampler".to_string()
    }
}

pub(crate) struct TextureSampleNode {
    deps: [NodeId; 3],
}

impl TextureSampleNode {
    pub fn new(texture: NodeId, sampler: NodeId, uv: NodeId) -> Self {
        Self {
            deps: [texture, sampler, uv],
        }
    }
}

impl NodeChunk for TextureSampleNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, id: NodeId, ctx: &EmitCtx) -> Option<String> {
        let tex = ctx.expr(self.deps[0]);
        let sampler = ctx.expr(self.deps[1]);
        let uv = ctx.expr(self.deps[2]);
        Some(format!(
            "let _pv{id}: vec4f = textureSample({tex}, {sampler}, {uv});"
        ))
    }

    fn expr(&self, id: NodeId, _ctx: &EmitCtx) -> String {
        format!("_pv{id}")
    }
}

pub(crate) struct ConstantF32Node {
    value: f32,
}

impl ConstantF32Node {
    pub fn new(value: f32) -> Self {
        Self { value }
    }
}

impl NodeChunk for ConstantF32Node {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        let mut literal = format!("{}", self.value);
        if !literal.contains(['.', 'e', 'E']) {
            literal.push_str(".0");
        }
        Some(format!("let _pv{id}: f32 = {literal};"))
    }

    fn expr(&self, id: NodeId, _ctx: &EmitCtx) -> String {
        format!("_pv{id}")
    }
}

pub(crate) struct PbrShaderNode {
    deps: [NodeId; 8],
}

impl PbrShaderNode {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_rgba: NodeId,
        normal: NodeId,
        roughness: NodeId,
        metallic: NodeId,
        alpha: NodeId,
        lit: NodeId,
        cast_shadows: NodeId,
        grayscale: NodeId,
    ) -> Self {
        Self {
            deps: [
                base_rgba,
                normal,
                roughness,
                metallic,
                alpha,
                lit,
                cast_shadows,
                grayscale,
            ],
        }
    }
}

impl NodeChunk for PbrShaderNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, id: NodeId, ctx: &EmitCtx) -> Option<String> {
        let base_rgba = ctx.expr(self.deps[0]);
        let normal = ctx.expr(self.deps[1]);
        let roughness = ctx.expr(self.deps[2]);
        let metallic = ctx.expr(self.deps[3]);
        let alpha = ctx.expr(self.deps[4]);
        let lit = ctx.expr(self.deps[5]);
        let cast_shadows = ctx.expr(self.deps[6]);
        let grayscale = ctx.expr(self.deps[7]);
        Some(format!(
            "let _pv{id}: FOutput = pbr_fragment(in, {base_rgba}, {normal}, {roughness}, {metallic}, {alpha}, {lit}, {cast_shadows}, {grayscale});"
        ))
    }

    fn expr(&self, id: NodeId, _ctx: &EmitCtx) -> String {
        format!("_pv{id}")
    }
}

pub(crate) struct PickColorNode;

impl NodeChunk for PickColorNode {
    fn deps(&self) -> &[NodeId] {
        &EMPTY_DEPS
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, _ctx: &EmitCtx) -> String {
        "pick.color".to_string()
    }
}

pub(crate) enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
}

pub(crate) struct MathNode {
    deps: [NodeId; 2],
    op: MathOp,
}

impl MathNode {
    pub fn new(a: NodeId, b: NodeId, op: MathOp) -> Self {
        Self { deps: [a, b], op }
    }
}

impl NodeChunk for MathNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, ctx: &EmitCtx) -> String {
        let a = ctx.expr(self.deps[0]);
        let b = ctx.expr(self.deps[1]);
        let op = match self.op {
            MathOp::Add => "+",
            MathOp::Sub => "-",
            MathOp::Mul => "*",
            MathOp::Div => "/",
        };
        format!("({a} {op} {b})")
    }
}

pub(crate) struct FunctionCallNode {
    deps: Vec<NodeId>,
    name: String,
}

impl FunctionCallNode {
    pub fn new(name: impl Into<String>, deps: Vec<NodeId>) -> Self {
        Self {
            deps,
            name: name.into(),
        }
    }
}

impl NodeChunk for FunctionCallNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, ctx: &EmitCtx) -> String {
        let args: Vec<String> = self.deps.iter().map(|&d| ctx.expr(d)).collect();
        format!("{}({})", self.name, args.join(", "))
    }
}

pub(crate) struct SwizzleNode {
    deps: [NodeId; 1],
    component: String,
}

impl SwizzleNode {
    pub fn new(id: NodeId, component: impl Into<String>) -> Self {
        Self {
            deps: [id],
            component: component.into(),
        }
    }
}

impl NodeChunk for SwizzleNode {
    fn deps(&self) -> &[NodeId] {
        &self.deps
    }

    fn emit(&self, _id: NodeId, _ctx: &EmitCtx) -> Option<String> {
        None
    }

    fn expr(&self, _id: NodeId, ctx: &EmitCtx) -> String {
        let expr = ctx.expr(self.deps[0]);
        format!("{}.{}", expr, self.component)
    }
}
