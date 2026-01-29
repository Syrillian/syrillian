use crate::assets::{AssetStore, HFont, HShader};
use crate::core::ModelUniform;
use crate::core::bone::BoneData;
use crate::math::{Vec2, Vec3};
#[cfg(debug_assertions)]
use crate::rendering::DebugRenderer;
use crate::rendering::glyph::{GlyphRenderData, generate_glyph_geometry_stream};
use crate::rendering::picking::hash_to_rgba;
use crate::rendering::proxies::mesh_proxy::MeshUniformIndex;
use crate::rendering::proxies::{PROXY_PRIORITY_TRANSPARENT, SceneProxy, SceneProxyBinding};
use crate::rendering::strobe::TextAlignment;
use crate::rendering::uniform::ShaderUniform;
use crate::rendering::{AssetCache, CPUDrawCtx, GPUDrawCtx, RenderPassType, Renderer};
use crate::utils::hsv_to_rgb;
use crate::{
    ViewportId, ensure_aligned, must_pipeline, proxy_data, proxy_data_mut, try_activate_shader,
};
use delegate::delegate;
use etagere::euclid::approxeq::ApproxEq;
use glamx::Affine3A;
use std::any::Any;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::RwLock;
use syrillian_utils::debug_panic;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, RenderPass};

#[derive(Debug, Clone)]
pub struct TextRenderData {
    pub uniform: ShaderUniform<MeshUniformIndex>,
    pub glyph_vbo: Buffer,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextImmediates {
    pub position: Vec2,
    pub em_scale: f32,
    pub msdf_range_px: f32,
    pub color: Vec3,
    pub padding: u32,
}

ensure_aligned!(TextImmediates { position, color }, align <= 16 * 2 => size);

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

    pc: TextImmediates,
    rainbow_mode: bool,
    constants_dirty: bool,
    translation: ModelUniform,

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

            pc: TextImmediates {
                em_scale,
                position: Vec2::ZERO,
                color: Vec3::ONE,
                msdf_range_px: 4.0,
                padding: 0,
            },
            rainbow_mode: false,
            constants_dirty: false,
            translation: ModelUniform::empty(),

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
            ctx.send_proxy_update(move |proxy| {
                let proxy: &mut Self = proxy_data_mut!(proxy);

                proxy.pc = constants;
                proxy.rainbow_mode = rainbow_mode;
            });
            self.constants_dirty = false;
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

    pub fn update_render_thread(
        &mut self,
        renderer: &Renderer,
        data: &mut TextRenderData,
        local_to_world: &Affine3A,
    ) {
        let hot_font = renderer.cache.font(self.font);
        let glyphs_ready = hot_font.pump(&renderer.cache, &renderer.state.queue, 10);

        if glyphs_ready {
            self.text_dirty = true;
        }

        let expected_glyphs = self.text.matches(|c: char| !c.is_whitespace()).count();
        if self.glyph_data.len() < expected_glyphs {
            self.text_dirty = true;
        }

        self.translation.update(&(*local_to_world).into());

        if self.text_dirty {
            self.regenerate_geometry(renderer);

            if (data.glyph_vbo.size() as usize) < size_of_val(&self.glyph_data[..]) {
                data.glyph_vbo = renderer
                    .state
                    .device
                    .create_buffer_init(&BufferInitDescriptor {
                        label: Some("Text 2D Glyph Data"),
                        contents: bytemuck::cast_slice(&self.glyph_data[..]),
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    });
            } else {
                renderer.state.queue.write_buffer(
                    &data.glyph_vbo,
                    0,
                    bytemuck::cast_slice(&self.glyph_data[..]),
                );
            }

            self.last_text_len = self.text.len();
            self.text_dirty = false;
        }

        if self.rainbow_mode {
            let time = renderer.start_time().elapsed().as_secs_f32() * 100.;
            self.pc.color = hsv_to_rgb(time % 360., 1.0, 1.0);
        }

        let mesh_buffer = data.uniform.buffer(MeshUniformIndex::MeshData);

        renderer
            .state
            .queue
            .write_buffer(mesh_buffer, 0, bytemuck::bytes_of(&self.translation));
    }

    pub fn render(&self, renderer: &Renderer, data: &TextRenderData, ctx: &GPUDrawCtx) {
        if data.glyph_vbo.size() == 0 || self.text.is_empty() {
            return;
        }

        let cache: &AssetCache = &renderer.cache;
        let pass: &RwLock<RenderPass> = &ctx.pass;

        let font = cache.font(self.font);

        let shader = cache.shader(DIM::shader());
        let material = cache.material(font.atlas());
        let groups = shader.bind_groups();

        let mut pass = pass.write().unwrap();
        must_pipeline!(pipeline = shader, ctx.pass_type => return);

        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, data.glyph_vbo.slice(..));
        pass.set_immediates(0, bytemuck::bytes_of(&self.pc));
        pass.set_bind_group(groups.render, ctx.render_bind_group, &[]);
        if let Some(idx) = groups.model {
            pass.set_bind_group(idx, data.uniform.bind_group(), &[]);
        }
        if let Some(idx) = groups.material {
            pass.set_bind_group(idx, material.uniform.bind_group(), &[]);
        }

        pass.draw(0..self.glyph_data.len() as u32 * 6, 0..1);

        #[cfg(debug_assertions)]
        if DebugRenderer::text_geometry() {
            self.draw_debug_edges(cache, &mut pass, ctx.pass_type, ctx, &data.uniform);
        }
    }

    #[cfg(debug_assertions)]
    fn draw_debug_edges(
        &self,
        cache: &AssetCache,
        pass: &mut RenderPass,
        pass_type: RenderPassType,
        ctx: &GPUDrawCtx,
        uniform: &ShaderUniform<MeshUniformIndex>,
    ) {
        let shader = cache.shader(DIM::debug_shader());
        let groups = shader.bind_groups();
        must_pipeline!(pipeline = shader, pass_type => return);

        pass.set_pipeline(pipeline);
        pass.set_bind_group(groups.render, ctx.render_bind_group, &[]);
        if let Some(idx) = groups.model {
            pass.set_bind_group(idx, uniform.bind_group(), &[]);
        }

        pass.set_immediates(0, bytemuck::bytes_of(&self.pc));

        pass.draw(0..self.glyph_data.len() as u32 * 6, 0..1);
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

    pub fn set_text(&mut self, text: impl Into<String>) {
        let new_text = text.into();
        if self.text == new_text {
            return;
        }

        self.text = new_text;
        self.text_dirty = true;
    }

    pub fn set_font(&mut self, font: HFont) {
        if self.font == font {
            return;
        }

        self.font = font;
        self.text_dirty = true;
    }

    pub fn set_letter_spacing(&mut self, spacing_em: f32) {
        let new_spacing = spacing_em.max(0.0);
        if self.letter_spacing_em.approx_eq(&new_spacing) {
            return;
        }

        self.letter_spacing_em = new_spacing;
        self.text_dirty = true;
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
    }

    pub const fn set_position_vec(&mut self, pos: Vec2) {
        self.pc.position = pos;
        self.constants_dirty = true;
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
    }

    pub const fn set_rainbow_mode(&mut self, enabled: bool) {
        self.rainbow_mode = enabled;
        self.constants_dirty = true;
    }
}

impl<const D: u8, DIM: TextDim<D>> SceneProxy for TextProxy<D, DIM> {
    fn setup_render(&mut self, renderer: &Renderer, _local_to_world: &Affine3A) -> Box<dyn Any> {
        self.regenerate_geometry(renderer);

        let device = &renderer.state.device;

        let glyph_vbo = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Text 2D Glyph Data"),
            contents: bytemuck::cast_slice(&self.glyph_data[..]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let model_bgl = renderer.cache.bgl_model();
        let uniform = ShaderUniform::<MeshUniformIndex>::builder((*model_bgl).clone())
            .with_buffer_data(&self.translation)
            .with_buffer_data(&BoneData::DUMMY)
            .build(device);

        Box::new(TextRenderData { uniform, glyph_vbo })
    }

    fn update_render(
        &mut self,
        renderer: &Renderer,
        data: &mut dyn Any,
        local_to_world: &Affine3A,
    ) {
        let data: &mut TextRenderData = proxy_data_mut!(data);

        self.update_render_thread(renderer, data, local_to_world);
    }

    fn render<'a>(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &TextRenderData = proxy_data!(binding.proxy_data());
        self.render(renderer, data, ctx);
    }

    fn render_shadows(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        if D == 3 {
            SceneProxy::render(self, renderer, ctx, binding);
        }
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

        let mut pass = ctx.pass.write().unwrap();
        try_activate_shader!(shader, &mut pass, ctx => return);

        let font = renderer.cache.font(self.font);
        let material = renderer.cache.material(font.atlas());

        if let Some(model) = shader.bind_groups().model {
            pass.set_bind_group(model, data.uniform.bind_group(), &[]);
        }
        if let Some(material_id) = shader.bind_groups().material {
            pass.set_bind_group(material_id, material.uniform.bind_group(), &[]);
        }

        let color = hash_to_rgba(binding.object_hash);
        let mut pc = self.pc;
        pc.color = Vec3::new(color[0], color[1], color[2]);

        pass.set_immediates(0, bytemuck::bytes_of(&pc));
        pass.set_vertex_buffer(0, data.glyph_vbo.slice(..));
        pass.draw(0..self.glyph_data.len() as u32 * 6, 0..1);
    }

    fn priority(&self, _store: &AssetStore) -> u32 {
        match D {
            2 => self.draw_order,
            _ => PROXY_PRIORITY_TRANSPARENT,
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
