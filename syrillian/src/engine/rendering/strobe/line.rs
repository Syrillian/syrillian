use crate::assets::HShader;
use crate::core::ObjectHash;
use crate::math::{Vec2, Vec4};
use crate::rendering::{RenderPassType, hash_to_rgba};
use crate::strobe::UiDrawContext;
use crate::try_activate_shader;
use syrillian::strobe::ui_element::UiElement;

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
pub struct UiLineDraw {
    pub draw_order: u32,
    pub from: Vec2,
    pub to: Vec2,
    pub from_color: Vec4,
    pub to_color: Vec4,
    pub thickness: f32,
    pub object_hash: ObjectHash,
}

impl UiElement for UiLineDraw {
    fn draw_order(&self) -> u32 {
        self.draw_order
    }

    fn render(&self, ctx: &mut UiDrawContext) {
        let shader = ctx.cache().shader(HShader::LINE_2D);

        let mut pc = UiLineData {
            from: self.from,
            to: self.to,
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
}
