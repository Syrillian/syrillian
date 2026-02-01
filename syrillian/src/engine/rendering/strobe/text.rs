use crate::assets::{HFont, HShader};
use crate::core::{ModelUniform, ObjectHash};
use crate::math::{Vec2, Vec3};
use crate::rendering::glyph::{GlyphRenderData, generate_glyph_geometry_stream};
use crate::rendering::proxies::{MeshUniformIndex, TextImmediates};
use crate::rendering::{RenderPassType, hash_to_rgba};
use crate::strobe::UiDrawContext;
use crate::strobe::ui_element::{Rect, UiElement};
use crate::try_activate_shader;
use crate::utils::hsv_to_rgb;
use wgpu::BufferUsages;
use wgpu::util::DeviceExt;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TextAlignment {
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone)]
pub struct UiText {
    pub draw_order: u32,
    pub font: HFont,
    pub alignment: TextAlignment,
    pub letter_spacing_em: f32,
    pub size_em: f32,
    pub color: Vec3,
    pub rainbow: bool,
    pub text: String,
    pub object_hash: ObjectHash,
}

impl UiText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            draw_order: 0,
            font: HFont::DEFAULT,
            alignment: TextAlignment::Left,
            letter_spacing_em: 0.0,
            size_em: 1.0,
            color: Vec3::ONE,
            rainbow: false,
            text: text.into(),
            object_hash: ObjectHash::default(),
        }
    }

    pub fn color(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.size_em = size;
        self
    }

    pub fn font(mut self, font: HFont) -> Self {
        self.font = font;
        self
    }

    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing_em = spacing;
        self
    }

    pub fn align(mut self, alignment: TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn rainbow(mut self, rainbow: bool) -> Self {
        self.rainbow = rainbow;
        self
    }

    pub fn click_listener(mut self, hash: ObjectHash) -> Self {
        self.object_hash = hash;
        self
    }
}

impl UiElement for UiText {
    fn draw_order(&self) -> u32 {
        self.draw_order
    }

    fn render(&self, ctx: &mut UiDrawContext, rect: Rect) {
        let pos = match self.alignment {
            TextAlignment::Left => rect.min(),
            TextAlignment::Center => rect.position + Vec2::new(rect.size.x * 0.5, 0.0),
            TextAlignment::Right => rect.position + Vec2::new(rect.size.x, 0.0),
        };

        self.render_internal(ctx, pos);
    }

    fn measure(&self, ctx: &mut UiDrawContext) -> Vec2 {
        let font = ctx.cache().font(self.font);
        font.request_glyphs(self.text.chars());

        let glyphs: Vec<GlyphRenderData> = generate_glyph_geometry_stream(
            &self.text,
            &font,
            self.alignment,
            1.0,
            self.letter_spacing_em,
        );

        if glyphs.is_empty() {
            return Vec2::ZERO;
        }

        let mut min = Vec2::new(f32::MAX, f32::MAX);
        let mut max = Vec2::new(f32::MIN, f32::MIN);

        for glyph in glyphs {
            for v in glyph.vertices() {
                min = min.min(Vec2::new(v.pos[0], v.pos[1]));
                max = max.max(Vec2::new(v.pos[0], v.pos[1]));
            }
        }

        if min.x > max.x || min.y > max.y {
            return Vec2::ZERO;
        }

        (max - min) * self.size_em
    }
}

impl UiText {
    fn render_internal(&self, ctx: &mut UiDrawContext, position: Vec2) {
        let shader = match ctx.gpu_ctx().pass_type {
            RenderPassType::Color2D => Some(ctx.cache().shader(HShader::TEXT_2D)),
            RenderPassType::PickingUi => Some(ctx.cache().shader(HShader::TEXT_2D_PICKING)),
            _ => None,
        };
        let Some(shader) = shader else {
            return;
        };

        let font = ctx.cache().font(self.font);
        font.request_glyphs(self.text.chars());
        let _ = font.pump(ctx.cache(), &ctx.state().queue, 10);

        let glyphs: Vec<GlyphRenderData> = generate_glyph_geometry_stream(
            &self.text,
            &font,
            self.alignment,
            1.0,
            self.letter_spacing_em,
        );

        if glyphs.is_empty() {
            return;
        }

        let mut cached_text = ctx.ui_text_data().clone();

        let glyph_bytes = bytemuck::cast_slice(&glyphs[..]);
        if (cached_text.glyph_vbo.size() as usize) < glyph_bytes.len() {
            cached_text.glyph_vbo =
                ctx.state()
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Strobe Text Glyphs"),
                        contents: glyph_bytes,
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    });
        } else {
            ctx.state()
                .queue
                .write_buffer(&cached_text.glyph_vbo, 0, glyph_bytes);
        }

        let model = ModelUniform::empty();
        ctx.state().queue.write_buffer(
            cached_text.uniform.buffer(MeshUniformIndex::MeshData),
            0,
            bytemuck::bytes_of(&model),
        );

        let mut pc = TextImmediates {
            position,
            em_scale: self.size_em,
            msdf_range_px: 4.0,
            color: self.color,
            padding: 0,
        };

        if self.rainbow {
            let time = ctx.start_time().elapsed().as_secs_f32() * 100.0;
            pc.color = hsv_to_rgb(time % 360.0, 1.0, 1.0);
        }

        if ctx.gpu_ctx().pass_type == RenderPassType::PickingUi {
            let color = hash_to_rgba(self.object_hash);
            pc.color = Vec3::new(color[0], color[1], color[2]);
        }

        let mut pass = ctx.gpu_ctx().pass.write().unwrap();
        try_activate_shader!(shader, &mut pass, ctx.gpu_ctx() => return);

        let groups = shader.bind_groups();
        if let Some(idx) = groups.model {
            pass.set_bind_group(idx, cached_text.uniform.bind_group(), &[]);
        }
        if let Some(idx) = groups.material {
            let material = ctx.cache().material(font.atlas());
            pass.set_bind_group(idx, material.uniform.bind_group(), &[]);
        }

        pass.set_immediates(0, bytemuck::bytes_of(&pc));
        pass.set_vertex_buffer(0, cached_text.glyph_vbo.slice(..));
        pass.draw(0..glyphs.len() as u32 * 6, 0..1);
    }
}
