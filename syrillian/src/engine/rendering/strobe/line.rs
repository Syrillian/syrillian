use crate::assets::HShader;
use crate::core::ObjectHash;
use crate::math::{Vec2, Vec4};
use crate::rendering::{RenderPassType, hash_to_rgba};
use crate::strobe::UiDrawContext;
use crate::strobe::ui_element::{Rect, UiElement};
use crate::try_activate_shader;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiLineData {
    pub from: Vec2,
    pub to: Vec2,
    pub from_color: [f32; 4],
    pub to_color: [f32; 4],
    pub thickness: f32,
}

#[derive(Debug, Clone)]
pub struct UiLine {
    pub draw_order: u32,
    pub size: Vec2,
    pub from_color: Vec4,
    pub to_color: Vec4,
    pub thickness: f32,
    pub object_hash: ObjectHash,
}

impl UiLine {
    pub fn new(size: Vec2) -> Self {
        Self {
            draw_order: 0,
            size,
            from_color: Vec4::ONE,
            to_color: Vec4::ONE,
            thickness: 1.0,
            object_hash: ObjectHash::default(),
        }
    }

    pub fn size(mut self, from: Vec2) -> Self {
        self.size = from;
        self
    }

    pub fn color(mut self, color: Vec4) -> Self {
        self.from_color = color;
        self.to_color = color;
        self
    }

    pub fn gradient(mut self, from_color: Vec4, to_color: Vec4) -> Self {
        self.from_color = from_color;
        self.to_color = to_color;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    pub fn click_listener(mut self, hash: ObjectHash) -> Self {
        self.object_hash = hash;
        self
    }
}

impl UiElement for UiLine {
    fn draw_order(&self) -> u32 {
        self.draw_order
    }

    fn render(&self, ctx: &mut UiDrawContext, rect: Rect) {
        let shader = ctx.cache().shader(HShader::LINE_2D);

        let mut pc = UiLineData {
            from: rect.position,
            to: rect.position + rect.size,
            from_color: self.from_color.to_array(),
            to_color: self.to_color.to_array(),
            thickness: self.thickness,
        };

        if ctx.gpu_ctx().pass_type == RenderPassType::PickingUi {
            let color = hash_to_rgba(self.object_hash);
            pc.from_color = color;
            pc.to_color = color;
        }

        let mut pass = ctx.gpu_ctx().pass.write().unwrap();
        try_activate_shader!(shader, &mut pass, ctx.gpu_ctx() => return);

        pass.set_immediates(0, bytemuck::bytes_of(&pc));
        pass.draw(0..6, 0..1);
    }

    fn measure(&self, _ctx: &mut UiDrawContext) -> Vec2 {
        self.size
    }
}
