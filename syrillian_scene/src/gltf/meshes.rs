use crate::GltfScene;
use gltf::{Node, mesh};
use itertools::izip;
use std::collections::HashMap;
use syrillian::assets::Mesh;
use syrillian::core::{Bones, Vertex3D};
use syrillian::math::{Vec2, Vec3};
use syrillian::tracing::warn;

/// Mesh and associated material indices for each sub-mesh range
pub type MeshData = Option<(Mesh, Vec<u32>)>;

pub type SkinAttributes = (Vec<[u16; 4]>, Vec<[f32; 4]>);

/// Outcome of attempting to read a primitive
pub enum PrimitiveOutcome {
    Ready(PrimitiveResult),
    Skip,
}

#[derive(Copy, Clone)]
pub struct SkinSlices<'a> {
    joints: &'a [[u16; 4]],
    weights: &'a [[f32; 4]],
}

#[derive(Default)]
pub struct PrimitiveBuffers {
    positions: Vec<Vec3>,
    tex_coords: Vec<Vec2>,
    normals: Vec<Vec3>,
    tangents: Vec<Vec3>,
    bitangents: Vec<Vec3>,
    bone_indices: Vec<Vec<u32>>,
    bone_weights: Vec<Vec<f32>>,
    ranges: Vec<std::ops::Range<u32>>,
    materials: Vec<u32>,
}

pub struct VertexSources<'a> {
    pub positions: &'a [[f32; 3]],
    pub normals: Option<&'a Vec<[f32; 3]>>,
    pub tangents: Option<&'a Vec<[f32; 4]>>,
    pub tex_coords: Option<&'a Vec<[f32; 2]>>,
    pub skin: Option<SkinSlices<'a>>,
    pub joint_map: &'a HashMap<usize, usize>,
}

/// Normalizes texture coordinates from the glTF accessor format
pub fn convert_tex_coords(iter: mesh::util::ReadTexCoords<'_>) -> Vec<[f32; 2]> {
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
            Some((joints, weights))
        }
        _ => None,
    }
}

/// Maps glTF joint indices to the corresponding engine bone indices
pub fn map_joint_indices(
    joints: &[u16; 4],
    joint_node_index_of: &HashMap<usize, usize>,
) -> Vec<u32> {
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

/// Normalizes the four bone weights associated with a vertex
pub fn normalize_weights(weights: [f32; 4]) -> Vec<f32> {
    let sum = weights.iter().copied().sum::<f32>().max(1e-8);
    weights.iter().map(|w| w / sum).collect()
}

/// Computes the bitangent vector for a vertex from the normal and tangent
pub fn compute_bitangent(normal: Vec3, tangent: Vec3, handedness: f32) -> Vec3 {
    let bitangent = normal.cross(tangent);
    if bitangent.length_squared() <= 1e-10 {
        return Vec3::ZERO;
    }

    let sign = if handedness < 0.0 { -1.0 } else { 1.0 };
    bitangent.normalize() * sign
}

impl VertexSources<'_> {
    pub fn vertex_position(&self, index: usize) -> Vec3 {
        Vec3::from(self.positions[index])
    }

    pub fn vertex_normal(&self, index: usize) -> Vec3 {
        self.normals
            .map_or(Vec3::ZERO, |list| Vec3::from(list[index]))
    }

    pub fn vertex_tex_coord(&self, index: usize) -> Vec2 {
        self.tex_coords
            .map_or(Vec2::ZERO, |list| Vec2::from(list[index]))
    }

    pub fn triangle_tangent_frame(&self, indices: [usize; 3]) -> Option<(Vec3, Vec3)> {
        let _ = self.tex_coords?;

        let p0 = self.vertex_position(indices[0]);
        let p1 = self.vertex_position(indices[1]);
        let p2 = self.vertex_position(indices[2]);

        let uv0 = self.vertex_tex_coord(indices[0]);
        let uv1 = self.vertex_tex_coord(indices[1]);
        let uv2 = self.vertex_tex_coord(indices[2]);

        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let delta_uv1 = uv1 - uv0;
        let delta_uv2 = uv2 - uv0;

        let det = delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x;
        if det.abs() <= 1e-10 {
            return None;
        }

        let inv_det = 1.0 / det;
        let tangent = (edge1 * delta_uv2.y - edge2 * delta_uv1.y) * inv_det;
        let bitangent = (edge2 * delta_uv1.x - edge1 * delta_uv2.x) * inv_det;
        Some((tangent, bitangent))
    }

    fn generated_tangent_frame(
        &self,
        index: usize,
        triangle_tangent: Vec3,
        triangle_bitangent: Vec3,
    ) -> (Vec3, Vec3) {
        let normal = self.vertex_normal(index);
        let tangent = orthonormalize_tangent(normal, triangle_tangent);

        let handedness =
            if normal.length_squared() <= 1e-10 || triangle_bitangent.length_squared() <= 1e-10 {
                1.0
            } else {
                let reference = normal.cross(tangent);
                if reference.dot(triangle_bitangent) < 0.0 {
                    -1.0
                } else {
                    1.0
                }
            };

        let bitangent = compute_bitangent(normal, tangent, handedness);
        (tangent, bitangent)
    }
}

fn fallback_tangent_for_normal(normal: Vec3) -> Vec3 {
    let n = if normal.length_squared() <= 1e-10 {
        Vec3::Y
    } else {
        normal.normalize()
    };
    let up = if n.y.abs() < 0.999 { Vec3::Y } else { Vec3::X };
    let tangent = up - n * n.dot(up);
    if tangent.length_squared() <= 1e-10 {
        Vec3::X
    } else {
        tangent.normalize()
    }
}

fn orthonormalize_tangent(normal: Vec3, tangent: Vec3) -> Vec3 {
    let candidate = if normal.length_squared() <= 1e-10 {
        tangent
    } else {
        tangent - normal * normal.dot(tangent)
    };

    if candidate.length_squared() <= 1e-10 {
        fallback_tangent_for_normal(normal)
    } else {
        candidate.normalize()
    }
}

impl PrimitiveBuffers {
    /// Extends the buffers with data from a single primitive and records its range
    pub fn extend(&mut self, data: PrimitiveResult, start: u32) {
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

    /// Returns true when no vertex data has been collected yet
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Fills missing attribute channels with zeros where necessary
    pub fn fill_missing(&mut self) {
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

    /// Builds the final mesh along with its material indices from the collected data
    pub fn build_mesh(self, bones: Bones) -> (Mesh, Vec<u32>) {
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

pub struct PrimitiveResult {
    positions: Vec<Vec3>,
    tex_coords: Vec<Vec2>,
    normals: Vec<Vec3>,
    tangents: Vec<Vec3>,
    bitangents: Vec<Vec3>,
    bone_indices: Vec<Vec<u32>>,
    bone_weights: Vec<Vec<f32>>,
    material_index: u32,
}

impl PrimitiveResult {
    /// Creates an empty primitive result for the given material slot.
    pub fn new(material_index: u32) -> Self {
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
    pub fn vertex_count(&self) -> u32 {
        self.positions.len() as u32
    }

    pub fn push_triangle_with_generated_tangents(
        &mut self,
        triangle: [usize; 3],
        sources: &VertexSources<'_>,
    ) {
        let (triangle_tangent, triangle_bitangent) =
            sources.triangle_tangent_frame(triangle).unwrap_or_else(|| {
                let normal = sources.vertex_normal(triangle[0]);
                let tangent = fallback_tangent_for_normal(normal);
                let bitangent = compute_bitangent(normal, tangent, 1.0);
                (tangent, bitangent)
            });

        for index in triangle {
            let tangent_frame =
                sources.generated_tangent_frame(index, triangle_tangent, triangle_bitangent);
            self.push_vertex_with_frame(index, sources, Some(tangent_frame));
        }
    }

    /// Appends a vertex with all available attributes to the primitive result.
    pub fn push_vertex(&mut self, index: usize, sources: &VertexSources<'_>) {
        self.push_vertex_with_frame(index, sources, None);
    }

    pub fn push_vertex_with_frame(
        &mut self,
        index: usize,
        sources: &VertexSources<'_>,
        tangent_frame: Option<(Vec3, Vec3)>,
    ) {
        let position = sources.vertex_position(index);
        self.positions.push(position);

        let normal = sources.vertex_normal(index);
        self.normals.push(normal);

        let (tangent, bitangent) = tangent_frame.unwrap_or_else(|| {
            sources.tangents.map_or_else(
                || (Vec3::ZERO, Vec3::ZERO),
                |list| {
                    let t = list[index];
                    let tangent = Vec3::new(t[0], t[1], t[2]);
                    let bitangent = compute_bitangent(normal, tangent, t[3]);
                    (tangent, bitangent)
                },
            )
        });
        self.tangents.push(tangent);
        self.bitangents.push(bitangent);

        let uv = sources.vertex_tex_coord(index);
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

        let material_index = prim
            .material()
            .index()
            .map(|i| i as u32)
            .unwrap_or(u32::MAX);

        let mut result = PrimitiveResult::new(material_index);
        for chunk in indices.chunks_exact(3) {
            let triangle = [chunk[0] as usize, chunk[1] as usize, chunk[2] as usize];
            if sources.tangents.is_some() {
                for index in triangle {
                    result.push_vertex(index, &sources);
                }
            } else {
                result.push_triangle_with_generated_tangents(triangle, &sources);
            }
        }

        Some(PrimitiveOutcome::Ready(result))
    }

    /// Loads the first mesh found in the scene graph
    pub(super) fn load_first_mesh_from_scene(&self) -> Option<(Mesh, Vec<u32>)> {
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
    pub(super) fn load_mesh(&self, node: Node) -> Option<(Mesh, Vec<u32>)> {
        let gltf_mesh = node.mesh()?;
        let mut bones = Bones::default();
        let mut joint_node_index_of = HashMap::new();

        if let Some(skin) = node.skin() {
            self.build_bones_from_skin(skin, node.clone(), &mut bones, &mut joint_node_index_of);
        }

        let mut buffers = self.read_mesh_primitives(gltf_mesh, &joint_node_index_of)?;

        if buffers.is_empty() {
            return None;
        }

        buffers.fill_missing();
        Some(buffers.build_mesh(bones))
    }

    /// Searches the node hierarchy recursively for the first available mesh
    fn load_first_mesh_from_node(&self, node: Node) -> Option<(Mesh, Vec<u32>)> {
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
        let mut start_vertex = 0u32;

        for prim in mesh.primitives() {
            match self.extract_primitive_mesh_data(prim, joint_node_index_of) {
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
}

impl<'a> From<&'a SkinAttributes> for SkinSlices<'a> {
    fn from(value: &'a SkinAttributes) -> Self {
        Self {
            joints: value.0.as_slice(),
            weights: value.1.as_slice(),
        }
    }
}
