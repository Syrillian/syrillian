use glamx::{Mat4, Vec3};
use syrillian_asset::ensure_aligned;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[repr(C)]
#[derive(Debug, Copy, Clone, Immutable, IntoBytes, FromBytes, KnownLayout)]
pub struct ModelUniform {
    pub model_mat: Mat4,
    pub normal_mat: Mat4,
}

ensure_aligned!(ModelUniform { model_mat, normal_mat }, align <= 16 * 8 => size);

impl ModelUniform {
    pub fn empty() -> Self {
        Self::from_matrix(&Mat4::IDENTITY)
    }

    #[inline]
    pub fn new_at(x: f32, y: f32, z: f32) -> Self {
        Self::new_at_vec(Vec3::new(x, y, z))
    }

    pub fn new_at_vec(pos: Vec3) -> Self {
        Self::from_matrix(&Mat4::from_translation(pos))
    }

    pub fn from_matrix(model_mat: &Mat4) -> Self {
        ModelUniform {
            model_mat: *model_mat,
            normal_mat: normal_matrix(model_mat),
        }
    }

    pub fn update(&mut self, transform: &Mat4) {
        self.model_mat = *transform;
        self.normal_mat = normal_matrix(transform);
    }
}

fn normal_matrix(model_mat: &Mat4) -> Mat4 {
    let normal_mat = model_mat.inverse().transpose();
    if normal_mat.is_finite() {
        normal_mat
    } else {
        Mat4::IDENTITY
    }
}
