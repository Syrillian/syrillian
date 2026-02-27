use crate::mesh::generic_vertex::Vertex3D;
use glam::{Vec2, Vec4};
use glamx::Vec3;
use itertools::Itertools;

#[derive(Debug, Default)]
pub struct RawVertexBuffers {
    pub positions: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec4>,
    pub indices: Option<Vec<u32>>,
}

#[derive(Debug, Default)]
pub struct RawSkinningVertexBuffers {
    pub bone_indices: Vec<[u16; 4]>,
    pub bone_weights: Vec<[f32; 4]>,
}

pub trait VertexBufferExt {
    fn positions(&self) -> &[Vec3];
    fn indices(&self) -> Option<&[u32]>;

    fn len(&self) -> usize {
        self.indices()
            .map_or_else(|| self.positions().len(), <[u32]>::len)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn make_triangle_indices(&self) -> Vec<[u32; 3]> {
        match &self.indices() {
            None => (0u32..self.positions().len() as u32)
                .collect_vec()
                .as_chunks()
                .0
                .to_vec(),
            Some(indices) => indices.as_chunks().0.to_vec(),
        }
    }

    fn interpolate(&mut self);

    fn from_positions(positions: Vec<Vec3>, indices: Option<Vec<u32>>) -> Self;
}

impl RawVertexBuffers {
    pub fn is_valid(&self) -> bool {
        if self.is_empty() {
            return false;
        }

        let p_count = self.positions().len();

        p_count == self.uvs.len() && p_count == self.normals.len() && p_count == self.tangents.len()
    }
}

impl VertexBufferExt for RawVertexBuffers {
    fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    fn indices(&self) -> Option<&[u32]> {
        self.indices.as_deref()
    }

    fn interpolate(&mut self) {
        let len = self.positions.len();
        self.uvs.resize(len, Vec2::ZERO);
        self.normals.resize(len, Vec3::ZERO);
        self.tangents.resize(len, Vec4::ZERO);
    }

    fn from_positions(positions: Vec<Vec3>, indices: Option<Vec<u32>>) -> Self {
        let len = positions.len();
        Self {
            positions,
            uvs: vec![Vec2::ZERO; len],
            normals: vec![Vec3::ZERO; len],
            tangents: vec![Vec4::ZERO; len],
            indices,
        }
    }
}

impl<V: Vertex3D> From<&[V]> for RawVertexBuffers {
    fn from(vertices: &[V]) -> Self {
        let mut buffers = Self::default();

        for v in vertices {
            buffers.positions.push(v.position());
            buffers.uvs.push(v.uv());
            buffers.normals.push(v.normal());
            buffers.tangents.push(v.tangent());
        }

        buffers
    }
}
