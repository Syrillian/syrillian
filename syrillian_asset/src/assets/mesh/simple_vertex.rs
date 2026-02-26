use crate::mesh::{SkinnedVertex3D, UnskinnedVertex3D};
use glamx::{Vec2, Vec3};

/// Convenience vertex used when constructing static meshes.
#[derive(Copy, Clone)]
pub struct SimpleVertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl SimpleVertex3D {
    /// Converts this simplified vertex into a full [`UnskinnedVertex3D`].
    /// This is not recommended as the tangent and bitangent calculation is just a rough approximation.
    pub const fn upgrade_unskinned(self) -> UnskinnedVertex3D {
        let px = self.position[0];
        let py = self.position[1];
        let pz = self.position[2];
        let nx = self.normal[0];
        let ny = self.normal[1];
        let nz = self.normal[2];
        let u = self.uv[0];
        let v = self.uv[1];

        let world_up = if ny.abs() < 0.999 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };

        let dot = nx * world_up[0] + ny * world_up[1] + nz * world_up[2];
        let tx = world_up[0] - nx * dot;
        let ty = world_up[1] - ny * dot;
        let tz = world_up[2] - nz * dot;
        let tangent = Vec3::new(tx, ty, tz);

        let position = Vec3::new(px, py, pz);
        let uv = Vec2::new(u, v);
        let normal = Vec3::new(nx, ny, nz);

        UnskinnedVertex3D {
            position,
            uv,
            normal,
            tangent,
        }
    }

    /// Converts this simplified vertex into a full [`SkinnedVertex3D`].
    /// This is not recommended as the tangent and bitangent calculation is just a rough approximation.
    pub const fn upgrade_skinned(self) -> SkinnedVertex3D {
        let px = self.position[0];
        let py = self.position[1];
        let pz = self.position[2];
        let nx = self.normal[0];
        let ny = self.normal[1];
        let nz = self.normal[2];
        let u = self.uv[0];
        let v = self.uv[1];

        let world_up = if ny.abs() < 0.999 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };

        let dot = nx * world_up[0] + ny * world_up[1] + nz * world_up[2];
        let tx = world_up[0] - nx * dot;
        let ty = world_up[1] - ny * dot;
        let tz = world_up[2] - nz * dot;
        let tangent = Vec3::new(tx, ty, tz);

        let position = Vec3::new(px, py, pz);
        let uv = Vec2::new(u, v);
        let normal = Vec3::new(nx, ny, nz);

        SkinnedVertex3D {
            position,
            uv,
            normal,
            tangent,
            bone_indices: [0xFF, 0xFF, 0xFF, 0xFF],
            bone_weights: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl From<SimpleVertex3D> for UnskinnedVertex3D {
    fn from(value: SimpleVertex3D) -> Self {
        value.upgrade_unskinned()
    }
}

impl From<SimpleVertex3D> for SkinnedVertex3D {
    fn from(value: SimpleVertex3D) -> Self {
        value.upgrade_skinned()
    }
}
