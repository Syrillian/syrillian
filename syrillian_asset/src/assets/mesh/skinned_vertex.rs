use crate::mesh::generic_vertex::{Vertex, Vertex3D};
use glam::Vec4;
use glamx::{Vec2, Vec3};
use std::fmt::{Debug, Formatter};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unalign};

/// A fully featured skinned vertex used for 3D rendering.
#[repr(C)]
#[derive(Copy, Clone, Immutable, IntoBytes, FromBytes, KnownLayout)]
pub struct SkinnedVertex3D {
    pub position: Vec3,
    pub uv: Vec2,
    pub normal: Vec3,
    pub tangent: Unalign<Vec4>,
    pub bone_indices: [u16; 4],
    pub bone_weights: [f32; 4],
}

impl Debug for SkinnedVertex3D {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkinnedVertex3D")
            .field("position", &self.position)
            .field("uv", &self.uv)
            .field("normal", &self.normal)
            .field("tangent", &self.tangent.into_inner())
            .field("bone_indices", &self.bone_indices)
            .field("bone_weights", &self.bone_weights)
            .finish()
    }
}

impl SkinnedVertex3D {
    /// Creates a new skinned vertex from individual attributes.
    pub fn new(
        position: Vec3,
        tex_coord: Vec2,
        normal: Vec3,
        tangent: Vec4,
        bone_indices: [u16; 4],
        bone_weights: [f32; 4],
    ) -> Self {
        SkinnedVertex3D {
            position,
            uv: tex_coord,
            normal,
            tangent: Unalign::new(tangent),
            bone_indices,
            bone_weights,
        }
    }

    pub const fn basic(position: Vec3, uv: Vec2, normal: Vec3) -> Self {
        SkinnedVertex3D {
            position,
            uv,
            normal,
            tangent: Unalign::new(Vec4::X),
            bone_indices: [0; 4],
            bone_weights: [0.0; 4],
        }
    }

    pub const fn position_only(position: Vec3) -> Self {
        SkinnedVertex3D {
            position,
            uv: Vec2::ZERO,
            normal: Vec3::Y,
            tangent: Unalign::new(Vec4::X),
            bone_indices: [0; 4],
            bone_weights: [0.0; 4],
        }
    }
}

impl Vertex for SkinnedVertex3D {
    #[inline]
    fn position(&self) -> Vec3 {
        self.position
    }

    #[inline]
    fn uv(&self) -> Vec2 {
        self.uv
    }
}

impl Vertex3D for SkinnedVertex3D {
    #[inline]
    fn normal(&self) -> Vec3 {
        self.normal
    }

    #[inline]
    fn tangent(&self) -> Vec4 {
        self.tangent.into_inner()
    }
}

/// Pads a slice to four elements using the provided default value.
fn pad_to_four<T: Copy>(input: &[T], default: T) -> [T; 4] {
    let mut arr = [default; 4];
    let count = input.len().min(4);
    arr[..count].copy_from_slice(&input[..count]);
    arr
}

pub type SkinnedVertex3DTuple<'a, IU, IF> = (Vec3, Vec2, Vec3, Vec4, IU, IF);

impl<'a, IU: AsRef<[u16]>, IF: AsRef<[f32]>> From<SkinnedVertex3DTuple<'a, IU, IF>>
    for SkinnedVertex3D
{
    fn from(value: SkinnedVertex3DTuple<IU, IF>) -> Self {
        SkinnedVertex3D::new(
            value.0,
            value.1,
            value.2,
            value.3,
            pad_to_four(value.4.as_ref(), 0),
            pad_to_four(value.5.as_ref(), 0.0),
        )
    }
}
