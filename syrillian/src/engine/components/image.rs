use crate::World;
use crate::components::Component;
use crate::engine::assets::HMaterial;
use crate::rendering::UiContext;
use crate::rendering::strobe::ImageScalingMode;
use crate::strobe::UiImageDraw;
use crate::windowing::RenderTargetId;
use nalgebra::Matrix4;
use syrillian_macros::Reflect;

#[derive(Debug, Reflect)]
pub struct Image {
    material: HMaterial,
    scaling: ImageScalingMode,
    translation: Matrix4<f32>,
    draw_order: u32,
    render_target: RenderTargetId,
}

impl Image {
    pub fn scaling_mode(&self) -> ImageScalingMode {
        self.scaling
    }

    pub fn set_scaling_mode(&mut self, scaling: ImageScalingMode) {
        self.scaling = scaling;
    }

    pub fn material(&self) -> HMaterial {
        self.material
    }

    pub fn set_material(&mut self, material: HMaterial) {
        self.material = material;
    }

    pub fn set_render_target(&mut self, target: RenderTargetId) {
        self.render_target = target;
    }

    pub fn set_ndc_layout(&mut self, center: [f32; 2], size: [f32; 2]) {
        self.scaling = ImageScalingMode::Ndc { center, size };
    }

    pub fn set_translation(&mut self, translation: Matrix4<f32>) {
        self.translation = translation;
    }

    pub fn set_draw_order(&mut self, order: u32) {
        if self.draw_order == order {
            return;
        }
        self.draw_order = order;
    }

    pub fn draw_order(&self) -> u32 {
        self.draw_order
    }

    pub fn render_target(&self) -> RenderTargetId {
        self.render_target
    }

    pub fn translation(&self) -> Matrix4<f32> {
        self.translation
    }

    fn strobe_draw(&self) -> UiImageDraw {
        UiImageDraw {
            draw_order: self.draw_order(),
            material: self.material(),
            scaling: self.scaling_mode(),
            object_hash: self.parent().object_hash(),
        }
    }
}

impl Default for Image {
    fn default() -> Self {
        Image {
            material: HMaterial::FALLBACK,
            scaling: ImageScalingMode::Absolute {
                left: 0.0,
                right: 100.0,
                top: 0.0,
                bottom: 100.0,
            },
            translation: Matrix4::identity(),
            draw_order: 0,
            render_target: RenderTargetId::PRIMARY,
        }
    }
}

impl Component for Image {
    fn on_gui(&mut self, world: &mut World, ui: UiContext) {
        ui.image(world, self.render_target, self.strobe_draw());
    }
}
