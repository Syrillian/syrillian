use crate::mesh::generic_vertex::{Vertex, Vertex3D};
use glam::{Vec2, Vec3};
use static_assertions::const_assert_eq;
use syrillian_utils::sizes::{VEC2_SIZE, VEC3_SIZE, vertex_layout_size};
use wgpu::{BufferAddress, VertexAttribute, VertexFormat};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// An unskinned vertex used for 3D rendering.
#[repr(C)]
#[derive(
    Copy,
    Clone,
    Debug,
    bytemuck::Pod,
    bytemuck::Zeroable,
    Immutable,
    IntoBytes,
    FromBytes,
    KnownLayout,
)]
pub struct UnskinnedVertex3D {
    pub position: Vec3,
    pub uv: Vec2,
    pub normal: Vec3,
    pub tangent: Vec3,
}

impl UnskinnedVertex3D {
    /// Creates a new unskinned vertex from individual attributes.
    pub fn new(position: Vec3, tex_coord: Vec2, normal: Vec3, tangent: Vec3) -> Self {
        UnskinnedVertex3D {
            position,
            uv: tex_coord,
            normal,
            tangent,
        }
    }

    /// Returns a [`wgpu::VertexBufferLayout`] describing the layout of this vertex.
    pub const fn continuous_descriptor<'a>() -> wgpu::VertexBufferLayout<'a> {
        const LAYOUT: wgpu::VertexBufferLayout = wgpu::VertexBufferLayout {
            array_stride: size_of::<UnskinnedVertex3D>() as BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: VEC3_SIZE,
                    shader_location: 1,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: (VEC3_SIZE + VEC2_SIZE) as BufferAddress,
                    shader_location: 2,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: (VEC3_SIZE * 2 + VEC2_SIZE) as BufferAddress,
                    shader_location: 3,
                },
            ],
        };

        const_assert_eq!(size_of::<UnskinnedVertex3D>(), vertex_layout_size(&LAYOUT));

        LAYOUT
    }

    pub const fn basic(position: Vec3, uv: Vec2, normal: Vec3) -> Self {
        UnskinnedVertex3D {
            position,
            uv,
            normal,
            tangent: Vec3::X,
        }
    }

    pub const fn position_only(position: Vec3) -> Self {
        UnskinnedVertex3D {
            position,
            uv: Vec2::ZERO,
            normal: Vec3::Y,
            tangent: Vec3::X,
        }
    }
}

impl Vertex for UnskinnedVertex3D {
    #[inline]
    fn position(&self) -> Vec3 {
        self.position
    }

    #[inline]
    fn uv(&self) -> Vec2 {
        self.uv
    }
}

impl Vertex3D for UnskinnedVertex3D {
    #[inline]
    fn normal(&self) -> Vec3 {
        self.normal
    }

    #[inline]
    fn tangent(&self) -> Vec3 {
        self.tangent
    }
}

pub type UnskinnedVertex3DTuple = (Vec3, Vec2, Vec3, Vec3, Vec3);

impl From<UnskinnedVertex3DTuple> for UnskinnedVertex3D {
    fn from(value: UnskinnedVertex3DTuple) -> Self {
        UnskinnedVertex3D::new(value.0, value.1, value.2, value.3)
    }
}
