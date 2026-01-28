use crate::assets::HFont;
use crate::core::{ModelUniform, ObjectHash};
use crate::math::{Vec2, Vec3};
use crate::rendering::glyph::{GlyphRenderData, generate_glyph_geometry_stream};
use crate::rendering::proxies::{MeshUniformIndex, TextImmediates};
use crate::rendering::{RenderPassType, hash_to_rgba};
use crate::strobe::UiDrawContext;
use crate::strobe::ui_element::UiElement;
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
pub struct UiTextDraw {
    pub draw_order: u32,
    pub font: HFont,
    pub alignment: TextAlignment,
    pub letter_spacing_em: f32,
    pub position: Vec2,
    pub size_em: f32,
    pub color: Vec3,
    pub rainbow: bool,
    pub text: String,
    pub object_hash: ObjectHash,
}

impl UiElement for UiTextDraw {
    fn draw_order(&self) -> u32 {
        self.draw_order
    }

    fn render(&self, ctx: &mut UiDrawContext) {
        let shader = match ctx.gpu_ctx().pass_type {
            RenderPassType::Color2D => Some(ctx.cache().shader(crate::assets::HShader::TEXT_2D)),
            RenderPassType::PickingUi => {
                Some(ctx.cache().shader(crate::assets::HShader::TEXT_2D_PICKING))
            }
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
            position: self.position,
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
