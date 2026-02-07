//! The [`AssetStore`] is used to store "raw" data, like meshes, images (textures) etc.
//!
//! It exists to cleanly differentiate between GPU state and plain-old-data.
//! You can safely add stuff to any store as you wish and then request to use it
//! when rendering. The [`AssetCache`](crate::rendering::AssetCache) is the other side of this component
//! which you will interact with to retrieve the instantiated hot GPU data.
//!
//! See module level documentation for more info.

use crate::engine::assets::*;
use std::sync::Arc;

pub struct AssetStore {
    pub meshes: Arc<Store<Mesh>>,
    pub shaders: Arc<Store<Shader>>,
    pub textures: Arc<Store<Texture2D>>,
    pub texture_arrays: Arc<Store<Texture2DArray>>,
    pub cubemaps: Arc<Store<Cubemap>>,
    pub render_textures: Arc<Store<RenderTexture2D>>,
    pub render_texture_arrays: Arc<Store<RenderTexture2DArray>>,
    pub render_cubemaps: Arc<Store<RenderCubemap>>,
    pub materials: Arc<Store<Material>>,
    pub material_instances: Arc<Store<MaterialInstance>>,
    pub bgls: Arc<Store<BGL>>,
    pub fonts: Arc<Store<Font>>,
    pub sounds: Arc<Store<Sound>>,
}

impl AssetStore {
    pub fn new() -> Arc<AssetStore> {
        Arc::new(AssetStore {
            meshes: Arc::new(Store::populated()),
            shaders: Arc::new(Store::populated()),
            textures: Arc::new(Store::populated()),
            texture_arrays: Arc::new(Store::empty()),
            cubemaps: Arc::new(Store::empty()),
            render_textures: Arc::new(Store::empty()),
            render_texture_arrays: Arc::new(Store::empty()),
            render_cubemaps: Arc::new(Store::empty()),
            materials: Arc::new(Store::populated()),
            material_instances: Arc::new(Store::populated()),
            bgls: Arc::new(Store::populated()),
            fonts: Arc::new(Store::populated()),
            sounds: Arc::new(Store::empty()),
        })
    }
}

impl AsRef<Store<Mesh>> for AssetStore {
    fn as_ref(&self) -> &Store<Mesh> {
        &self.meshes
    }
}

impl AsRef<Store<Shader>> for AssetStore {
    fn as_ref(&self) -> &Store<Shader> {
        &self.shaders
    }
}

impl AsRef<Store<Texture2D>> for AssetStore {
    fn as_ref(&self) -> &Store<Texture2D> {
        &self.textures
    }
}
