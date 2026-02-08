use crate::BoundingSphere;
use glamx::{Mat4, Vec3, Vec4};
use tracing::instrument;

#[derive(Debug, Clone, Copy)]
pub struct FrustumPlane {
    normal: Vec3,
    d: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [FrustumPlane; 6],
}

#[allow(unused)]
pub enum FrustumSide {
    Left,
    Right,
    Bottom,
    Top,
    Near,
    Far,
}

impl FrustumPlane {
    pub fn distance_to(&self, sphere: &BoundingSphere) -> f32 {
        self.normal.dot(sphere.center) + self.d
    }
}

impl Frustum {
    #[instrument(skip_all)]
    pub fn from_matrix(m: &Mat4) -> Self {
        let row0 = m.row(0);
        let row1 = m.row(1);
        let row2 = m.row(2);
        let row3 = m.row(3);

        let plane_from = |v: Vec4| {
            let normal = Vec3::new(v.x, v.y, v.z);
            let len = normal.length();
            if len > 0.0 {
                FrustumPlane {
                    normal: normal / len,
                    d: v.w / len,
                }
            } else {
                FrustumPlane { normal, d: v.w }
            }
        };

        let planes = [
            plane_from(row3 + row0), // left
            plane_from(row3 - row0), // right
            plane_from(row3 + row1), // bottom
            plane_from(row3 - row1), // top
            plane_from(row3 + row2), // near
            plane_from(row3 - row2), // far
        ];

        Frustum { planes }
    }

    pub fn side(&self, side: FrustumSide) -> &FrustumPlane {
        match side {
            FrustumSide::Left => &self.planes[0],
            FrustumSide::Right => &self.planes[1],
            FrustumSide::Bottom => &self.planes[2],
            FrustumSide::Top => &self.planes[3],
            FrustumSide::Near => &self.planes[4],
            FrustumSide::Far => &self.planes[5],
        }
    }

    #[instrument(skip_all)]
    pub fn intersects_sphere(&self, sphere: &BoundingSphere) -> bool {
        self.planes
            .iter()
            .all(|p| p.distance_to(sphere) >= -sphere.radius)
    }

    pub fn corners(&self) -> [Vec3; 8] {
        let left = self.side(FrustumSide::Left);
        let right = self.side(FrustumSide::Right);
        let bottom = self.side(FrustumSide::Bottom);
        let top = self.side(FrustumSide::Top);
        let near = self.side(FrustumSide::Near);
        let far = self.side(FrustumSide::Far);

        [
            Self::intersect_planes(near, left, bottom),
            Self::intersect_planes(near, left, top),
            Self::intersect_planes(near, right, bottom),
            Self::intersect_planes(near, right, top),
            Self::intersect_planes(far, left, bottom),
            Self::intersect_planes(far, left, top),
            Self::intersect_planes(far, right, bottom),
            Self::intersect_planes(far, right, top),
        ]
    }

    pub fn bounding_sphere(&self) -> BoundingSphere {
        BoundingSphere::from_corners(&self.corners())
    }

    fn intersect_planes(p1: &FrustumPlane, p2: &FrustumPlane, p3: &FrustumPlane) -> Vec3 {
        let n1 = p1.normal;
        let n2 = p2.normal;
        let n3 = p3.normal;

        let denom = n1.dot(n2.cross(n3));
        if denom.abs() <= 1e-6 || !denom.is_finite() {
            return Vec3::splat(f32::NAN);
        }

        let term1 = (-p1.d) * n2.cross(n3);
        let term2 = (-p2.d) * n3.cross(n1);
        let term3 = (-p3.d) * n1.cross(n2);
        (term1 + term2 + term3) / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_distance_to_sphere() {
        let plane = FrustumPlane {
            normal: Vec3::new(0.0, 1.0, 0.0),
            d: 0.0,
        };

        let sphere_above = BoundingSphere {
            center: Vec3::new(0.0, 5.0, 0.0),
            radius: 1.0,
        };

        let sphere_below = BoundingSphere {
            center: Vec3::new(0.0, -5.0, 0.0),
            radius: 1.0,
        };

        assert_eq!(plane.distance_to(&sphere_above), 5.0);
        assert_eq!(plane.distance_to(&sphere_below), -5.0);
    }

    #[test]
    fn frustum_from_identity_matrix() {
        let m = Mat4::IDENTITY;
        let frustum = Frustum::from_matrix(&m);

        let left = frustum.side(FrustumSide::Left);
        assert!((left.normal - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6);
        assert!((left.d - 1.0).abs() < 1e-6);

        let right = frustum.side(FrustumSide::Right);
        assert!((right.normal - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-6);
        assert!((right.d - 1.0).abs() < 1e-6);
    }

    #[test]
    fn intersection_test() {
        let m = Mat4::IDENTITY;
        let frustum = Frustum::from_matrix(&m);

        let sphere_inside = BoundingSphere {
            center: Vec3::ZERO,
            radius: 0.5,
        };
        assert!(frustum.intersects_sphere(&sphere_inside));

        let sphere_outside = BoundingSphere {
            center: Vec3::new(5.0, 0.0, 0.0),
            radius: 0.5,
        };
        assert!(!frustum.intersects_sphere(&sphere_outside));

        let sphere_intersecting = BoundingSphere {
            center: Vec3::new(1.2, 0.0, 0.0),
            radius: 0.5,
        };
        assert!(frustum.intersects_sphere(&sphere_intersecting));

        let sphere_far_outside = BoundingSphere {
            center: Vec3::new(2.0, 0.0, 0.0),
            radius: 0.5,
        };
        assert!(!frustum.intersects_sphere(&sphere_far_outside));
    }
}
