use crate::assets::{HMaterial, HShader};
use crate::core::ObjectHash;
use crate::math::Vec3;
use crate::rendering::proxies::MeshUniformIndex;
use crate::rendering::{RenderPassType, hash_to_rgba};
use crate::strobe::UiDrawContext;
use crate::strobe::ui_element::UiElement;
use syrillian::math::Affine3A;

#[derive(Debug, Clone)]
pub struct UiImageDraw {
    pub draw_order: u32,
    pub material: HMaterial,
    pub scaling: ImageScalingMode,
    pub object_hash: ObjectHash,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageScalingMode {
    Absolute {
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
    },
    Relative {
        width: f32,
        height: f32,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
    },
    RelativeStretch {
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
    },
    Ndc {
        center: [f32; 2],
        size: [f32; 2],
    },
}

impl ImageScalingMode {
    pub fn screen_matrix(&self, window_width: f32, window_height: f32) -> Vec3 {
        match self {
            ImageScalingMode::Absolute {
                left,
                right,
                top,
                bottom,
            } => {
                let left = (*left / window_width) * 2.0 - 1.0;
                let right = (*right / window_width) * 2.0 - 1.0;
                let bottom = (*bottom / window_height) * 2.0 - 1.0;
                let top = (*top / window_height) * 2.0 - 1.0;

                let sx = (right - left) * 0.5;
                let sy = (top - bottom) * 0.5;

                let tx = (right + left) * 0.5;
                let ty = (top + bottom) * 0.5;

                Vec3::new(tx, ty, 0.0) * Vec3::new(sx, sy, 1.0)
            }
            ImageScalingMode::Relative {
                width,
                height,
                left,
                right,
                top,
                bottom,
            } => {
                let width = *width;
                let height = *height;

                let left = (*left / width) * 2.0 - 1.0;
                let right = (*right / width) * 2.0 - 1.0;
                let bottom = (*bottom / height) * 2.0 - 1.0;
                let top = (*top / height) * 2.0 - 1.0;

                let sx = (right - left) * 0.5;
                let sy = (top - bottom) * 0.5;

                let tx = (right + left) * 0.5;
                let ty = (top + bottom) * 0.5;

                Vec3::new(tx, ty, 0.0) * Vec3::new(sx, sy, 1.0)
            }
            ImageScalingMode::RelativeStretch {
                left,
                right,
                top,
                bottom,
            } => {
                let sx = right - left;
                let sy = top - bottom;

                let tx = left + right - 1.0;
                let ty = bottom + top - 1.0;

                Vec3::new(tx, ty, 0.0) * Vec3::new(sx, sy, 1.0)
            }
            ImageScalingMode::Ndc { center, size } => {
                let sx = size[0] * 0.5;
                let sy = size[1] * 0.5;
                let tx = center[0];
                let ty = center[1];

                Vec3::new(tx, ty, 0.0) * Vec3::new(sx, sy, 1.0)
            }
        }
    }
}

impl UiElement for UiImageDraw {
    fn draw_order(&self) -> u32 {
        self.draw_order
    }

    fn render(&self, ctx: &mut UiDrawContext) {
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

        let model_matrix = self.scaling.screen_matrix(width, height);
        if model_matrix.length() < f32::EPSILON {
            return;
        }

        let cached_image = ctx
            .ui_image_data(&Affine3A::from_translation(model_matrix))
            .clone();

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
