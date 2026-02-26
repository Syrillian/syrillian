use crate::mesh::generic_vertex::{Vertex, Vertex3D};
use crate::mesh::{SkinnedVertex3D, UnskinnedVertex3D};
use glamx::{Vec2, Vec3};
use std::fmt::Debug;

/// Convenience vertex used when constructing static meshes.
#[derive(Debug, Copy, Clone)]
pub struct SimpleVertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl SimpleVertex3D {
    pub const fn calculate_tangent(normal: Vec3) -> Vec3 {
        let world_up = if normal.y.abs() < 0.999 {
            Vec3::Y
        } else {
            Vec3::X
        };

        let dot = normal.x * world_up.x + normal.y * world_up.y + normal.z * world_up.z;
        let tx = world_up.x - normal.x * dot;
        let ty = world_up.y - normal.y * dot;
        let tz = world_up.z - normal.z * dot;
        Vec3::new(tx, ty, tz)
    }
    /// Converts this simplified vertex into a full [`UnskinnedVertex3D`].
    /// This is not recommended as the tangent and bitangent calculation is just a rough approximation.
    pub const fn upgrade_unskinned(self) -> UnskinnedVertex3D {
        let position = Vec3::from_array(self.position);
        let normal = Vec3::from_array(self.normal);
        let uv = Vec2::from_array(self.uv);
        let tangent = Self::calculate_tangent(normal);

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
        let unskinned = self.upgrade_unskinned();

        SkinnedVertex3D {
            position: unskinned.position,
            uv: unskinned.uv,
            normal: unskinned.normal,
            tangent: unskinned.tangent,
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

impl Vertex for SimpleVertex3D {
    fn position(&self) -> Vec3 {
        Vec3::from(self.position)
    }

    fn uv(&self) -> Vec2 {
        Vec2::from(self.uv)
    }
}

impl Vertex3D for SimpleVertex3D {
    fn normal(&self) -> Vec3 {
        Vec3::from(self.normal)
    }

    fn tangent(&self) -> Vec3 {
        Self::calculate_tangent(self.normal())
    }
}
