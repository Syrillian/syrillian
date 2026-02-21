use crate::prefab_material_instantiation::PrefabMaterialInstantiation;
use crate::utils::json_to_reflection_value;
use snafu::{ResultExt, Snafu};
use std::collections::{HashMap, HashSet};
use syrillian::World;
use syrillian::assets::{HMaterialInstance, HMesh, HTexture2D, Mesh, Texture2D};
use syrillian::core::GameObjectId;
use syrillian::tracing::{trace, warn};
use syrillian_asset::store::H;
use syrillian_asset::store::packaged_scene::PackagedScene;
use syrillian_asset::store::streaming_asset_store::{hash_relative_path, normalize_asset_path};
use syrillian_asset::{
    AnimationClip, AssetStore, AssetStreamingError, PrefabAsset, PrefabMaterial, PrefabMeshBinding,
    StreamingLoadableAsset,
};
use syrillian_components::{AnimationComponent, MeshRenderer, SkeletalComponent};

#[derive(Debug, Snafu)]
#[snafu(context(suffix(Err)), visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("failed to stream packaged asset: {source}"))]
    Streaming { source: AssetStreamingError },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/// Loader utilities for world instantiation from packaged scene definitions.
pub struct SceneLoader;

impl SceneLoader {
    /// Loads a prefab from mounted streaming packages and instantiates it in the world.
    pub fn load_prefab_by_path(world: &mut World, prefab_path: &str) -> Result<GameObjectId> {
        let prefab_handle = world
            .assets
            .load_by_path::<PrefabAsset>(prefab_path)
            .context(StreamingErr)?;
        let Some(prefab) = world
            .assets
            .prefabs
            .try_get(prefab_handle)
            .map(|item| item.clone())
        else {
            return Err(Error::Streaming {
                source: AssetStreamingError::AssetNotFound {
                    path: prefab_path.to_string(),
                },
            });
        };
        Ok(Self::load_prefab_asset(world, &prefab))
    }

    /// Instantiates a prefab already available in memory or the local asset store.
    pub fn load_prefab_asset(world: &mut World, prefab: &PrefabAsset) -> GameObjectId {
        let mut context = PrefabInstantiationContext::new(world);
        context.spawn_prefab(prefab)
    }

    /// Registers all assets from a packaged scene and spawns its prefab hierarchy.
    pub fn load_packaged_scene(world: &mut World, scene: PackagedScene) -> GameObjectId {
        let mut context = PrefabInstantiationContext::new(world);
        let prefab = context.register_packaged_scene(scene);
        context.spawn_prefab(&prefab)
    }
}

struct PrefabInstantiationContext<'a> {
    world: &'a mut World,
    mesh_handles_by_path: HashMap<String, HMesh>,
    texture_handles_by_path: HashMap<String, HTexture2D>,
    material_handles_by_hash: HashMap<u64, HMaterialInstance>,
    animation_clips_by_path: HashMap<String, AnimationClip>,
}

impl<'a> PrefabInstantiationContext<'a> {
    fn new(world: &'a mut World) -> Self {
        Self {
            world,
            mesh_handles_by_path: HashMap::new(),
            texture_handles_by_path: HashMap::new(),
            material_handles_by_hash: HashMap::new(),
            animation_clips_by_path: HashMap::new(),
        }
    }

    fn register_packaged_scene(&mut self, scene: PackagedScene) -> PrefabAsset {
        for mesh_asset in scene.meshes {
            let path = normalize_asset_path(&mesh_asset.virtual_path);
            let handle = self.world.assets.meshes.add(mesh_asset.mesh);
            self.mesh_handles_by_path.insert(path, handle);
        }

        for texture_asset in scene.textures {
            let path = normalize_asset_path(&texture_asset.virtual_path);
            let handle = self.world.assets.textures.add(texture_asset.texture);
            self.texture_handles_by_path.insert(path, handle);
        }

        for material_asset in scene.materials {
            let hash = hash_relative_path(&material_asset.virtual_path);
            let prefab_material = material_asset.material;
            let material_instance =
                PrefabMaterialInstantiation::instantiate(&prefab_material, |path| {
                    self.resolve_texture(path)
                });
            self.world.assets.prefab_materials.add(prefab_material);
            let material_handle = self.world.assets.material_instances.add(material_instance);
            self.material_handles_by_hash.insert(hash, material_handle);
        }

        for animation_asset in scene.animations {
            let path = normalize_asset_path(&animation_asset.virtual_path);
            self.animation_clips_by_path
                .insert(path, animation_asset.clip);
        }

        let prefab = scene.prefab.prefab;
        self.world.assets.prefabs.add(prefab.clone());
        prefab
    }

    fn spawn_prefab(&mut self, prefab: &PrefabAsset) -> GameObjectId {
        let mut root = self.world.new_object("Instantiated Prefab");
        for node_index in &prefab.root_nodes {
            if let Some(child) = self.spawn_node(prefab, *node_index) {
                root.add_child(child);
            }
        }

        self.attach_animations(prefab, root);
        root
    }

    fn spawn_node(&mut self, prefab: &PrefabAsset, node_index: u32) -> Option<GameObjectId> {
        let node = prefab.nodes.get(node_index as usize)?.clone();
        let mut object = self.world.new_object(node.name.clone());

        object.transform.set_local_position_vec(node.local_position);
        object.transform.set_local_rotation(node.local_rotation);
        object
            .transform
            .set_nonuniform_local_scale(node.local_scale);

        if let Some(extras_json) = node.extras_json.as_deref() {
            match serde_json::from_str::<serde_json::Value>(extras_json) {
                Ok(serde_json::Value::Object(props)) => object.add_properties(
                    props
                        .into_iter()
                        .map(|(key, value)| (key, json_to_reflection_value(value))),
                ),
                Ok(_) => {
                    trace!("Ignoring non-object prefab extras for node '{}'", node.name);
                }
                Err(error) => {
                    trace!(
                        "Failed to parse prefab extras for node '{}': {error}",
                        node.name
                    );
                }
            }
        }

        if let Some(mesh_binding) = node.mesh.as_ref() {
            self.attach_mesh_binding(&mut object, mesh_binding);
        }

        for child in node.children {
            if let Some(child_object) = self.spawn_node(prefab, child) {
                object.add_child(child_object);
            }
        }

        Some(object)
    }

    fn attach_mesh_binding(&mut self, object: &mut GameObjectId, mesh_binding: &PrefabMeshBinding) {
        let Some(mesh_handle) = self.resolve_mesh(&mesh_binding.mesh_asset) else {
            return;
        };

        let has_bones = self
            .world
            .assets
            .meshes
            .try_get(mesh_handle)
            .is_some_and(|mesh| !mesh.bones.is_empty());

        let material_handles = if mesh_binding.material_hashes.is_empty() {
            None
        } else {
            Some(
                mesh_binding
                    .material_hashes
                    .iter()
                    .map(|material_hash| self.resolve_material(*material_hash))
                    .collect::<Vec<_>>(),
            )
        };

        object
            .add_component::<MeshRenderer>()
            .change_mesh(mesh_handle, material_handles);

        if has_bones {
            object.add_component::<SkeletalComponent>();
        }
    }

    fn attach_animations(&mut self, prefab: &PrefabAsset, mut root: GameObjectId) {
        let mut clips = Vec::new();
        for animation_path in &prefab.animation_assets {
            let Some(clip) = self.resolve_animation(animation_path) else {
                warn!("Requested animation clip {animation_path} couldn't be resolved");
                continue;
            };
            clips.push(clip);
        }

        if !clips.is_empty() {
            let autoplay_indices = select_default_autoplay_indices(&clips);
            let mut animation = root.add_component::<AnimationComponent>();
            animation.set_clips(clips);
            animation.play_indices(&autoplay_indices, true, 1.0, 1.0);
        }
    }

    fn resolve_mesh(&mut self, mesh_asset_path: &str) -> Option<HMesh> {
        resolve_cached_by_path::<Mesh>(
            self.world.assets.as_ref(),
            &mut self.mesh_handles_by_path,
            mesh_asset_path,
        )
    }

    fn resolve_texture(&mut self, texture_path: &str) -> Option<HTexture2D> {
        resolve_cached_by_path::<Texture2D>(
            self.world.assets.as_ref(),
            &mut self.texture_handles_by_path,
            texture_path,
        )
    }

    fn resolve_material(&mut self, material_hash: Option<u64>) -> HMaterialInstance {
        let Some(material_hash) = material_hash else {
            return HMaterialInstance::DEFAULT;
        };

        if let Some(handle) = self.material_handles_by_hash.get(&material_hash).copied() {
            return handle;
        }

        let Ok(prefab_material_handle) = self
            .world
            .assets
            .load_by_hash::<PrefabMaterial>(material_hash)
        else {
            return HMaterialInstance::FALLBACK;
        };
        let Some(prefab_material) = self
            .world
            .assets
            .prefab_materials
            .try_get(prefab_material_handle)
            .map(|material| material.clone())
        else {
            return HMaterialInstance::FALLBACK;
        };

        let material_instance =
            PrefabMaterialInstantiation::instantiate(&prefab_material, |path| {
                self.resolve_texture(path)
            });
        let handle = self.world.assets.material_instances.add(material_instance);
        self.material_handles_by_hash.insert(material_hash, handle);
        handle
    }

    fn resolve_animation(&mut self, animation_path: &str) -> Option<AnimationClip> {
        let path = normalize_asset_path(animation_path);
        if let Some(clip) = self.animation_clips_by_path.get(&path).cloned() {
            return Some(clip);
        }

        let handle = self
            .world
            .assets
            .load_by_path::<AnimationClip>(&path)
            .ok()?;
        let clip = self
            .world
            .assets
            .animation_clips
            .try_get(handle)
            .map(|item| item.clone())?;

        self.animation_clips_by_path.insert(path, clip.clone());
        Some(clip)
    }
}

fn resolve_cached_by_path<A: StreamingLoadableAsset>(
    assets: &AssetStore,
    cache: &mut HashMap<String, H<A>>,
    asset_path: &str,
) -> Option<H<A>> {
    let path = normalize_asset_path(asset_path);
    if let Some(handle) = cache.get(&path).copied() {
        return Some(handle);
    }

    let handle = assets.load_by_path::<A>(&path).ok()?;
    cache.insert(path, handle);
    Some(handle)
}

fn select_default_autoplay_indices(clips: &[AnimationClip]) -> Vec<usize> {
    if clips.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut claimed_targets = HashSet::<String>::new();
    for (index, clip) in clips.iter().enumerate() {
        if index == 0 {
            selected.push(index);
            for channel in &clip.channels {
                claimed_targets.insert(channel.target_name.clone());
            }
            continue;
        }

        let mut overlaps = false;
        for channel in &clip.channels {
            if claimed_targets.contains(&channel.target_name) {
                overlaps = true;
                break;
            }
        }
        if overlaps {
            continue;
        }

        selected.push(index);
        for channel in &clip.channels {
            claimed_targets.insert(channel.target_name.clone());
        }
    }

    if selected.is_empty() {
        selected.push(0);
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::select_default_autoplay_indices;
    use syrillian_asset::{AnimationChannel, AnimationClip};

    fn clip(name: &str, targets: &[&str]) -> AnimationClip {
        AnimationClip {
            name: name.to_string(),
            channels: targets
                .iter()
                .map(|target| AnimationChannel {
                    target_name: (*target).to_string(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn autoplay_selects_first_and_disjoint_targets() {
        let clips = vec![
            clip("rig", &["root", "arm"]),
            clip("stick", &["other"]),
            clip("alt_rig", &["root"]),
        ];

        let selected = select_default_autoplay_indices(&clips);
        assert_eq!(selected, vec![0, 1]);
    }

    #[test]
    fn autoplay_falls_back_to_first_clip() {
        let clips = vec![AnimationClip::default()];
        let selected = select_default_autoplay_indices(&clips);
        assert_eq!(selected, vec![0]);
    }
}
