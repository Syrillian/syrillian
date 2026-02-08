use super::gltf_loader::GltfScene;
use gltf::{self, Node};
use std::collections::HashMap;
use syrillian::core::Bones;
use syrillian::math::Mat4;

/// Populates the engine bone structure from a glTF skin.
pub(super) fn build_bones_from_skin(
    scene: &GltfScene,
    skin: gltf::Skin,
    mesh_node: Node,
    out: &mut Bones,
    joint_map: &mut HashMap<usize, usize>,
) {
    let mut names = Vec::<String>::new();
    let mut parents = Vec::<Option<usize>>::new();
    let mut inverse_bind = Vec::<Mat4>::new();
    let mut index_of = HashMap::<String, usize>::new();

    let mut node_map = HashMap::<usize, (Option<usize>, Mat4)>::new();
    for scene0 in scene.doc.scenes() {
        for node in scene0.nodes() {
            build_node_map_recursive(node, None, &mut node_map);
        }
    }

    let get_buf = |b: gltf::Buffer| -> Option<&[u8]> { Some(&scene.buffers[b.index()].0) };
    let inverse_matrices: Vec<Mat4> = skin
        .reader(get_buf)
        .read_inverse_bind_matrices()
        .map(|iter| iter.map(|m| Mat4::from_cols_array_2d(&m)).collect())
        .unwrap_or_default();

    for (joint_idx, joint_node) in skin.joints().enumerate() {
        let name = joint_node.name().unwrap_or("<unnamed>").to_string();
        let my_index = names.len();
        names.push(name.clone());
        index_of.insert(name.clone(), my_index);
        joint_map.insert(joint_idx, my_index);

        let parent = node_map
            .get(&joint_node.index())
            .and_then(|(parent, _)| *parent)
            .and_then(|pi| {
                skin.joints()
                    .position(|jn| jn.index() == pi)
                    .and_then(|local| joint_map.get(&local).copied())
            });
        parents.push(parent);

        let inverse = inverse_matrices.get(joint_idx).cloned().unwrap_or_default();
        inverse_bind.push(inverse);
    }

    let mesh_global = global_transform_of(mesh_node.index(), &node_map);
    let mesh_global_inv = mesh_global.inverse();

    let mut bind_global = vec![Mat4::IDENTITY; names.len()];
    for (i, joint_node) in skin.joints().enumerate() {
        let g_world = global_transform_of(joint_node.index(), &node_map);
        bind_global[i] = mesh_global_inv * g_world;
    }

    let mut bind_local = vec![Mat4::IDENTITY; names.len()];
    for (i, parent) in parents.iter().enumerate() {
        bind_local[i] = match parent {
            None => bind_global[i],
            Some(p) => bind_global[*p].inverse() * bind_global[i],
        };
    }

    let mut children = vec![Vec::new(); names.len()];
    for (i, parent) in parents.iter().enumerate() {
        match parent {
            None => out.roots.push(i),
            Some(p) => children[*p].push(i),
        }
    }

    out.names = names;
    out.parents = parents;
    out.children = children;
    out.inverse_bind = inverse_bind;
    out.bind_global = bind_global;
    out.bind_local = bind_local;
    out.index_of = index_of;
}

/// Builds a mapping from glTF node indices to their parents and transforms.
fn build_node_map_recursive(
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

/// Computes the global transform matrix of a node from the cached node map.
fn global_transform_of(node_idx: usize, node_map: &HashMap<usize, (Option<usize>, Mat4)>) -> Mat4 {
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
