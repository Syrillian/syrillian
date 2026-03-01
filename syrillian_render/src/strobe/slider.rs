use crate::ObjectHash;
use crate::rendering::RenderPassType;
use crate::rendering::picking::hash_to_rgba;
use crate::strobe::UiDrawContext;
use crate::strobe::ui_element::{Rect, UiElement};
use glamx::{Vec2, Vec4};
use syrillian_asset::HShader;
use syrillian_asset::shader::immediates::UiLineImmediate;
use zerocopy::IntoBytes;

#[derive(Debug, Clone)]
pub struct UiSlider {
    pub draw_order: u32,
    pub size: Vec2,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub track_color: Vec4,
    pub fill_color: Vec4,
    pub knob_color: Vec4,
    pub track_thickness: f32,
    pub knob_thickness: f32,
    pub object_hash: ObjectHash,
}

impl UiSlider {
    pub fn new(value: f32, min: f32, max: f32) -> Self {
        Self {
            draw_order: 0,
            size: Vec2::new(240.0, 14.0),
            value,
            min,
            max,
            track_color: Vec4::new(0.18, 0.20, 0.24, 1.0),
            fill_color: Vec4::new(0.31, 0.66, 1.0, 1.0),
            knob_color: Vec4::new(0.96, 0.96, 0.96, 1.0),
            track_thickness: 4.0,
            knob_thickness: 8.0,
            object_hash: ObjectHash::default(),
        }
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn colors(mut self, track: Vec4, fill: Vec4, knob: Vec4) -> Self {
        self.track_color = track;
        self.fill_color = fill;
        self.knob_color = knob;
        self
    }

    pub fn track_thickness(mut self, px: f32) -> Self {
        self.track_thickness = px.max(1.0);
        self
    }

    pub fn knob_thickness(mut self, px: f32) -> Self {
        self.knob_thickness = px.max(1.0);
        self
    }

    pub fn click_listener(mut self, hash: ObjectHash) -> Self {
        self.object_hash = hash;
        self
    }

    fn normalized(&self) -> f32 {
        let span = self.max - self.min;
        if span.abs() <= f32::EPSILON {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }
}

impl UiElement for UiSlider {
    fn draw_order(&self) -> u32 {
        self.draw_order
    }

    fn render(&self, ctx: &mut UiDrawContext, rect: Rect) {
        let left = rect.position.x;
        let right = rect.position.x + rect.size.x.max(1.0);
        let center_y = rect.position.y + rect.size.y * 0.5;
        let t = self.normalized();
        let handle_x = left + (right - left) * t;

        let mut track_color = self.track_color;
        let mut fill_color = self.fill_color;
        let mut knob_color = self.knob_color;

        if ctx.gpu_ctx().pass_type == RenderPassType::PickingUi {
            let color = hash_to_rgba(self.object_hash);
            let color = Vec4::new(color[0], color[1], color[2], color[3]);
            track_color = color;
            fill_color = color;
            knob_color = color;
        }

        let track_thickness = self.track_thickness.min(rect.size.y.max(1.0));
        let knob_thickness = self.knob_thickness.min(rect.size.x.max(1.0));

        let shader = ctx.cache().shader(HShader::LINE_2D);

        let mut pass = ctx.gpu_ctx().pass.write();

        shader.activate_ui(&mut pass, ctx);

        emit_line(
            &mut pass,
            Vec2::new(left, center_y),
            Vec2::new(right, center_y),
            track_color,
            track_color,
            track_thickness,
        );

        emit_line(
            &mut pass,
            Vec2::new(left, center_y),
            Vec2::new(handle_x, center_y),
            fill_color,
            fill_color,
            track_thickness,
        );

        emit_line(
            &mut pass,
            Vec2::new(handle_x, rect.position.y),
            Vec2::new(handle_x, rect.position.y + rect.size.y),
            knob_color,
            knob_color,
            knob_thickness,
        );
    }

    fn measure(&self, _ctx: &mut UiDrawContext) -> Vec2 {
        self.size
    }
}

fn emit_line(
    pass: &mut wgpu::RenderPass<'_>,
    from: Vec2,
    to: Vec2,
    from_color: Vec4,
    to_color: Vec4,
    thickness: f32,
) {
    let pc = UiLineImmediate {
        from,
        to,
        from_color: from_color.to_array(),
        to_color: to_color.to_array(),
        thickness,
    };
    pass.set_immediates(0, pc.as_bytes());
    pass.draw(0..6, 0..1);
}
