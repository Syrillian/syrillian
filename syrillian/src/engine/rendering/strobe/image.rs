use crate::assets::{HMaterial, HShader};
use crate::core::ObjectHash;
use crate::math::{Mat4, Vec3};
use crate::rendering::proxies::MeshUniformIndex;
use crate::rendering::{RenderPassType, hash_to_rgba};
use crate::strobe::UiDrawContext;
use crate::strobe::ui_element::{Rect, UiElement};
use glamx::vec2;
use syrillian::math::{Affine3A, Vec2};

#[derive(Debug, Clone)]
pub struct UiImage {
    pub draw_order: u32,
    pub material: HMaterial,
    pub size: Vec2,
    pub object_hash: ObjectHash,
}

impl UiImage {
    pub fn new(material: HMaterial) -> Self {
        Self {
            draw_order: 0,
            material,
            size: vec2(100.0, 100.0),
            object_hash: ObjectHash::default(),
        }
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn object(mut self, hash: ObjectHash) -> Self {
        self.object_hash = hash;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageScaleMode {
    Absolute,
    Relative,
    RelativeStretch,
}

impl UiElement for UiImage {
    fn draw_order(&self) -> u32 {
        self.draw_order
    }

    fn render(&self, ctx: &mut UiDrawContext, rect: Rect) {
        self.render_internal(ctx, rect);
    }

    fn measure(&self, _ctx: &mut UiDrawContext) -> Vec2 {
        self.size
    }
}

impl UiImage {
    fn screen_matrix(&self, width: f32, height: f32, rect: Rect) -> Mat4 {
        let rect_ndc = rect / vec2(width, height);
        let center = rect_ndc.position + rect_ndc.size * 0.5;

        let scale = Vec3::new(rect_ndc.size.x, rect_ndc.size.y, 1.0);
        let translation = Vec3::new(center.x * 2.0 - 1.0, 1.0 - center.y * 2.0, 0.0);

        (Affine3A::from_translation(translation) * Affine3A::from_scale(scale)).into()
    }

    fn render_internal(&self, ctx: &mut UiDrawContext, rect: Rect) {
        let shader = match ctx.pass_type() {
            RenderPassType::Color2D => Some(ctx.cache().shader_2d()),
            RenderPassType::PickingUi => Some(ctx.cache().shader(HShader::DIM2_PICKING)),
            _ => None,
        };
        let Some(shader) = shader else {
            return;
        };

        let width = ctx.viewport_size().width.max(1) as f32;
        let height = ctx.viewport_size().height.max(1) as f32;

        let model_matrix = self.screen_matrix(width, height, rect);

        let cached_image = ctx.ui_image_data(&model_matrix).clone();

        ctx.state().queue.write_buffer(
            cached_image.uniform.buffer(MeshUniformIndex::MeshData),
            0,
            bytemuck::bytes_of(&model_matrix),
        );

        let mut pass = ctx.pass().write().unwrap();
        if !shader.activate(&mut pass, ctx.gpu_ctx()) {
            return;
        }

        if let Some(idx) = shader.bind_groups().model {
            pass.set_bind_group(idx, cached_image.uniform.bind_group(), &[]);
        }

        match ctx.pass_type() {
            RenderPassType::Color2D => {
                let material = ctx.cache().material(self.material);
                if let Some(idx) = shader.bind_groups().material {
                    pass.set_bind_group(idx, material.uniform.bind_group(), &[]);
                }
            }
            RenderPassType::PickingUi => {
                let color = hash_to_rgba(self.object_hash);
                pass.set_immediates(0, bytemuck::bytes_of(&color));
            }
            _ => {}
        }

        ctx.cache().mesh_unit_square().draw_all(&mut pass);
    }
}
