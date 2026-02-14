use glamx::{Mat4, Vec3, Vec4};
use std::ops::Mul;

/// AABB
#[derive(Debug, Copy, Clone)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl<F: Into<f32>> Mul<F> for BoundingBox {
    type Output = BoundingBox;

    fn mul(self, rhs: F) -> Self::Output {
        let rhs = rhs.into();
        BoundingBox {
            min: self.min * rhs,
            max: self.max * rhs,
        }
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            min: -Vec3::ONE,
            max: Vec3::ONE,
        }
    }
}

impl BoundingBox {
    /// The matrix is guaranteed to be affine
    pub fn transformed_affine(&self, transform: &Mat4) -> Self {
        let mut new_min = Vec3::splat(f32::INFINITY);
        let mut new_max = Vec3::splat(f32::NEG_INFINITY);

        for i in 0..8 {
            let x = if i & 1 == 0 { self.min.x } else { self.max.x };
            let y = if i & 2 == 0 { self.min.y } else { self.max.y };
            let z = if i & 4 == 0 { self.min.z } else { self.max.z };

            let point = Vec4::new(x, y, z, 1.0);
            let transformed = transform * point;

            let transformed_point = Vec3::new(transformed.x, transformed.y, transformed.z);

            new_min = new_min.min(transformed_point);
            new_max = new_max.max(transformed_point);
        }

        Self {
            min: new_min,
            max: new_max,
        }
    }

    pub fn transformed(&self, transform: &Mat4) -> Self {
        let mut new_min = Vec3::splat(f32::INFINITY);
        let mut new_max = Vec3::splat(f32::NEG_INFINITY);

        for i in 0..8 {
            let x = if i & 1 == 0 { self.min.x } else { self.max.x };
            let y = if i & 2 == 0 { self.min.y } else { self.max.y };
            let z = if i & 4 == 0 { self.min.z } else { self.max.z };

            let point = Vec4::new(x, y, z, 1.0);
            let transformed = transform * point;

            let transformed_point = Vec3::new(
                transformed.x / transformed.w,
                transformed.y / transformed.w,
                transformed.z / transformed.w,
            );

            new_min = new_min.min(transformed_point);
            new_max = new_max.max(transformed_point);
        }

        Self {
            min: new_min,
            max: new_max,
        }
    }

    pub fn from_min_max(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }
}

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

impl Default for BoundingSphere {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 1.0,
        }
    }
}

impl BoundingSphere {
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

    pub fn from_corners(corners: &[Vec3; 8]) -> Self {
        let mut center = Vec3::ZERO;
        let mut count = 0;
        for corner in corners {
            if corner.is_finite() {
                center += corner;
                count += 1;
            }
        }
        if count == 0 {
            return BoundingSphere::default();
        }
        center /= count as f32;

        let mut radius: f32 = 0.0;
        for corner in corners {
            if corner.is_finite() {
                radius = radius.max((corner - center).length());
            }
        }
        if !radius.is_finite() {
            radius = 1.0;
        }

        Self { center, radius }
    }
}
