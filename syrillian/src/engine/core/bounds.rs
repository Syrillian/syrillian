use crate::math::{Mat4, Vec3, Vec4};
use std::ops::Mul;

#[derive(Debug, Copy, Clone)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl<F: Into<f32>> Mul<F> for BoundingSphere {
    type Output = BoundingSphere;

    fn mul(self, rhs: F) -> Self::Output {
        let rhs = rhs.into();
        BoundingSphere {
            center: self.center,
            radius: self.radius * rhs,
        }
    }
}

impl BoundingSphere {
    pub fn empty() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 0.0,
        }
    }

    pub fn transformed(&self, transform: &Mat4) -> Self {
        let pos = transform * Vec4::new(self.center.x, self.center.y, self.center.z, 1.0);
        let w = if pos.w.abs() > f32::EPSILON {
            pos.w
        } else {
            1.0
        };

        let center = Vec3::new(pos.x / w, pos.y / w, pos.z / w);

        let sx = transform.col(0).length();
        let sy = transform.col(1).length();
        let sz = transform.col(2).length();
        let scale = sx.max(sy).max(sz);

        Self {
            center,
            radius: self.radius * scale,
        }
    }
}
