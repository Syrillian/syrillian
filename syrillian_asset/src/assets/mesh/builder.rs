use super::{Bones, Mesh, Vertex3D, bounding_sphere_from_vertices};
use crate::mesh::MeshVertexData;
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MeshBuilder {
    vertices: Vec<Vertex3D>,
    indices: Option<Vec<u32>>,
    material_ranges: Option<Vec<Range<u32>>>,
    bones: Option<Bones>,
}

impl MeshBuilder {
    pub fn new(vertices: Vec<Vertex3D>) -> Self {
        MeshBuilder {
            vertices,
            indices: None,
            material_ranges: None,
            bones: None,
        }
    }

    pub fn build(self) -> Mesh {
        let mut material_ranges = self.material_ranges.unwrap_or_default();

        if material_ranges.is_empty() {
            let vert_count = self
                .indices
                .as_ref()
                .map_or_else(|| self.vertices.len(), |indices| indices.len());

            material_ranges.push(0u32..vert_count as u32);
        }

        let bounding_sphere = bounding_sphere_from_vertices(&self.vertices);

        Mesh {
            data: Arc::new(MeshVertexData::new(self.vertices, self.indices)),
            material_ranges,
            bones: self.bones.unwrap_or_default(),
            bounding_sphere,
        }
    }

    pub fn with_bones(mut self, bones: Bones) -> Self {
        self.bones = Some(bones);
        self
    }

    pub fn with_many_textures(mut self, materials: Vec<Range<u32>>) -> Self {
        self.material_ranges = Some(materials);
        self
    }

    pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
        self.indices = Some(indices);
        self
    }
}

impl From<MeshBuilder> for Mesh {
    fn from(value: MeshBuilder) -> Self {
        value.build()
    }
}
