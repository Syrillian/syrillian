use crate::mesh::generic_vertex::Vertex3D;
use crate::mesh::{SkinnedVertex3D, UnskinnedVertex3D};
use glamx::Vec3;

pub type SkinnedStaticMeshData = StaticMeshData<SkinnedVertex3D>;

#[derive(Debug, Clone)]
pub struct StaticMeshData<V: Vertex3D = UnskinnedVertex3D> {
    pub vertices: Vec<V>,
    pub indices: Option<Vec<u32>>,
}

impl<V: Vertex3D> StaticMeshData<V> {
    pub fn new(vertices: Vec<V>, indices: Option<Vec<u32>>) -> Self {
        StaticMeshData { vertices, indices }
    }

    pub fn make_triangle_indices(&self) -> Vec<[u32; 3]> {
        match &self.indices {
            None => (0u32..self.vertices.len() as u32)
                .collect::<Vec<_>>()
                .as_chunks()
                .0
                .to_vec(),
            Some(indices) => indices.as_chunks().0.to_vec(),
        }
    }

    pub fn make_point_cloud(&self) -> Vec<Vec3> {
        self.vertices.iter().map(|v| v.position()).collect()
    }
}
