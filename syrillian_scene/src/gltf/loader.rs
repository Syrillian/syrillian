use crate::GltfScene;
use crate::gltf::meshes::{MeshData, MeshLoadResult};
use crate::gltf::prefabs::build_prefab_node;
use crate::gltf::textures::{TextureUsageInfo, collect_material_texture_usage};
use crate::scene_loader::SceneLoader;
use crate::utils::{unique_name, virtual_root_from_path};
use gltf::{self, Node};
use snafu::{OptionExt, Snafu};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use syrillian::World;
use syrillian::core::GameObjectId;
use syrillian_asset::PrefabAsset;
use syrillian_asset::store::streaming::asset_store::hash_relative_path;
use syrillian_asset::store::streaming::packaged_scene::{
    PackagedAnimationAsset, PackagedMaterialAsset, PackagedMeshAsset, PackagedPrefabAsset,
    PackagedScene, PackagedSkinnedMeshAsset, PackagedTextureAsset,
};

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
#[derive(Debug, Snafu)]
#[snafu(context(suffix(Err)), visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("glTF contains no scenes"))]
    GltfNoScenes,
    #[snafu(display("failed to import glTF scene: {source}"))]
    GltfImport { source: gltf::Error },
}

/// Loader utilities for glTF extraction and optional immediate world instantiation
pub struct GltfLoader;

impl GltfLoader {
    /// Loads a glTF file by first extracting it to packaged assets and then instantiating it in the world
    pub fn spawn(world: &mut World, path: &str) -> Result<GameObjectId, Error> {
        let virtual_root = virtual_root_from_path(path);
        let packaged_scene = Self::extract_packaged_scene_from_path(path, virtual_root)?;
        Ok(SceneLoader::load_packaged_scene(world, packaged_scene))
    }

    /// Loads a glTF memory buffer by extracting it and immediately instantiating it in the world
    pub fn load_buffer(world: &mut World, model: &[u8]) -> Result<GameObjectId, Error> {
        let packaged_scene = Self::extract_packaged_scene_from_buffer(model, "memory_scene")?;
        Ok(SceneLoader::load_packaged_scene(world, packaged_scene))
    }

    /// Parses a glTF scene directly from an in-memory buffer
    pub fn load_scene_from_buffer(model: &[u8]) -> Result<GltfScene, Error> {
        GltfScene::from_slice(model)
    }

    /// Loads the first mesh found in the document referenced by the provided path
    pub fn load_first_mesh(path: &str) -> Result<MeshData, Error> {
        let scene = GltfScene::import(path)?;
        Ok(scene.load_first_mesh_from_scene())
    }

    /// Loads the first mesh contained in the provided glTF buffer
    pub fn load_first_mesh_from_buffer(model: &[u8]) -> Result<MeshData, Error> {
        let scene = GltfScene::from_slice(model)?;
        Ok(scene.load_first_mesh_from_scene())
    }

    /// Extracts a glTF file into package representation
    pub fn extract_packaged_scene_from_path<P: AsRef<Path>>(
        path: P,
        virtual_root: impl Into<String>,
    ) -> Result<PackagedScene, Error> {
        let scene = GltfScene::import(path)?;
        Self::extract_packaged_scene(&scene, virtual_root)
    }

    /// Extracts a glTF memory buffer into packaged representation
    pub fn extract_packaged_scene_from_buffer(
        model: &[u8],
        virtual_root: impl Into<String>,
    ) -> Result<PackagedScene, Error> {
        let scene = GltfScene::from_slice(model)?;
        Self::extract_packaged_scene(&scene, virtual_root)
    }

    /// Extracts meshes/textures/animations and prefab layout from a parsed glTF scene
    pub fn extract_packaged_scene(
        scene: &GltfScene,
        virtual_root: impl Into<String>,
    ) -> Result<PackagedScene, Error> {
        let root_scene = scene
            .doc
            .default_scene()
            .or_else(|| scene.doc.scenes().next())
            .context(GltfNoScenesErr)?;

        let virtual_root = virtual_root.into();

        let parent_of = build_parent_index_map(scene);
        let node_by_index = scene
            .doc
            .nodes()
            .map(|node| (node.index(), node))
            .collect::<HashMap<_, _>>();

        let mut reachable = HashSet::new();
        for node in root_scene.nodes() {
            collect_node_indices(node, &mut reachable);
        }

        let mut required_indices = HashSet::new();
        for node in scene.doc.nodes() {
            if node.mesh().is_some() {
                required_indices.insert(node.index());
            }
        }
        for animation in scene.doc.animations() {
            for channel in animation.channels() {
                required_indices.insert(channel.target().node().index());
            }
        }
        for skin in scene.doc.skins() {
            if let Some(skeleton_root) = skin.skeleton() {
                required_indices.insert(skeleton_root.index());
            }
            for joint in skin.joints() {
                required_indices.insert(joint.index());
            }
        }

        let mut supplemental_root_indices = HashSet::new();
        for index in required_indices {
            if reachable.contains(&index) {
                continue;
            }
            supplemental_root_indices.insert(find_unreachable_root(index, &reachable, &parent_of));
        }

        let mut scene_roots = Vec::new();
        let mut seen_root_indices = HashSet::new();
        for node in root_scene.nodes() {
            if seen_root_indices.insert(node.index()) {
                scene_roots.push(node);
            }
        }

        let mut supplemental_root_indices =
            supplemental_root_indices.into_iter().collect::<Vec<_>>();
        supplemental_root_indices.sort_unstable();
        for index in supplemental_root_indices {
            if seen_root_indices.contains(&index) {
                continue;
            }
            let Some(node) = node_by_index.get(&index).cloned() else {
                continue;
            };
            seen_root_indices.insert(index);
            scene_roots.push(node);
        }

        let mut mesh_nodes = Vec::new();
        let mut visited_mesh_nodes = HashSet::new();
        for node in &scene_roots {
            collect_mesh_nodes(node.clone(), &mut mesh_nodes, &mut visited_mesh_nodes);
        }

        let mut meshes_out = Vec::new();
        let mut skinned_meshes_out = Vec::new();
        let mut mesh_path_of = HashMap::new();
        let mut mesh_materials_of: HashMap<usize, Vec<u32>> = HashMap::new();
        let mut used_mesh_names = HashSet::new();
        let mut used_material_indices = HashSet::new();

        for node in mesh_nodes {
            let Some(gltf_mesh) = node.mesh() else {
                continue;
            };
            let mesh_index = gltf_mesh.index();
            let node_index = node.index();

            let Some((mesh, material_indices)) = scene.load_mesh(node.clone()) else {
                continue;
            };

            for material_index in &material_indices {
                if *material_index != u32::MAX {
                    used_material_indices.insert(*material_index);
                }
            }

            let name = unique_name(
                gltf_mesh.name(),
                || format!("mesh_{mesh_index}_node_{node_index}"),
                &mut used_mesh_names,
            );
            let virtual_path = format!("{virtual_root}/Meshes/{name}");

            mesh_path_of.insert(node_index, virtual_path.clone());
            mesh_materials_of.insert(node_index, material_indices);
            match mesh {
                MeshLoadResult::Unskinned(mesh) => meshes_out.push(PackagedMeshAsset {
                    virtual_path,
                    asset: mesh,
                }),

                MeshLoadResult::Skinned(mesh) => {
                    skinned_meshes_out.push(PackagedSkinnedMeshAsset {
                        virtual_path,
                        asset: mesh,
                    })
                }
            }
        }

        let mut texture_usage = HashMap::new();
        for material_index in &used_material_indices {
            let Some(material) = scene
                .doc
                .materials()
                .find(|material| material.index() == Some(*material_index as usize))
            else {
                continue;
            };
            collect_material_texture_usage(material, &mut texture_usage);
        }

        let mut textures_out = Vec::new();
        let mut texture_path_of = HashMap::new();
        let mut used_texture_names = HashSet::new();
        let mut texture_requests: Vec<(usize, TextureUsageInfo)> =
            texture_usage.into_iter().collect();
        texture_requests.sort_by_key(|(index, _)| *index);

        for (texture_index, usage) in texture_requests {
            let Some(texture) = scene
                .doc
                .textures()
                .find(|texture| texture.index() == texture_index)
            else {
                continue;
            };

            let Some(decoded) = scene.decode_texture(&texture, usage) else {
                continue;
            };

            let name = unique_name(
                texture.name(),
                || format!("texture_{texture_index}"),
                &mut used_texture_names,
            );
            let virtual_path = format!("{virtual_root}/Textures/{name}");

            texture_path_of.insert(texture_index, virtual_path.clone());
            textures_out.push(PackagedTextureAsset {
                virtual_path,
                asset: decoded,
            });
        }

        let mut material_indices_sorted = used_material_indices.into_iter().collect::<Vec<_>>();
        material_indices_sorted.sort_unstable();

        let mut materials_out = Vec::new();
        let mut material_hash_of = HashMap::new();
        let mut used_material_names = HashSet::new();

        for material_index in material_indices_sorted {
            let Some(material) = scene
                .doc
                .materials()
                .find(|material| material.index() == Some(material_index as usize))
            else {
                continue;
            };

            let prefab_material =
                scene.decode_material(&material, material_index, &texture_path_of);

            let material_name = unique_name(
                Some(prefab_material.name.as_str()),
                || format!("material_{material_index}"),
                &mut used_material_names,
            );
            let material_virtual_path = format!("{virtual_root}/Materials/{material_name}");
            let material_hash = hash_relative_path(&material_virtual_path);
            materials_out.push(PackagedMaterialAsset {
                virtual_path: material_virtual_path,
                asset: prefab_material,
            });

            material_hash_of.insert(material_index, material_hash);
        }

        let mut animation_assets = Vec::new();
        let mut animations_out = Vec::new();
        let mut used_animation_names = HashSet::new();

        for (index, clip) in scene.decode_animations().into_iter().enumerate() {
            let name = unique_name(
                Some(clip.name.as_str()),
                || format!("animation_{index}"),
                &mut used_animation_names,
            );
            let virtual_path = format!("{virtual_root}/Animations/{name}");
            animation_assets.push(virtual_path.clone());
            animations_out.push(PackagedAnimationAsset {
                virtual_path,
                asset: clip,
            });
        }

        let mut prefab_nodes = Vec::new();
        let mut prefab_roots = Vec::new();
        for node in scene_roots {
            let root_index = build_prefab_node(
                node,
                &mut prefab_nodes,
                &mesh_path_of,
                &mesh_materials_of,
                &material_hash_of,
            );
            prefab_roots.push(root_index);
        }

        let prefab = PrefabAsset {
            source: virtual_root.clone(),
            root_nodes: prefab_roots,
            nodes: prefab_nodes,
            animation_assets,
        };

        Ok(PackagedScene {
            virtual_root: virtual_root.clone(),
            meshes: meshes_out,
            skinned_meshes: skinned_meshes_out,
            textures: textures_out,
            materials: materials_out,
            animations: animations_out,
            prefab: PackagedPrefabAsset {
                virtual_path: format!("{virtual_root}/Prefab/scene_prefab"),
                asset: prefab,
            },
        })
    }
}

fn collect_mesh_nodes<'a>(
    node: Node<'a>,
    mesh_nodes: &mut Vec<Node<'a>>,
    visited: &mut HashSet<usize>,
) {
    if !visited.insert(node.index()) {
        return;
    }

    if node.mesh().is_some() {
        mesh_nodes.push(node.clone());
    }

    for child in node.children() {
        collect_mesh_nodes(child, mesh_nodes, visited);
    }
}

fn collect_node_indices(node: Node<'_>, out: &mut HashSet<usize>) {
    if !out.insert(node.index()) {
        return;
    }
    for child in node.children() {
        collect_node_indices(child, out);
    }
}

fn build_parent_index_map(scene: &GltfScene) -> HashMap<usize, Option<usize>> {
    let mut parent_of = scene
        .doc
        .nodes()
        .map(|node| (node.index(), None))
        .collect::<HashMap<_, _>>();

    for node in scene.doc.nodes() {
        for child in node.children() {
            parent_of.insert(child.index(), Some(node.index()));
        }
    }

    parent_of
}

fn find_unreachable_root(
    mut node_index: usize,
    reachable: &HashSet<usize>,
    parent_of: &HashMap<usize, Option<usize>>,
) -> usize {
    loop {
        let Some(parent) = parent_of.get(&node_index).copied().flatten() else {
            return node_index;
        };
        if reachable.contains(&parent) {
            return node_index;
        }
        node_index = parent;
    }
}
