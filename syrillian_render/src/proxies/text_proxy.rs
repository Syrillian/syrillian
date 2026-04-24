#[cfg(debug_assertions)]
use crate::rendering::debug_renderer::DebugRenderer;

use crate::cache::AssetCache;
#[cfg(debug_assertions)]
use crate::cache::mesh::BindMeshBuffers;
use crate::cache::glyph::{GlyphRenderData, generate_glyph_geometry_stream};
use crate::model_uniform::ModelUniform;
use crate::proxies::mesh_proxy::MeshUniformIndex;
use crate::proxies::{PROXY_PRIORITY_TRANSPARENT, SceneProxy, SceneProxyBinding};
use crate::rendering::picking::hash_to_rgba;
use crate::rendering::renderer::Renderer;
use crate::rendering::uniform::ShaderUniform;
use crate::rendering::viewport::ViewportId;
use crate::rendering::{CPUDrawCtx, GPUDrawCtx, RenderPassType};
use crate::strobe::TextAlignment;
use crate::{proxy_data, proxy_data_mut};
use delegate::delegate;
use etagere::euclid::approxeq::ApproxEq;
use glamx::Affine3A;
use glamx::{Vec2, Vec3};
use parking_lot::RwLock;
use std::any::Any;
use std::fmt::Debug;
use std::marker::PhantomData;
use syrillian_asset::shader::immediates::TextImmediate;
use syrillian_asset::{HFont, HMesh, HShader, ensure_aligned};
use syrillian_utils::color::hsv_to_rgb;
use syrillian_utils::debug_panic;
use syrillian_utils::BoundingSphere;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, RenderPass};
use zerocopy::IntoBytes;

#[derive(Debug, Clone)]
pub struct TextRenderData {
    pub uniform: ShaderUniform<MeshUniformIndex>,
    pub glyph_vbo: Buffer,
    #[cfg(debug_assertions)]
    pub bounds_uniform: Option<ShaderUniform<MeshUniformIndex>>,
}

ensure_aligned!(TextImmediate { position, color }, align <= 16 * 2 => size);

#[derive(Debug, Copy, Clone)]
pub struct ThreeD;
#[derive(Debug, Copy, Clone)]
pub struct TwoD;

pub trait TextDim<const D: u8>: Copy + Clone + Debug + Send + Sync + 'static {
    fn shader() -> HShader;
    #[cfg(debug_assertions)]
    fn debug_shader() -> HShader;
    fn dimensions() -> u8 {
        D
    }
}

#[derive(Debug, Clone)]
pub struct TextProxy<const D: u8, DIM: TextDim<D>> {
    text: String,
    alignment: TextAlignment,
    last_text_len: usize,
    glyph_data: Vec<GlyphRenderData>,
    text_dirty: bool,

    font: HFont,
    letter_spacing_em: f32,

    pc: TextImmediate,
    rainbow_mode: bool,
    constants_dirty: bool,
    translation: ModelUniform,
    model_bounding: Option<BoundingSphere>,
    local_bounding: Option<BoundingSphere>,
    bounds_dirty_local: bool,
    bounds_dirty_model: bool,
    pc_bounds_dirty: bool,
    current_render_affine: Affine3A,

    draw_order: u32,
    order_dirty: bool,

    render_target: ViewportId,

    _dim: PhantomData<DIM>,
}

impl<const D: u8, DIM: TextDim<D>> TextProxy<D, DIM> {
    pub fn new(text: String, font: HFont, em_scale: f32) -> Self {
        Self {
            text,
            alignment: TextAlignment::Left,
            last_text_len: 0,
            glyph_data: Vec::new(),
            text_dirty: false,

            font,
            letter_spacing_em: 0.0,

            pc: TextImmediate {
                em_scale,
                position: Vec2::ZERO,
                color: Vec3::ONE,
                msdf_range_px: 4.0,
                padding: 0,
            },
            rainbow_mode: false,
            constants_dirty: false,
            translation: ModelUniform::empty(),
            model_bounding: None,
            local_bounding: None,
            bounds_dirty_local: true,
            bounds_dirty_model: true,
            pc_bounds_dirty: false,
            current_render_affine: Affine3A::IDENTITY,

            draw_order: 0,
            order_dirty: false,

            render_target: ViewportId::PRIMARY,

            _dim: PhantomData,
        }
    }

    pub fn set_draw_order(&mut self, order: u32) {
        if self.draw_order == order {
            return;
        }
        self.draw_order = order;
        self.order_dirty = true;
    }

    pub fn set_render_target(&mut self, target: ViewportId) {
        if self.render_target == target {
            return;
        }
        self.render_target = target;
        self.constants_dirty = true;
    }

    delegate! {
        to self {
            #[field]
            pub fn draw_order(&self) -> u32;
            #[field]
            pub fn render_target(&self) -> ViewportId;
            #[field(&)]
            pub fn text(&self) -> &str;
            #[field]
            pub fn font(&self) -> HFont;
            #[field]
            pub fn alignment(&self) -> TextAlignment;
            #[field(letter_spacing_em)]
            pub fn letter_spacing(&self) -> f32;
            #[field]
            pub fn rainbow_mode(&self) -> bool;
        }

        to self.pc {
            #[field(em_scale)]
            pub fn size(&self) -> f32;
            #[field]
            pub fn color(&self) -> Vec3;
            #[field]
            pub fn position(&self) -> Vec2;
        }
    }

    pub fn update_game_thread(&mut self, mut ctx: CPUDrawCtx) {
        if self.constants_dirty {
            let constants = self.pc;
            let rainbow_mode = self.rainbow_mode;
            let pc_bounds_dirty = self.pc_bounds_dirty;
            ctx.send_proxy_update(move |proxy| {
                let proxy: &mut Self = proxy_data_mut!(proxy);

                proxy.pc = constants;
                proxy.rainbow_mode = rainbow_mode;
                if pc_bounds_dirty {
                    proxy.bounds_dirty_local = true;
                }
            });
            self.constants_dirty = false;
            self.pc_bounds_dirty = false;
        }

        if self.order_dirty {
            let order = self.draw_order;
            ctx.send_proxy_update(move |proxy| {
                let proxy: &mut Self = proxy_data_mut!(proxy);
                proxy.draw_order = order;
            });
            self.order_dirty = false;
        }

        if self.text_dirty {
            let text = self.text.clone();
            let font = self.font;
            let alignment = self.alignment;
            let spacing = self.letter_spacing_em;
            ctx.send_proxy_update(move |proxy| {
                let proxy: &mut Self = proxy_data_mut!(proxy);

                proxy.text = text;
                proxy.font = font;
                proxy.alignment = alignment;
                proxy.letter_spacing_em = spacing;
                proxy.text_dirty = true;
            });

            self.text_dirty = false;
        }
    }

    pub fn update_render_thread(&mut self, renderer: &Renderer, data: &mut TextRenderData) {
        let hot_font = renderer.cache.font(self.font);
        let glyphs_ready = hot_font.pump(&renderer.state.queue, 10);

        if glyphs_ready {
            self.text_dirty = true;
        }

        let expected_glyphs = self.text.matches(|c: char| !c.is_whitespace()).count();
        if self.glyph_data.len() < expected_glyphs {
            self.text_dirty = true;
        }

        if self.text_dirty {
            self.regenerate_geometry(renderer);

            if (data.glyph_vbo.size() as usize) < size_of_val(&self.glyph_data[..]) {
                data.glyph_vbo = renderer
                    .state
                    .device
                    .create_buffer_init(&BufferInitDescriptor {
                        label: Some("Text Glyph Data"),
                        contents: self.glyph_data.as_bytes(),
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    });
            } else {
                renderer
                    .state
                    .queue
                    .write_buffer(&data.glyph_vbo, 0, self.glyph_data.as_bytes());
            }

            self.last_text_len = self.text.len();
            self.text_dirty = false;
            self.bounds_dirty_local = true;
        }

        if self.rainbow_mode {
            let time = renderer.start_time().elapsed().as_secs_f32() * 100.;
            self.pc.color = hsv_to_rgb(time % 360., 1.0, 1.0);
        }

        let bounds_were_dirty = self.bounds_dirty_local || self.bounds_dirty_model;
        self.update_bounds();

        #[cfg(debug_assertions)]
        if bounds_were_dirty {
            self.sync_bounds_uniform(renderer, data);
        }
    }

    pub fn render(&self, renderer: &Renderer, data: &TextRenderData, ctx: &GPUDrawCtx) {
        if data.glyph_vbo.size() == 0 || self.text.is_empty() {
            return;
        }

        let cache: &AssetCache = &renderer.cache;
        let pass: &RwLock<RenderPass> = &ctx.pass;

        let font = cache.font(self.font);

        let shader = cache.shader(DIM::shader());
        let atlas_binding = font.atlas_binding();
        let groups = shader.bind_groups();

        let mut pass = pass.write();
        shader.activate(&mut pass, ctx);

        pass.set_vertex_buffer(0, data.glyph_vbo.slice(..));
        pass.set_immediates(0, self.pc.as_bytes());
        if let Some(idx) = groups.model {
            pass.set_bind_group(idx, data.uniform.bind_group(), &[]);
        }
        let Some(material) = groups.material else {
            debug_panic!("Text shader is missing material bind group mapping");
            return;
        };
        pass.set_bind_group(material, &atlas_binding, &[]);

        pass.draw(0..self.glyph_data.len() as u32 * 6, 0..1);

        #[cfg(debug_assertions)]
        if DebugRenderer::text_geometry() {
            self.draw_debug_edges(cache, &mut pass, ctx, &data.uniform);
        }

        #[cfg(debug_assertions)]
        if D == 3 && !ctx.transparency_pass && DebugRenderer::mesh_bounds() {
            self.draw_debug_bounds(cache, &mut pass, ctx, data);
        }
    }

    #[cfg(debug_assertions)]
    fn draw_debug_edges(
        &self,
        cache: &AssetCache,
        pass: &mut RenderPass,
        ctx: &GPUDrawCtx,
        uniform: &ShaderUniform<MeshUniformIndex>,
    ) {
        let shader = cache.shader(DIM::debug_shader());
        let groups = shader.bind_groups();

        shader.activate(pass, ctx);
        if let Some(idx) = groups.model {
            pass.set_bind_group(idx, uniform.bind_group(), &[]);
        }

        pass.set_immediates(0, self.pc.as_bytes());

        pass.draw(0..self.glyph_data.len() as u32 * 6, 0..1);
    }

    #[cfg(debug_assertions)]
    fn draw_debug_bounds(
        &self,
        cache: &AssetCache,
        pass: &mut RenderPass,
        ctx: &GPUDrawCtx,
        data: &TextRenderData,
    ) {
        use glamx::Vec4;

        const COLOR: Vec4 = Vec4::new(0.2, 1.0, 1.0, 1.0);

        let Some(bounds_uniform) = &data.bounds_uniform else {
            return;
        };

        let Some(bounds_mesh) = cache.mesh(HMesh::BOUNDS_GIZMO) else {
            return;
        };

        let shader = cache.shader(HShader::DEBUG_MESH_BOUNDS);
        shader.activate(pass, ctx);
        pass.set_immediates(0, COLOR.as_bytes());

        if let Some(idx) = shader.bind_groups().model {
            pass.set_bind_group(idx, bounds_uniform.bind_group(), &[]);
        }

        bounds_mesh.draw_all(pass, BindMeshBuffers::POSITION);
    }

    pub fn regenerate_geometry(&mut self, renderer: &Renderer) {
        let hot_font = renderer.cache.font(self.font);

        hot_font.request_glyphs(self.text.chars());

        self.glyph_data = generate_glyph_geometry_stream(
            &self.text,
            &hot_font,
            self.alignment,
            1.0,
            self.letter_spacing_em,
        );
    }

    fn update_bounds(&mut self) {
        if D != 3 {
            self.local_bounding = None;
            self.model_bounding = None;
            self.bounds_dirty_local = false;
            self.bounds_dirty_model = false;
            return;
        }

        if self.bounds_dirty_local {
            self.local_bounding = self.compute_local_bounding();
            self.bounds_dirty_local = false;
            self.bounds_dirty_model = true;
        }

        if self.bounds_dirty_model {
            let transform = self.current_render_affine.into();
            self.model_bounding = self.local_bounding.map(|b| b.transformed(&transform));
            self.bounds_dirty_model = false;
        }
    }

    fn compute_local_bounding(&self) -> Option<BoundingSphere> {
        if self.glyph_data.is_empty() || self.text.is_empty() {
            return None;
        }

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut has_points = false;

        for glyph in &self.glyph_data {
            for v in glyph.vertices() {
                let world_x = self.pc.position.x + v.pos[0] * self.pc.em_scale;
                let world_y = self.pc.position.y + v.pos[1] * self.pc.em_scale;
                let point = Vec3::new(world_x, world_y, 0.0);
                min = min.min(point);
                max = max.max(point);
                has_points = true;
            }
        }

        if !has_points || !min.is_finite() || !max.is_finite() {
            return None;
        }

        let center = (min + max) * 0.5;
        let radius = (max - center).length();
        Some(BoundingSphere { center, radius })
    }

    #[cfg(debug_assertions)]
    fn bounds_model_uniform(&self) -> Option<ModelUniform> {
        let bounds = self.model_bounding?;
        let radius = bounds.radius.abs().max(f32::EPSILON);
        let transform = glamx::Mat4::from_scale_rotation_translation(
            glamx::Vec3::splat(radius),
            glamx::Quat::IDENTITY,
            bounds.center,
        );
        Some(ModelUniform::from_matrix(&transform))
    }

    #[cfg(debug_assertions)]
    fn sync_bounds_uniform(&self, renderer: &Renderer, data: &mut TextRenderData) {
        if D != 3 {
            data.bounds_uniform = None;
            return;
        }

        if let Some(bounds_data) = self.bounds_model_uniform() {
            if let Some(bounds_uniform) = &data.bounds_uniform {
                bounds_uniform.write_buffer(
                    MeshUniformIndex::MeshData,
                    &bounds_data,
                    &renderer.state.queue,
                );
            } else {
                data.bounds_uniform = Some(
                    ShaderUniform::<MeshUniformIndex>::builder(renderer.cache.bgl_model().clone())
                        .with_buffer_data(&bounds_data)
                        .build(&renderer.state.device),
                );
            }
        } else {
            data.bounds_uniform = None;
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        let new_text = text.into();
        if self.text == new_text {
            return;
        }

        self.text = new_text;
        self.text_dirty = true;
        self.bounds_dirty_local = true;
    }

    pub fn set_font(&mut self, font: HFont) {
        if self.font == font {
            return;
        }

        self.font = font;
        self.text_dirty = true;
        self.bounds_dirty_local = true;
    }

    pub fn set_letter_spacing(&mut self, spacing_em: f32) {
        let new_spacing = spacing_em.max(0.0);
        if self.letter_spacing_em.approx_eq(&new_spacing) {
            return;
        }

        self.letter_spacing_em = new_spacing;
        self.text_dirty = true;
        self.bounds_dirty_local = true;
    }

    pub const fn set_position(&mut self, x: f32, y: f32) {
        self.set_position_vec(Vec2::new(x, y));
    }

    pub fn set_alignment(&mut self, alignment: TextAlignment) {
        if self.alignment == alignment {
            return;
        }

        self.alignment = alignment;
        self.text_dirty = true;
        self.bounds_dirty_local = true;
    }

    pub const fn set_position_vec(&mut self, pos: Vec2) {
        self.pc.position = pos;
        self.constants_dirty = true;
        self.pc_bounds_dirty = true;
    }

    pub const fn set_color(&mut self, r: f32, g: f32, b: f32) {
        self.set_color_vec(Vec3::new(r, g, b));
    }

    pub const fn set_color_vec(&mut self, color: Vec3) {
        self.pc.color = color;
        self.constants_dirty = true;
    }

    pub const fn set_size(&mut self, text_size_em: f32) {
        self.pc.em_scale = text_size_em;
        self.constants_dirty = true;
        self.pc_bounds_dirty = true;
    }

    pub const fn set_rainbow_mode(&mut self, enabled: bool) {
        self.rainbow_mode = enabled;
        self.constants_dirty = true;
    }
}

impl<const D: u8, DIM: TextDim<D>> SceneProxy for TextProxy<D, DIM> {
    fn setup_render(
        &mut self,
        renderer: &Renderer,
        render_affine: Affine3A,
        _world_affine: Option<Affine3A>,
    ) -> Box<dyn Any + Send> {
        self.regenerate_geometry(renderer);
        self.current_render_affine = render_affine;
        self.translation.update(&render_affine);
        self.bounds_dirty_local = true;
        self.bounds_dirty_model = true;
        self.update_bounds();

        let device = &renderer.state.device;

        let glyph_vbo = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Text Glyph Data"),
            contents: self.glyph_data.as_bytes(),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let model_bgl = renderer.cache.bgl_model();
        let uniform = ShaderUniform::<MeshUniformIndex>::builder(model_bgl)
            .with_buffer_data(&self.translation)
            .build(device);
        #[cfg(debug_assertions)]
        let bounds_uniform = self
            .bounds_model_uniform()
            .map(|bounds_data| {
                ShaderUniform::<MeshUniformIndex>::builder(renderer.cache.bgl_model().clone())
                    .with_buffer_data(&bounds_data)
                    .build(device)
            });

        Box::new(TextRenderData {
            uniform,
            glyph_vbo,
            #[cfg(debug_assertions)]
            bounds_uniform,
        })
    }

    fn refresh_transform(
        &mut self,
        renderer: &Renderer,
        data: &mut (dyn Any + Send),
        render_affine: Affine3A,
        _world_affine: Option<Affine3A>,
    ) {
        let data: &mut TextRenderData = proxy_data_mut!(data);

        let mesh_buffer = data.uniform.buffer(MeshUniformIndex::MeshData);
        self.translation.update(&render_affine);
        self.current_render_affine = render_affine;
        self.bounds_dirty_model = true;
        let bounds_were_dirty = self.bounds_dirty_local || self.bounds_dirty_model;
        self.update_bounds();

        renderer
            .state
            .queue
            .write_buffer(mesh_buffer, 0, self.translation.as_bytes());

        #[cfg(debug_assertions)]
        if bounds_were_dirty {
            self.sync_bounds_uniform(renderer, data);
        }
    }

    fn update_render(&mut self, renderer: &Renderer, data: &mut (dyn Any + Send)) {
        let data: &mut TextRenderData = proxy_data_mut!(data);

        self.update_render_thread(renderer, data);
    }

    fn render<'a>(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &TextRenderData = proxy_data!(binding.proxy_data());
        self.render(renderer, data, ctx);
    }

    fn render_shadows(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        if D != 3 {
            return;
        }

        let data: &TextRenderData = proxy_data!(binding.proxy_data());
        if data.glyph_vbo.size() == 0 || self.text.is_empty() {
            return;
        }

        let shader = renderer.cache.shader(HShader::TEXT_3D_SHADOW);
        let font = renderer.cache.font(self.font);
        let atlas_binding = font.atlas_binding();
        let groups = shader.bind_groups();

        let mut pass = ctx.pass.write();
        shader.activate(&mut pass, ctx);

        pass.set_vertex_buffer(0, data.glyph_vbo.slice(..));
        pass.set_immediates(0, self.pc.as_bytes());
        if let Some(idx) = groups.model {
            pass.set_bind_group(idx, data.uniform.bind_group(), &[]);
        }
        let Some(material) = groups.material else {
            debug_panic!("Text shadow shader is missing material bind group mapping");
            return;
        };
        pass.set_bind_group(material, &atlas_binding, &[]);

        pass.draw(0..self.glyph_data.len() as u32 * 6, 0..1);
    }

    fn render_picking(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        debug_assert_ne!(ctx.pass_type, RenderPassType::Shadow);

        let data: &TextRenderData = proxy_data!(binding.proxy_data());
        if data.glyph_vbo.size() == 0 || self.text.is_empty() {
            return;
        }

        let shader = match D {
            2 => renderer.cache.shader(HShader::TEXT_2D_PICKING),
            3 => renderer.cache.shader(HShader::TEXT_3D_PICKING),
            _ => {
                debug_panic!("Text Proxy Dimensions out of Bounds");
                return;
            }
        };

        let mut pass = ctx.pass.write();
        shader.activate(&mut pass, ctx);

        let font = renderer.cache.font(self.font);
        let atlas_binding = font.atlas_binding();

        let groups = shader.bind_groups();

        if let Some(model) = groups.model {
            pass.set_bind_group(model, data.uniform.bind_group(), &[]);
        }
        let Some(material) = groups.material else {
            debug_panic!("Text picking shader is missing material bind group mapping");
            return;
        };
        pass.set_bind_group(material, &atlas_binding, &[]);

        let color = hash_to_rgba(binding.object_hash);
        let mut pc = self.pc;
        pc.color = Vec3::new(color[0], color[1], color[2]);

        pass.set_immediates(0, pc.as_bytes());
        pass.set_vertex_buffer(0, data.glyph_vbo.slice(..));
        pass.draw(0..self.glyph_data.len() as u32 * 6, 0..1);
    }

    fn priority(&self, _cache: Option<&AssetCache>) -> u32 {
        match D {
            2 => self.draw_order,
            _ => PROXY_PRIORITY_TRANSPARENT,
        }
    }

    fn bounds(&self) -> Option<BoundingSphere> {
        if D == 3 {
            self.model_bounding
        } else {
            None
        }
    }
}

impl TextDim<3> for ThreeD {
    fn shader() -> HShader {
        HShader::TEXT_3D
    }

    #[cfg(debug_assertions)]
    fn debug_shader() -> HShader {
        HShader::DEBUG_TEXT3D_GEOMETRY
    }
}

impl TextDim<2> for TwoD {
    fn shader() -> HShader {
        HShader::TEXT_2D
    }

    #[cfg(debug_assertions)]
    fn debug_shader() -> HShader {
        HShader::DEBUG_TEXT2D_GEOMETRY
    }
}
