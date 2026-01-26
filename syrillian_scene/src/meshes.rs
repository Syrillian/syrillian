use super::bones::build_bones_from_skin;
use crate::gltf_loader::GltfScene;
use gltf::mesh;
use gltf::{self, Node};
use itertools::izip;
use std::collections::HashMap;
use syrillian::assets::Mesh;
use syrillian::core::{Bones, Vertex3D};
use syrillian::math::{Vector2, Vector3};
use syrillian::tracing::warn;

/// Mesh and associated material indices for each sub-mesh range.
pub type MeshData = Option<(Mesh, Vec<u32>)>;

type SkinAttributes = (Vec<[u16; 4]>, Vec<[f32; 4]>);

#[derive(Copy, Clone)]
struct SkinSlices<'a> {
    joints: &'a [[u16; 4]],
    weights: &'a [[f32; 4]],
}

impl<'a> From<&'a SkinAttributes> for SkinSlices<'a> {
    fn from(value: &'a SkinAttributes) -> Self {
        Self {
            joints: value.0.as_slice(),
            weights: value.1.as_slice(),
        }
    }
}

struct VertexSources<'a> {
    positions: &'a [[f32; 3]],
    normals: Option<&'a Vec<[f32; 3]>>,
    tangents: Option<&'a Vec<[f32; 4]>>,
    tex_coords: Option<&'a Vec<[f32; 2]>>,
    skin: Option<SkinSlices<'a>>,
    joint_map: &'a HashMap<usize, usize>,
}

/// Loads the first mesh found in the scene graph.
pub(super) fn load_first_from_scene(scene: &GltfScene) -> Option<(Mesh, Vec<u32>)> {
    let doc = &scene.doc;
    let scene0 = doc.default_scene().or_else(|| doc.scenes().next())?;
    for node in scene0.nodes() {
        if let Some(mesh) = load_first_from_node(scene, node) {
            return Some(mesh);
        }
    }
    None
}

/// Loads a mesh attached to the given node if one exists.
pub(super) fn load_mesh(scene: &GltfScene, node: Node) -> Option<(Mesh, Vec<u32>)> {
    let gltf_mesh = node.mesh()?;
    let mut bones = Bones::default();
    let mut joint_node_index_of = HashMap::new();

    if let Some(skin) = node.skin() {
        build_bones_from_skin(
            scene,
            skin,
            node.clone(),
            &mut bones,
            &mut joint_node_index_of,
        );
    }

    let mut buffers = read_mesh_primitives(scene, gltf_mesh, &joint_node_index_of)?;

    if buffers.is_empty() {
        return None;
    }

    buffers.fill_missing();
    Some(buffers.build_mesh(bones))
}

/// Searches the node hierarchy recursively for the first available mesh.
fn load_first_from_node(scene: &GltfScene, node: Node) -> Option<(Mesh, Vec<u32>)> {
    if let Some(mesh) = load_mesh(scene, node.clone()) {
        return Some(mesh);
    }
    for child in node.children() {
        if let Some(mesh) = load_first_from_node(scene, child) {
            return Some(mesh);
        }
    }
    None
}

/// Reads all primitives from a glTF mesh into intermediate buffers.
fn read_mesh_primitives(
    scene: &GltfScene,
    mesh: gltf::Mesh,
    joint_node_index_of: &HashMap<usize, usize>,
) -> Option<PrimitiveBuffers> {
    let mut buffers = PrimitiveBuffers::default();
    let mut start_vertex = 0u32;

    for prim in mesh.primitives() {
        match extract_primitive_data(scene, prim, joint_node_index_of) {
            Some(PrimitiveOutcome::Ready(result)) => {
                let count = result.vertex_count();
                buffers.extend(result, start_vertex);
                start_vertex += count;
            }
            Some(PrimitiveOutcome::Skip) => continue,
            None => return None,
        }
    }

    Some(buffers)
}

/// Outcome of attempting to read a primitive.
enum PrimitiveOutcome {
    Ready(PrimitiveResult),
    Skip,
}

/// Converts a single glTF primitive into vertex data ready for assembly.
fn extract_primitive_data(
    scene: &GltfScene,
    prim: gltf::Primitive,
    joint_node_index_of: &HashMap<usize, usize>,
) -> Option<PrimitiveOutcome> {
    if prim.mode() != mesh::Mode::Triangles {
        warn!("Non-triangle primitive encountered; skipping.");
        return Some(PrimitiveOutcome::Skip);
    }

    let reader = prim.reader(|b| Some(&scene.buffers[b.index()].0));
    let positions = reader.read_positions()?.collect::<Vec<_>>();
    let normals = reader.read_normals().map(|it| it.collect::<Vec<_>>());
    let tangents = reader.read_tangents().map(|it| it.collect::<Vec<_>>());
    let tex_coords = reader.read_tex_coords(0).map(convert_tex_coords);
    let joints_raw = reader.read_joints(0);
    let weights_raw = reader.read_weights(0);
    let indices: Vec<u32> = if let Some(ind) = reader.read_indices() {
        ind.into_u32().collect()
    } else {
        (0u32..positions.len() as u32).collect()
    };

    let skin_attributes = read_skin_attributes(joints_raw, weights_raw);
    let skin_slices = skin_attributes.as_ref().map(SkinSlices::from);
    let sources = VertexSources {
        positions: &positions,
        normals: normals.as_ref(),
        tangents: tangents.as_ref(),
        tex_coords: tex_coords.as_ref(),
        skin: skin_slices,
        joint_map: joint_node_index_of,
    };

    let material_index = prim.material().index().map(|i| i as u32).unwrap_or(0);

    let mut result = PrimitiveResult::new(material_index);
    for chunk in indices.chunks_exact(3) {
        for &index in chunk {
            result.push_vertex(index as usize, &sources);
        }
    }

    Some(PrimitiveOutcome::Ready(result))
}

/// Normalizes texture coordinates from the glTF accessor format.
fn convert_tex_coords(iter: mesh::util::ReadTexCoords<'_>) -> Vec<[f32; 2]> {
    match iter {
        mesh::util::ReadTexCoords::F32(it) => it.collect::<Vec<_>>(),
        mesh::util::ReadTexCoords::U16(it) => it
            .map(|v| [v[0] as f32 / 65535.0, v[1] as f32 / 65535.0])
            .collect(),
        mesh::util::ReadTexCoords::U8(it) => it
            .map(|v| [v[0] as f32 / 255.0, v[1] as f32 / 255.0])
            .collect(),
    }
}

/// Reads joint indices and weights for skinning data if available.
fn read_skin_attributes(
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
            Some((joints, weights))
        }
        _ => None,
    }
}

/// Maps glTF joint indices to the corresponding engine bone indices.
fn map_joint_indices(joints: &[u16; 4], joint_node_index_of: &HashMap<usize, usize>) -> Vec<u32> {
    joints
        .iter()
        .map(|j| {
            joint_node_index_of
                .get(&(*j as usize))
                .copied()
                .unwrap_or(0) as u32
        })
        .collect()
}

/// Normalizes the four bone weights associated with a vertex.
fn normalize_weights(weights: [f32; 4]) -> Vec<f32> {
    let sum = weights.iter().copied().sum::<f32>().max(1e-8);
    weights.iter().map(|w| w / sum).collect()
}

/// Computes the bitangent vector for a vertex from the normal and tangent.
fn compute_bitangent(
    normal: &Vector3<f32>,
    tangent: &Vector3<f32>,
    handedness: f32,
) -> Vector3<f32> {
    let cross = normal.cross(tangent);
    match cross.try_normalize(1e-6) {
        Some(unit) => unit * handedness.signum(),
        None => Vector3::zeros(),
    }
}

#[derive(Default)]
struct PrimitiveBuffers {
    positions: Vec<Vector3<f32>>,
    tex_coords: Vec<Vector2<f32>>,
    normals: Vec<Vector3<f32>>,
    tangents: Vec<Vector3<f32>>,
    bitangents: Vec<Vector3<f32>>,
    bone_indices: Vec<Vec<u32>>,
    bone_weights: Vec<Vec<f32>>,
    ranges: Vec<std::ops::Range<u32>>,
    materials: Vec<u32>,
}

impl PrimitiveBuffers {
    /// Extends the buffers with data from a single primitive and records its range.
    fn extend(&mut self, data: PrimitiveResult, start: u32) {
        let PrimitiveResult {
            positions,
            tex_coords,
            normals,
            tangents,
            bitangents,
            bone_indices,
            bone_weights,
            material_index,
        } = data;

        let vertex_count = positions.len() as u32;
        self.positions.extend(positions);
        self.tex_coords.extend(tex_coords);
        self.normals.extend(normals);
        self.tangents.extend(tangents);
        self.bitangents.extend(bitangents);
        self.bone_indices.extend(bone_indices);
        self.bone_weights.extend(bone_weights);

        let end = start + vertex_count;
        self.ranges.push(start..end);
        self.materials.push(material_index);
    }

    /// Returns true when no vertex data has been collected yet.
    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Fills missing attribute channels with zeros where necessary.
    fn fill_missing(&mut self) {
        syrillian::utils::iter::interpolate_zeros(
            self.positions.len(),
            &mut [
                &mut self.tex_coords,
                &mut self.normals,
                &mut self.tangents,
                &mut self.bitangents,
            ],
        );
    }

    /// Builds the final mesh along with its material indices from the collected data.
    fn build_mesh(self, bones: Bones) -> (Mesh, Vec<u32>) {
        let PrimitiveBuffers {
            positions,
            tex_coords,
            normals,
            tangents,
            bitangents,
            bone_indices,
            bone_weights,
            ranges,
            materials,
        } = self;

        let vertices = izip!(
            positions,
            tex_coords,
            normals,
            tangents,
            bitangents,
            bone_indices,
            bone_weights
        )
        .map(Vertex3D::from)
        .collect();

        let mesh = Mesh::builder(vertices)
            .with_many_textures(ranges)
            .with_bones(bones)
            .build();
        (mesh, materials)
    }
}

struct PrimitiveResult {
    positions: Vec<Vector3<f32>>,
    tex_coords: Vec<Vector2<f32>>,
    normals: Vec<Vector3<f32>>,
    tangents: Vec<Vector3<f32>>,
    bitangents: Vec<Vector3<f32>>,
    bone_indices: Vec<Vec<u32>>,
    bone_weights: Vec<Vec<f32>>,
    material_index: u32,
}

impl PrimitiveResult {
    /// Creates an empty primitive result for the given material slot.
    fn new(material_index: u32) -> Self {
        Self {
            positions: Vec::new(),
            tex_coords: Vec::new(),
            normals: Vec::new(),
            tangents: Vec::new(),
            bitangents: Vec::new(),
            bone_indices: Vec::new(),
            bone_weights: Vec::new(),
            material_index,
        }
    }

    /// Returns the number of vertices collected so far.
    fn vertex_count(&self) -> u32 {
        self.positions.len() as u32
    }

    /// Appends a vertex with all available attributes to the primitive result.
    fn push_vertex(&mut self, index: usize, sources: &VertexSources<'_>) {
        let pos = sources.positions[index];
        let position = Vector3::new(pos[0], pos[1], pos[2]);
        self.positions.push(position);

        let normal = sources.normals.map_or_else(Vector3::zeros, |list| {
            let n = list[index];
            Vector3::new(n[0], n[1], n[2])
        });
        self.normals.push(normal);

        let (tangent, bitangent) = sources.tangents.map_or_else(
            || (Vector3::zeros(), Vector3::zeros()),
            |list| {
                let t = list[index];
                let tangent = Vector3::new(t[0], t[1], t[2]);
                let bitangent = compute_bitangent(&normal, &tangent, t[3]);
                (tangent, bitangent)
            },
        );
        self.tangents.push(tangent);
        self.bitangents.push(bitangent);

        let uv = sources.tex_coords.map_or_else(Vector2::zeros, |list| {
            let uv = list[index];
            Vector2::new(uv[0], uv[1])
        });
        self.tex_coords.push(uv);

        if let Some(skin) = sources.skin {
            let joint = skin.joints[index];
            let weight = skin.weights[index];
            self.bone_indices
                .push(map_joint_indices(&joint, sources.joint_map));
            self.bone_weights.push(normalize_weights(weight));
        } else {
            self.bone_indices.push(Vec::new());
            self.bone_weights.push(Vec::new());
        }
    }
}
