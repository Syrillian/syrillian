use crate::core::BoundingSphere;
use crate::math::{Mat4, Vec3, Vec4};
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
}
