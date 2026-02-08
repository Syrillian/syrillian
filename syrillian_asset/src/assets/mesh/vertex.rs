use glamx::{Vec2, Vec3};
use static_assertions::const_assert_eq;
use syrillian_utils::sizes::{VEC2_SIZE, VEC3_SIZE, VEC4_SIZE, vertex_layout_size};
use wgpu::{BufferAddress, VertexAttribute, VertexFormat};

/// Convenience vertex used when constructing static meshes.
#[derive(Copy, Clone)]
pub struct SimpleVertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl SimpleVertex3D {
    /// Converts this simplified vertex into a full [`Vertex3D`].
    /// This is not recommended as the tangent and bitangent calculation is just a rought approximation.
    pub const fn upgrade(self) -> Vertex3D {
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

        Vertex3D {
            position,
            uv,
            normal,
            tangent,
            bone_indices: [0xFF, 0xFF, 0xFF, 0xFF],
            bone_weights: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// A fully featured vertex used for 3D rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3D {
    pub position: Vec3,
    pub uv: Vec2,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub bone_indices: [u32; 4],
    pub bone_weights: [f32; 4],
}

impl Vertex3D {
    /// Creates a new vertex from individual attributes.
    pub fn new(
        position: Vec3,
        tex_coord: Vec2,
        normal: Vec3,
        tangent: Vec3,
        bone_indices: &[u32],
        bone_weights: &[f32],
    ) -> Self {
        Vertex3D {
            position,
            uv: tex_coord,
            normal,
            tangent,
            bone_indices: pad_to_four(bone_indices, 0x0),
            bone_weights: pad_to_four(bone_weights, 0.0),
        }
    }

    /// Returns a [`wgpu::VertexBufferLayout`] describing the layout of this vertex.
    pub const fn continuous_descriptor<'a>() -> wgpu::VertexBufferLayout<'a> {
        const LAYOUT: wgpu::VertexBufferLayout = wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex3D>() as BufferAddress,
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
                VertexAttribute {
                    format: VertexFormat::Uint32x4,
                    offset: (VEC3_SIZE * 3 + VEC2_SIZE) as BufferAddress,
                    shader_location: 4,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: (VEC4_SIZE + VEC3_SIZE * 3 + VEC2_SIZE) as BufferAddress,
                    shader_location: 5,
                },
            ],
        };

        const_assert_eq!(size_of::<Vertex3D>(), vertex_layout_size(&LAYOUT));

        LAYOUT
    }

    pub const fn basic(position: Vec3, uv: Vec2, normal: Vec3) -> Self {
        Vertex3D {
            position,
            uv,
            normal,
            tangent: Vec3::X,
            bone_indices: [0; 4],
            bone_weights: [0.0; 4],
        }
    }

    pub const fn position_only(position: Vec3) -> Self {
        Vertex3D {
            position,
            uv: Vec2::ZERO,
            normal: Vec3::Y,
            tangent: Vec3::X,
            bone_indices: [0; 4],
            bone_weights: [0.0; 4],
        }
    }
}

pub type Vertex3DTuple<'a, IU, IF> = (Vec3, Vec2, Vec3, Vec3, Vec3, IU, IF);

impl<'a, IU: AsRef<[u32]>, IF: AsRef<[f32]>> From<Vertex3DTuple<'a, IU, IF>> for Vertex3D {
    fn from(value: Vertex3DTuple<IU, IF>) -> Self {
        Vertex3D::new(
            value.0,
            value.1,
            value.2,
            value.3,
            value.5.as_ref(),
            value.6.as_ref(),
        )
    }
}

/// Pads a slice to four elements using the provided default value.
fn pad_to_four<T: Copy>(input: &[T], default: T) -> [T; 4] {
    let mut arr = [default; 4];
    let count = input.len().min(4);
    arr[..count].copy_from_slice(&input[..count]);
    arr
}
