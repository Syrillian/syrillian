use crate::GltfScene;
use gltf::{Node, mesh};
use mikktspace::{Geometry, generate_tangents};
use std::collections::HashMap;
use std::sync::Arc;
use syrillian::assets::Mesh;
use syrillian::core::Bones;
use syrillian::math::{Vec2, Vec3, Vec4, vec2, vec4};
use syrillian::tracing::warn;
use syrillian::utils::iter::interpolate_zeros;
use syrillian_asset::SkinnedMesh;
use syrillian_asset::mesh::PartialMesh;
use syrillian_asset::mesh::static_mesh_data::{RawSkinningVertexBuffers, RawVertexBuffers};
use syrillian_utils::BoundingSphere;

/// Mesh and associated material indices for each sub-mesh range
pub type MeshData = Option<(MeshLoadResult, Vec<u32>)>;

/// Outcome of attempting to read a primitive
pub enum PrimitiveOutcome {
    Ready(PrimitiveResult),
    Skip,
}

pub enum MeshLoadResult {
    Skinned(Box<SkinnedMesh>),
    Unskinned(Mesh),
}

#[derive(Clone)]
pub struct SkinAttributes {
    indices: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
}

#[derive(Default)]
pub struct PrimitiveBuffers {
    positions: Vec<Vec3>,
    uvs: Vec<Vec2>,
    normals: Vec<Vec3>,
    tangents: Vec<Vec4>,
    bone_indices: Vec<[u16; 4]>,
    bone_weights: Vec<[f32; 4]>,
    indices: Option<Vec<u32>>,
    ranges: Vec<std::ops::Range<u32>>,
    materials: Vec<u32>,
}

pub struct VertexSources<'a> {
    pub positions: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec4>,
    pub skin: Option<SkinAttributes>,
    pub indices: Option<Vec<u32>>,
    pub joint_map: &'a HashMap<usize, usize>,
}

/// Normalizes texture coordinates from the glTF accessor format
pub fn convert_tex_coords(iter: mesh::util::ReadTexCoords<'_>) -> Vec<Vec2> {
    match iter {
        mesh::util::ReadTexCoords::F32(it) => it.map(Vec2::from_array).collect::<Vec<_>>(),
        mesh::util::ReadTexCoords::U16(it) => it
            .map(|v| vec2(v[0] as f32 / 65535.0, v[1] as f32 / 65535.0))
            .collect(),
        mesh::util::ReadTexCoords::U8(it) => it
            .map(|v| vec2(v[0] as f32 / 255.0, v[1] as f32 / 255.0))
            .collect(),
    }
}

/// Reads joint indices and weights for skinning data if available
pub fn read_skin_attributes(
    joints: Option<mesh::util::ReadJoints<'_>>,
    weights: Option<mesh::util::ReadWeights<'_>>,
) -> Option<SkinAttributes> {
    match (joints, weights) {
        (Some(joints), Some(weights)) => {
            let joints = match joints {
                mesh::util::ReadJoints::U8(it) => it
                    .map(|j| [j[0] as u16, j[1] as u16, j[2] as u16, j[3] as u16])
                    .collect(),
                mesh::util::ReadJoints::U16(it) => it.collect(),
            };
            let weights = match weights {
                mesh::util::ReadWeights::F32(it) => it.collect(),
                mesh::util::ReadWeights::U16(it) => it
                    .map(|w| {
                        [
                            w[0] as f32 / 65535.0,
                            w[1] as f32 / 65535.0,
                            w[2] as f32 / 65535.0,
                            w[3] as f32 / 65535.0,
                        ]
                    })
                    .collect(),
                mesh::util::ReadWeights::U8(it) => it
                    .map(|w| {
                        [
                            w[0] as f32 / 255.0,
                            w[1] as f32 / 255.0,
                            w[2] as f32 / 255.0,
                            w[3] as f32 / 255.0,
                        ]
                    })
                    .collect(),
            };
            Some(SkinAttributes {
                indices: joints,
                weights,
            })
        }
        _ => None,
    }
}

impl PrimitiveBuffers {
    fn total_point_count(&self) -> u32 {
        self.indices
            .as_ref()
            .map_or(self.positions.len() as u32, |indices| indices.len() as u32)
    }

    /// Extends the buffers with data from a single primitive and records its range
    pub fn extend(&mut self, data: PrimitiveResult) {
        let PrimitiveResult {
            positions,
            uvs,
            normals,
            tangents,
            bone_indices,
            bone_weights,
            indices,
            material_index,
        } = data;

        let vertex_offset = self.positions.len() as u32;
        let vertex_count = positions.len() as u32;
        let primitive_point_count = indices
            .as_ref()
            .map_or(vertex_count, |indices| indices.len() as u32);
        let range_start = self.total_point_count();
        let new_vertices_len = self.positions.len() + positions.len();
        self.positions.extend(positions);
        self.uvs.extend(uvs);
        self.normals.extend(normals);
        self.tangents.extend(tangents);

        if let Some(bone_indices) = bone_indices {
            self.bone_indices.extend(bone_indices);
        } else if !self.bone_indices.is_empty() {
            self.bone_indices.resize(new_vertices_len, [0; 4]);
        }

        if let Some(bone_weights) = bone_weights {
            self.bone_weights.extend(bone_weights);
        }
        self.bone_weights.resize(new_vertices_len, [0.0; 4]);

        match indices {
            Some(indices) => {
                let all_indices = self
                    .indices
                    .get_or_insert_with(|| (0..vertex_offset).collect());
                all_indices.extend(indices.into_iter().map(|i| i + vertex_offset));
            }
            None => {
                if let Some(all_indices) = self.indices.as_mut() {
                    all_indices.extend(vertex_offset..vertex_offset + vertex_count);
                }
            }
        }

        let range_end = range_start + primitive_point_count;
        self.ranges.push(range_start..range_end);
        self.materials.push(material_index);
    }

    /// Returns true when no vertex data has been collected yet
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Fills missing attribute channels with zeros where necessary
    pub fn fill_missing(&mut self) {
        interpolate_zeros(
            self.positions.len(),
            &mut [&mut self.uvs, &mut self.normals, &mut self.tangents],
        );
    }

    /// Builds the final mesh along with its material indices from the collected data
    pub fn build_skinned_mesh(self, bones: Bones) -> (SkinnedMesh, Vec<u32>) {
        let PrimitiveBuffers {
            positions,
            uvs,
            normals,
            tangents,
            bone_indices,
            bone_weights,
            indices,
            ranges,
            materials,
        } = self;

        let buffers = RawVertexBuffers {
            positions,
            uvs,
            normals,
            tangents,
            indices,
        };

        let skinning_buffers = RawSkinningVertexBuffers {
            bone_indices,
            bone_weights,
        };

        debug_assert!(buffers.is_valid());

        let mesh = SkinnedMesh::builder()
            .data(Arc::new(buffers))
            .skinning_data(Arc::new(skinning_buffers))
            .material_ranges(ranges)
            .bones(bones)
            .build();
        (mesh, materials)
    }

    pub fn build_mesh(self) -> (Mesh, Vec<u32>) {
        let PrimitiveBuffers {
            positions,
            uvs,
            normals,
            tangents,
            ranges,
            indices,
            bone_indices: _,
            bone_weights: _,
            materials,
        } = self;

        let buffers = RawVertexBuffers {
            positions,
            uvs,
            normals,
            tangents,
            indices,
        };
        let bounding_sphere = BoundingSphere::from_positions(buffers.positions.iter().copied());

        debug_assert!(buffers.is_valid());

        let mesh = Mesh::builder()
            .data(Arc::new(buffers))
            .material_ranges(ranges)
            .bounding_sphere(bounding_sphere)
            .build();
        (mesh, materials)
    }
}

pub struct PrimitiveResult {
    positions: Vec<Vec3>,
    uvs: Vec<Vec2>,
    normals: Vec<Vec3>,
    tangents: Vec<Vec4>,
    bone_indices: Option<Vec<[u16; 4]>>,
    bone_weights: Option<Vec<[f32; 4]>>,
    indices: Option<Vec<u32>>,
    material_index: u32,
}

impl PrimitiveResult {
    /// Creates an empty primitive result for the given material slot.
    pub fn new(material_index: u32, mut sources: VertexSources) -> Self {
        let len = sources.positions.len();
        let has_normals = !sources.normals.is_empty();
        let has_tangents = !sources.tangents.is_empty();

        sources.uvs.resize(len, Vec2::ZERO);
        sources.normals.resize(len, Vec3::Y);
        sources.tangents.resize(len, vec4(1.0, 0.0, 0.0, 1.0));

        if !has_tangents && has_normals {
            warn!(
                "Model has no tangents. Tangents will be generated. You might experience artifacts"
            );
            if generate_tangents(&mut sources) {
                warn!("Tangents couldn't be generated. The loaded primitive might look odd");
            }
        }

        let mut bone_indices = None;
        let mut bone_weights = None;
        if let Some(skin) = &mut sources.skin
            && !skin.indices.is_empty()
            && !skin.weights.is_empty()
        {
            skin.indices.resize(len, [0; 4]);
            skin.weights.resize(len, [0.0; 4]);

            for indices in skin.indices.iter_mut() {
                *indices = map_joint_indices(*indices, sources.joint_map);
            }
            for weights in skin.weights.iter_mut() {
                *weights = normalize_weights(*weights);
            }

            bone_indices = Some(skin.indices.clone());
            bone_weights = Some(skin.weights.clone());
        }

        Self {
            positions: sources.positions,
            uvs: sources.uvs,
            normals: sources.normals,
            tangents: sources.tangents,
            bone_indices,
            bone_weights,
            indices: sources.indices,
            material_index,
        }
    }

    /// Returns the number of vertices collected so far.
    pub fn vertex_count(&self) -> u32 {
        self.positions.len() as u32
    }
}

impl GltfScene {
    /// Converts a single glTF primitive into vertex data ready for assembly
    pub fn extract_primitive_mesh_data(
        &self,
        prim: gltf::Primitive,
        joint_node_index_of: &HashMap<usize, usize>,
    ) -> Option<PrimitiveOutcome> {
        if prim.mode() != mesh::Mode::Triangles {
            warn!("Non-triangle primitive encountered; skipping.");
            return Some(PrimitiveOutcome::Skip);
        }

        let reader = prim.reader(|b| Some(&self.buffers[b.index()].0));
        let positions = reader
            .read_positions()?
            .map(Vec3::from_array)
            .collect::<Vec<_>>();
        let normals = reader
            .read_normals()
            .map(|it| it.map(Vec3::from_array).collect::<Vec<_>>())
            .unwrap_or_default();
        let tangents = reader
            .read_tangents()
            .map(|it| it.map(Vec4::from_array).collect::<Vec<_>>())
            .unwrap_or_default();
        let uvs = reader
            .read_tex_coords(0)
            .map(convert_tex_coords)
            .unwrap_or_default();
        let joints_raw = reader.read_joints(0);
        let weights_raw = reader.read_weights(0);
        let indices = reader
            .read_indices()
            .map(|indices| indices.into_u32().collect::<Vec<_>>());

        let skin = read_skin_attributes(joints_raw, weights_raw);
        let sources = VertexSources {
            positions,
            normals,
            uvs,
            tangents,
            skin,
            indices,
            joint_map: joint_node_index_of,
        };

        let material_index = prim
            .material()
            .index()
            .map(|i| i as u32)
            .unwrap_or(u32::MAX);

        let result = PrimitiveResult::new(material_index, sources);

        Some(PrimitiveOutcome::Ready(result))
    }

    /// Loads the first mesh found in the scene graph
    pub(super) fn load_first_mesh_from_scene(&self) -> Option<(MeshLoadResult, Vec<u32>)> {
        let doc = &self.doc;
        let scene0 = doc.default_scene().or_else(|| doc.scenes().next())?;
        for node in scene0.nodes() {
            if let Some(mesh) = self.load_first_mesh_from_node(node) {
                return Some(mesh);
            }
        }
        None
    }

    /// Loads a mesh attached to the given node if one exists
    pub(super) fn load_mesh(&self, node: Node) -> Option<(MeshLoadResult, Vec<u32>)> {
        let gltf_mesh = node.mesh()?;
        let mut node_bones = None;
        let mut joint_node_index_of = HashMap::new();

        if let Some(skin) = node.skin() {
            let bones = self.build_bones_from_skin(skin, node.clone(), &mut joint_node_index_of);
            node_bones = Some(bones);
        }

        let mut buffers = self.read_mesh_primitives(gltf_mesh, &joint_node_index_of)?;

        if buffers.is_empty() {
            return None;
        }

        buffers.fill_missing();
        match node_bones {
            None => {
                let res = buffers.build_mesh();
                Some((MeshLoadResult::Unskinned(res.0), res.1))
            }
            Some(bones) => {
                let res = buffers.build_skinned_mesh(bones);
                Some((MeshLoadResult::Skinned(Box::new(res.0)), res.1))
            }
        }
    }

    /// Searches the node hierarchy recursively for the first available mesh
    fn load_first_mesh_from_node(&self, node: Node) -> Option<(MeshLoadResult, Vec<u32>)> {
        if let Some(mesh) = self.load_mesh(node.clone()) {
            return Some(mesh);
        }

        for child in node.children() {
            if let Some(mesh) = self.load_first_mesh_from_node(child) {
                return Some(mesh);
            }
        }
        None
    }

    /// Reads all primitives from a glTF mesh into intermediate buffers
    fn read_mesh_primitives(
        &self,
        mesh: gltf::Mesh,
        joint_node_index_of: &HashMap<usize, usize>,
    ) -> Option<PrimitiveBuffers> {
        let mut buffers = PrimitiveBuffers::default();

        for prim in mesh.primitives() {
            match self.extract_primitive_mesh_data(prim, joint_node_index_of) {
                Some(PrimitiveOutcome::Ready(result)) => buffers.extend(result),
                Some(PrimitiveOutcome::Skip) => continue,
                None => return None,
            }
        }

        Some(buffers)
    }
}

impl Geometry for VertexSources<'_> {
    fn num_faces(&self) -> usize {
        self.point_count() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        let index = self.vertex_index(face, vert);
        self.positions[index].to_array()
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        let index = self.vertex_index(face, vert);
        self.normals[index].to_array()
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        let index = self.vertex_index(face, vert);
        self.uvs[index].to_array()
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        let index = self.vertex_index(face, vert);
        self.tangents[index] = Vec4::from_array(tangent);
    }
}

impl VertexSources<'_> {
    fn point_count(&self) -> usize {
        self.indices
            .as_ref()
            .map_or(self.positions.len(), |indices| indices.len())
    }

    fn vertex_index(&self, face: usize, vert: usize) -> usize {
        let point_index = face * 3 + vert;
        self.indices
            .as_ref()
            .map_or(point_index, |indices| indices[point_index] as usize)
    }
}

impl MeshLoadResult {
    pub fn vertex_count(&self) -> usize {
        match self {
            MeshLoadResult::Unskinned(mesh) => mesh.position_count(),
            MeshLoadResult::Skinned(mesh) => mesh.position_count(),
        }
    }

    pub fn is_unskinned(&self) -> bool {
        matches!(self, MeshLoadResult::Unskinned(_))
    }

    pub fn is_skinned(&self) -> bool {
        matches!(self, MeshLoadResult::Skinned(_))
    }
}

/// Maps glTF joint indices to the corresponding engine bone indices
#[inline]
pub fn map_joint_indices(
    joints: [u16; 4],
    joint_node_index_of: &HashMap<usize, usize>,
) -> [u16; 4] {
    joints.map(|j| joint_node_index_of.get(&(j as usize)).copied().unwrap_or(0) as u16)
}

/// Normalizes the four bone weights associated with a vertex
#[inline]
///
pub fn normalize_weights(weights: [f32; 4]) -> [f32; 4] {
    let sum = weights.iter().copied().sum::<f32>().max(1e-8);
    weights.map(|w| w / sum)
}
