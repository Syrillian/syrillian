use crate::GltfScene;
use crate::gltf::meshes::MeshData;
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
use syrillian_asset::store::packaged_scene::{
    PackagedAnimationAsset, PackagedMaterialAsset, PackagedMeshAsset, PackagedPrefabAsset,
    PackagedScene, PackagedTextureAsset,
};
use syrillian_asset::store::streaming_asset_store::hash_relative_path;

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

        let mut mesh_node_of: HashMap<usize, Node<'_>> = HashMap::new();
        for node in root_scene.nodes() {
            collect_mesh_nodes(node, &mut mesh_node_of);
        }

        let mut meshes_out = Vec::new();
        let mut mesh_path_of = HashMap::new();
        let mut mesh_materials_of: HashMap<usize, Vec<u32>> = HashMap::new();
        let mut used_mesh_names = HashSet::new();
        let mut used_material_indices = HashSet::new();

        for gltf_mesh in scene.doc.meshes() {
            let mesh_index = gltf_mesh.index();
            let Some(node) = mesh_node_of.get(&mesh_index).cloned() else {
                continue;
            };

            let Some((mesh, material_indices)) = scene.load_mesh(node) else {
                continue;
            };

            for material_index in &material_indices {
                if *material_index != u32::MAX {
                    used_material_indices.insert(*material_index);
                }
            }

            let name = unique_name(
                gltf_mesh.name(),
                || format!("mesh_{mesh_index}"),
                &mut used_mesh_names,
            );
            let virtual_path = format!("{virtual_root}/Meshes/{name}");

            mesh_path_of.insert(mesh_index, virtual_path.clone());
            mesh_materials_of.insert(mesh_index, material_indices);
            meshes_out.push(PackagedMeshAsset { virtual_path, mesh });
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
                texture: decoded,
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
                material: prefab_material,
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
            animations_out.push(PackagedAnimationAsset { virtual_path, clip });
        }

        let mut prefab_nodes = Vec::new();
        let mut prefab_roots = Vec::new();
        for node in root_scene.nodes() {
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
            textures: textures_out,
            materials: materials_out,
            animations: animations_out,
            prefab: PackagedPrefabAsset {
                virtual_path: format!("{virtual_root}/Prefab/scene_prefab"),
                prefab,
            },
        })
    }
}

fn collect_mesh_nodes<'a>(node: Node<'a>, mesh_node_of: &mut HashMap<usize, Node<'a>>) {
    if let Some(mesh) = node.mesh() {
        mesh_node_of.entry(mesh.index()).or_insert(node.clone());
    }

    for child in node.children() {
        collect_mesh_nodes(child, mesh_node_of);
    }
}
