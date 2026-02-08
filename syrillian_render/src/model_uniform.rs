use glamx::{Mat4, Vec3};
use syrillian_asset::ensure_aligned;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelUniform {
    pub model_mat: Mat4,
}

ensure_aligned!(ModelUniform { model_mat }, align <= 16 * 4 => size);

impl ModelUniform {
    pub fn empty() -> Self {
        ModelUniform {
            model_mat: Mat4::IDENTITY,
        }
    }

    #[inline]
    pub fn new_at(x: f32, y: f32, z: f32) -> Self {
        Self::new_at_vec(Vec3::new(x, y, z))
    }

    pub fn new_at_vec(pos: Vec3) -> Self {
        ModelUniform {
            model_mat: Mat4::from_translation(pos),
        }
    }

    pub fn from_matrix(translation: &Mat4) -> Self {
        ModelUniform {
            model_mat: *translation,
        }
    }

    pub fn update(&mut self, transform: &Mat4) {
        self.model_mat = *transform;
    }
}
