use gltf::Node;
use std::collections::HashMap;
use syrillian::math::{Quat, Vec3};
use syrillian_asset::{PrefabMeshBinding, PrefabNode};

pub fn build_prefab_node(
    node: Node,
    nodes: &mut Vec<PrefabNode>,
    mesh_path_of: &HashMap<usize, String>,
    mesh_materials_of: &HashMap<usize, Vec<u32>>,
    material_hash_of: &HashMap<u32, u64>,
) -> u32 {
    let node_index = nodes.len() as u32;
    let name = node.name().unwrap_or("Unnamed").to_string();
    let (position, rotation, scale) = node.transform().decomposed();

    let mesh_binding = node.mesh().and_then(|_mesh| {
        let node_index = node.index();
        let mesh_asset = mesh_path_of.get(&node_index)?.clone();
        let material_hashes = mesh_materials_of
            .get(&node_index)
            .map(|materials| {
                materials
                    .iter()
                    .map(|material_index| {
                        if *material_index == u32::MAX {
                            None
                        } else {
                            material_hash_of.get(material_index).copied()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(PrefabMeshBinding {
            mesh_asset,
            material_hashes,
        })
    });

    let extras_json = node
        .extras()
        .as_ref()
        .map(|extras| extras.get().to_string());

    nodes.push(PrefabNode {
        name,
        local_position: Vec3::from(position),
        local_rotation: Quat::from_array(rotation),
        local_scale: Vec3::from(scale),
        children: Vec::new(),
        mesh: mesh_binding,
        extras_json,
    });

    let children = node
        .children()
        .map(|child| {
            build_prefab_node(
                child,
                nodes,
                mesh_path_of,
                mesh_materials_of,
                material_hash_of,
            )
        })
        .collect::<Vec<_>>();
    nodes[node_index as usize].children = children;

    node_index
}
