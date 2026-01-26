use crate::World;
use crate::assets::{HMaterial, Mesh};
use crate::components::{
    AnimationComponent, MeshRenderer, PointLightComponent, SkeletalComponent, SpotLightComponent,
    SunLightComponent,
};
use crate::core::{GameObjectId, reflection};
use crate::rendering::lights::Light;
use crate::utils::animation::{AnimationClip, Channel, TransformKeys};
use gltf::animation::util::ReadOutputs;
use gltf::khr_lights_punctual::Kind;
use gltf::{self, Document, Node};
use nalgebra::{Quaternion, UnitQuaternion, Vector3};
use snafu::{OptionExt, ResultExt, Snafu};
use std::collections::HashMap;
use syrillian_utils::debug_panic;
use tracing::trace;

mod bones;
mod meshes;
mod textures;

pub use meshes::MeshData;

#[derive(Debug, Snafu)]
#[snafu(context(suffix(Err)))]
pub enum Error {
    #[snafu(display("glTF contains no scenes"))]
    GltfNoScenes,
    #[snafu(display("failed to import glTF scene: {source}"))]
    GltfImport { source: gltf::Error },
}

/// Container for a glTF document and its binary attachments.
pub struct GltfScene {
    pub doc: Document,
    pub buffers: Vec<gltf::buffer::Data>,
    pub images: Vec<gltf::image::Data>,
}

impl GltfScene {
    /// Imports a glTF scene from disk and gathers its buffers and images.
    pub fn import(path: &str) -> Result<Self, Error> {
        let (doc, buffers, images) = gltf::import(path).context(GltfImportErr)?;
        Ok(Self {
            doc,
            buffers,
            images,
        })
    }

    /// Imports a glTF scene from an in-memory byte slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        let (doc, buffers, images) = gltf::import_slice(bytes).context(GltfImportErr)?;
        Ok(Self {
            doc,
            buffers,
            images,
        })
    }
}

/// Loader utilities for bringing glTF content into the engine.
pub struct SceneLoader;

impl SceneLoader {
    /// Loads a glTF file from disk and spawns its root into the world.
    pub fn load(world: &mut World, path: &str) -> Result<GameObjectId, Error> {
        let scene = GltfScene::import(path)?;
        Self::load_into_world(world, &scene)
    }

    /// Loads a glTF scene from memory and spawns its root into the world.
    pub fn load_buffer(world: &mut World, model: &[u8]) -> Result<GameObjectId, Error> {
        let scene = Self::load_scene_from_buffer(model)?;
        Self::load_into_world(world, &scene)
    }

    /// Parses a glTF scene directly from an in-memory buffer.
    pub fn load_scene_from_buffer(model: &[u8]) -> Result<GltfScene, Error> {
        GltfScene::from_slice(model)
    }

    /// Loads the first mesh found in the document referenced by the provided path.
    pub fn load_first_mesh(path: &str) -> Result<MeshData, Error> {
        let scene = GltfScene::import(path)?;
        Ok(meshes::load_first_from_scene(&scene))
    }

    /// Loads the first mesh contained in the provided glTF buffer.
    pub fn load_first_mesh_from_buffer(model: &[u8]) -> Result<MeshData, Error> {
        let scene = GltfScene::from_slice(model)?;
        Ok(meshes::load_first_from_scene(&scene))
    }

    /// Returns the first mesh found directly from a parsed glTF scene.
    pub fn load_first_from_scene(scene: &GltfScene) -> Option<(Mesh, Vec<u32>)> {
        meshes::load_first_from_scene(scene)
    }

    /// Spawns the glTF scene graph into the world and returns the created root object.
    fn load_into_world(world: &mut World, gltf_scene: &GltfScene) -> Result<GameObjectId, Error> {
        let doc = &gltf_scene.doc;
        let root_scene = doc
            .default_scene()
            .or_else(|| doc.scenes().next())
            .context(GltfNoScenesErr)?;

        let materials = textures::load_materials(gltf_scene, world);
        trace!("Loaded materials");

        let mut root = world.new_object("glTF Scene");
        for node in root_scene.nodes() {
            let child = Self::spawn_node(world, gltf_scene, node, Some(&materials));
            root.add_child(child);
        }

        Self::load_animations(gltf_scene, root);

        Ok(root)
    }

    /// Collects animations from the scene and attaches them to the spawned root.
    fn load_animations(gltf_scene: &GltfScene, mut root: GameObjectId) {
        let clips = animations_from_scene(gltf_scene);
        if !clips.is_empty() {
            let mut anim = root.add_component::<AnimationComponent>();
            anim.set_clips(clips);
            anim.play_index(0, true, 1.0, 1.0);
        }
    }

    /// Recursively spawns a glTF node hierarchy into the world.
    fn spawn_node(
        world: &mut World,
        scene: &GltfScene,
        node: Node,
        materials: Option<&HashMap<u32, HMaterial>>,
    ) -> GameObjectId {
        let name = node.name().unwrap_or("Unnamed").to_string();
        trace!("Starting to build scene object {name:?}");
        let mut obj = world.new_object(name);

        if let Some(extras) = node.extras() {
            match serde_json::de::from_str::<serde_json::Value>(extras.get()) {
                Ok(serde_json::Value::Object(props)) => obj.add_properties(
                    props
                        .into_iter()
                        .map(|(k, v)| (k, reflection::Value::Serde(v))),
                ),
                Ok(_) => trace!(
                    "Ignored custom property that was not a map when loading node into an object"
                ),
                Err(_) => debug_panic!("Custom Property \"{extras}\" couldn't be read"),
            }
        }

        if let Some((mesh, mats)) = meshes::load_mesh(scene, node.clone()) {
            Self::attach_mesh(world, materials, &mut obj, mesh, mats);
        }

        let (p, r, s) = node.transform().decomposed();
        obj.transform.set_local_position_vec(Vector3::from(p));
        obj.transform
            .set_local_rotation(UnitQuaternion::from_quaternion(Quaternion::from(r)));
        obj.transform.set_nonuniform_local_scale(Vector3::from(s));

        load_node_light(node.clone(), obj);

        for child in node.children() {
            let c = Self::spawn_node(world, scene, child, materials);
            obj.add_child(c);
        }

        obj
    }

    /// Attaches a mesh renderer (and skeletal component if required) to the node.
    fn attach_mesh(
        world: &mut World,
        scene_materials: Option<&HashMap<u32, HMaterial>>,
        node_obj: &mut GameObjectId,
        mesh: Mesh,
        materials: Vec<u32>,
    ) {
        let has_bones = !mesh.bones.is_empty();
        let handle = world.assets.meshes.add(mesh);

        if let Some(scene_materials) = scene_materials {
            let m = materials
                .iter()
                .map(|&id| {
                    scene_materials
                        .get(&id)
                        .copied()
                        .unwrap_or(HMaterial::FALLBACK)
                })
                .collect();
            node_obj
                .add_component::<MeshRenderer>()
                .change_mesh(handle, Some(m));
        } else {
            node_obj
                .add_component::<MeshRenderer>()
                .change_mesh(handle, None);
        }

        if has_bones {
            node_obj.add_component::<SkeletalComponent>();
        }
    }
}

/// Builds animation clips from the glTF scene.
fn animations_from_scene(scene: &GltfScene) -> Vec<AnimationClip> {
    let mut clips = Vec::<AnimationClip>::new();

    for anim in scene.doc.animations() {
        let clip = build_animation_clip(scene, anim);
        if !clip.channels.is_empty() {
            clips.push(clip);
        }
    }

    clips
}

/// Converts a glTF animation into an engine animation clip.
fn build_animation_clip(scene: &GltfScene, anim: gltf::Animation) -> AnimationClip {
    let name = anim.name().unwrap_or("Animation").to_string();
    let (channels, duration) = collect_animation_channels(scene, anim);

    AnimationClip {
        name,
        duration,
        channels,
    }
}

/// Collects all channels of a glTF animation.
fn collect_animation_channels(scene: &GltfScene, anim: gltf::Animation) -> (Vec<Channel>, f32) {
    let mut channels = Vec::new();
    let mut max_time = 0.0f32;

    for ch in anim.channels() {
        if let Some((channel, duration)) = read_channel(scene, ch) {
            channels.push(channel);
            max_time = max_time.max(duration);
        }
    }

    (channels, max_time)
}

/// Reads a single animation channel and converts it into an engine animation channel.
fn read_channel(scene: &GltfScene, channel: gltf::animation::Channel) -> Option<(Channel, f32)> {
    let node = channel.target().node();
    let target_name = node
        .name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("node{}", node.index()));

    let reader = channel.reader(|b| Some(&scene.buffers[b.index()].0));
    let times: Vec<f32> = reader.read_inputs()?.collect();
    let duration = times.last().copied().unwrap_or(0.0);
    let outputs = reader.read_outputs()?;
    let keys = build_transform_keys(outputs, &times);

    Some((Channel { target_name, keys }, duration))
}

/// Builds transform keyframes from glTF animation outputs.
fn build_transform_keys(outputs: ReadOutputs, times: &[f32]) -> TransformKeys {
    match outputs {
        ReadOutputs::Translations(values) => {
            let translations: Vec<Vector3<f32>> = values
                .into_iter()
                .map(|v| Vector3::new(v[0], v[1], v[2]))
                .collect();
            TransformKeys {
                t_times: times.to_vec(),
                t_values: translations,
                ..TransformKeys::default()
            }
        }
        ReadOutputs::Rotations(values) => {
            let rotations: Vec<UnitQuaternion<f32>> = values
                .into_f32()
                .map(|q| UnitQuaternion::from_quaternion(Quaternion::new(q[3], q[0], q[1], q[2])))
                .collect();
            TransformKeys {
                r_times: times.to_vec(),
                r_values: rotations,
                ..TransformKeys::default()
            }
        }
        ReadOutputs::Scales(values) => {
            let scales: Vec<Vector3<f32>> = values
                .into_iter()
                .map(|v| Vector3::new(v[0], v[1], v[2]))
                .collect();
            TransformKeys {
                s_times: times.to_vec(),
                s_values: scales,
                ..TransformKeys::default()
            }
        }
        _ => TransformKeys::default(),
    }
}

/// Attaches lights defined on a glTF node.
fn load_node_light(node: Node, mut obj: GameObjectId) {
    if let Some(nl) = node.light() {
        let color = Vector3::new(nl.color()[0], nl.color()[1], nl.color()[2]);
        let intensity = nl.intensity();
        let range = nl.range().unwrap_or(100.0);

        match nl.kind() {
            Kind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            } => {
                let mut spot = obj.add_component::<SpotLightComponent>();
                let d = spot.data_mut(true);
                d.color = color;
                d.inner_angle = inner_cone_angle;
                d.outer_angle = outer_cone_angle;
                d.range = range;
                d.radius = 0.05;
                d.intensity = intensity / 100.0;
            }
            Kind::Point => {
                let mut point = obj.add_component::<PointLightComponent>();
                let d = point.data_mut(true);
                d.color = color;
                d.range = range;
                d.radius = 0.05;
                d.intensity = intensity;
            }
            Kind::Directional => {
                let mut sun = obj.add_component::<SunLightComponent>();
                let d = sun.data_mut(true);
                d.color = color;
                d.range = range;
                d.radius = 0.05;
                d.intensity = intensity;
            }
        }
    }
}
