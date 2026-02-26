use crate::GltfScene;
use gltf::{self, Node};
use std::collections::HashMap;
use syrillian::math::Mat4;
use syrillian_asset::mesh::Bones;
impl GltfScene {
    /// Populates the bone structure from a glTF skin
    /// Builds a mapping from glTF node indices to their parents and transforms
    pub fn build_bones_from_skin(
        &self,
        skin: gltf::Skin,
        mesh_node: Node,
        joint_map: &mut HashMap<usize, usize>,
    ) -> Bones {
        let mut bones = Bones::new();

        let mut node_map = HashMap::<usize, (Option<usize>, Mat4)>::new();
        for scene0 in self.doc.scenes() {
            for node in scene0.nodes() {
                build_node_map_recursive(node, None, &mut node_map);
            }
        }

        let get_buf = |b: gltf::Buffer| -> Option<&[u8]> { Some(&self.buffers[b.index()].0) };
        let inverse_matrices: Vec<Mat4> = skin
            .reader(get_buf)
            .read_inverse_bind_matrices()
            .map(|iter| iter.map(|m| Mat4::from_cols_array_2d(&m)).collect())
            .unwrap_or_default();

        for (joint_idx, joint_node) in skin.joints().enumerate() {
            let name = joint_node.name().unwrap_or("<unnamed>").to_string();
            let my_index = bones.names.len();
            bones.names.push(name.clone());
            bones.index_of.insert(name.clone(), my_index);
            joint_map.insert(joint_idx, my_index);

            let parent = node_map
                .get(&joint_node.index())
                .and_then(|(parent, _)| *parent)
                .and_then(|pi| {
                    skin.joints()
                        .position(|jn| jn.index() == pi)
                        .and_then(|local| joint_map.get(&local).copied())
                });
            bones.parents.push(parent);

            let inverse = inverse_matrices.get(joint_idx).cloned().unwrap_or_default();
            bones.inverse_bind.push(inverse);
        }

        let mesh_global = global_transform_of(mesh_node.index(), &node_map);
        let mesh_global_inv = mesh_global.inverse();

        bones.bind_global = vec![Mat4::IDENTITY; bones.names.len()];
        for (i, joint_node) in skin.joints().enumerate() {
            let g_world = global_transform_of(joint_node.index(), &node_map);
            bones.bind_global[i] = mesh_global_inv * g_world;
        }

        bones.bind_local = vec![Mat4::IDENTITY; bones.names.len()];
        for (i, parent) in bones.parents.iter().enumerate() {
            bones.bind_local[i] = match parent {
                None => bones.bind_global[i],
                Some(p) => bones.bind_global[*p].inverse() * bones.bind_global[i],
            };
        }

        bones.children = vec![Vec::new(); bones.names.len()];
        for (i, parent) in bones.parents.iter().enumerate() {
            match parent {
                None => bones.roots.push(i),
                Some(p) => bones.children[*p].push(i),
            }
        }

        bones
    }
}
pub fn build_node_map_recursive(
    node: Node,
    parent: Option<usize>,
    out: &mut HashMap<usize, (Option<usize>, Mat4)>,
) {
    out.insert(
        node.index(),
        (parent, Mat4::from_cols_array_2d(&node.transform().matrix())),
    );
    for child in node.children() {
        build_node_map_recursive(child, Some(node.index()), out);
    }
}

/// Computes the global transform matrix of a node from the cached node map
pub fn global_transform_of(
    node_idx: usize,
    node_map: &HashMap<usize, (Option<usize>, Mat4)>,
) -> Mat4 {
    let mut matrix = Mat4::IDENTITY;
    let mut current = Some(node_idx);
    while let Some(index) = current {
        if let Some((parent, local)) = node_map.get(&index) {
            matrix = *local * matrix;
            current = *parent;
        } else {
            break;
        }
    }
    matrix
}
