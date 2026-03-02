use glamx::{Affine3A, Mat3A, Mat4, Vec3};
use std::fmt::Debug;
use syrillian_asset::ensure_aligned;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Immutable, IntoBytes, FromBytes, KnownLayout)]
pub struct ModelUniform {
    pub transform: Mat4,
    pub normal: Mat3A,
    pub object_hash: [f32; 4],
}

ensure_aligned!(ModelUniform { transform, normal, object_hash }, align <= 16 * 8 => size);

impl ModelUniform {
    pub fn empty() -> Self {
        Self::from_affine(&Affine3A::IDENTITY)
    }

    #[inline]
    pub fn new_at(x: f32, y: f32, z: f32) -> Self {
        Self::new_at_vec(Vec3::new(x, y, z))
    }

    pub fn new_at_vec(pos: Vec3) -> Self {
        Self::from_affine(&Affine3A::from_translation(pos))
    }

    pub fn from_affine(render_affine: &Affine3A) -> Self {
        Self::from_matrix(&(*render_affine).into())
    }

    pub fn from_matrix(full_trs: &Mat4) -> Self {
        ModelUniform {
            normal: normal_matrix(full_trs),
            transform: *full_trs,
            object_hash: [0.0; 4],
        }
    }

    pub fn update(&mut self, full_trs: &Affine3A) {
        let full_trs = (*full_trs).into();
        self.normal = normal_matrix(&full_trs);
        self.transform = full_trs;
    }
}

fn normal_matrix(model_mat: &Mat4) -> Mat3A {
    let normal_mat = Mat3A::from_mat4(*model_mat).inverse().transpose();
    if normal_mat.is_finite() {
        normal_mat
    } else {
        Mat3A::IDENTITY
    }
}
