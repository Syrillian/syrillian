//! A cache of hot GPU Runtime Data, uploaded from the [`AssetStore`]
//!
//! For more information please see module level documentation.

use crate::engine::assets::*;
use crate::engine::rendering::State;
use crate::engine::rendering::cache::generic_cache::Cache;
use crate::rendering::cache::GpuTexture;
use crate::rendering::{FontAtlas, RuntimeMaterial, RuntimeMesh, RuntimeShader};
use std::sync::{Arc, Mutex};
use web_time::Instant;
use wgpu::BindGroupLayout;

pub struct AssetCache {
    pub meshes: Cache<Mesh>,
    pub shaders: Cache<Shader>,
    pub textures: Cache<Texture2D>,
    pub texture_arrays: Cache<Texture2DArray>,
    pub render_textures: Cache<RenderTexture2D>,
    pub render_texture_arrays: Cache<RenderTexture2DArray>,
    pub materials: Cache<Material>,
    pub bgls: Cache<BGL>,
    pub fonts: Cache<Font>,

    store: Arc<AssetStore>,

    last_refresh: Mutex<Instant>,
}

impl AssetCache {
    pub fn new(store: Arc<AssetStore>, state: &State) -> Self {
        let device = &state.device;
        let queue = &state.queue;
        Self {
            meshes: Cache::new(store.meshes.clone(), device.clone(), queue.clone()),
            shaders: Cache::new(store.shaders.clone(), device.clone(), queue.clone()),
            textures: Cache::new(store.textures.clone(), device.clone(), queue.clone()),
            texture_arrays: Cache::new(store.texture_arrays.clone(), device.clone(), queue.clone()),
            render_textures: Cache::new(
                store.render_textures.clone(),
                device.clone(),
                queue.clone(),
            ),
            render_texture_arrays: Cache::new(
                store.render_texture_arrays.clone(),
                device.clone(),
                queue.clone(),
            ),
            materials: Cache::new(store.materials.clone(), device.clone(), queue.clone()),
            bgls: Cache::new(store.bgls.clone(), device.clone(), queue.clone()),
            fonts: Cache::new(store.fonts.clone(), device.clone(), queue.clone()),
            store,
            last_refresh: Mutex::new(Instant::now()),
        }
    }

    pub fn store(&self) -> &AssetStore {
        &self.store
    }

    pub fn mesh(&self, handle: HMesh) -> Option<Arc<RuntimeMesh>> {
        self.meshes.try_get(handle, self)
    }

    pub fn mesh_unit_square(&self) -> Arc<RuntimeMesh> {
        self.meshes
            .try_get(HMesh::UNIT_SQUARE, self)
            .expect("Unit square is a default mesh")
    }

    pub fn shader(&self, handle: HShader) -> Arc<RuntimeShader> {
        self.shaders.get(handle, self).clone()
    }

    pub fn shader_3d(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::DIM3, self)
    }

    pub fn shader_2d(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::DIM2, self)
    }

    pub fn shader_post_process(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::POST_PROCESS, self)
    }

    pub fn shader_post_process_ssr(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::POST_PROCESS_SSR, self)
    }

    pub fn texture(&self, handle: HTexture2D) -> Arc<GpuTexture> {
        self.textures.get(handle, self)
    }

    pub fn texture_array(&self, handle: HTexture2DArray) -> Option<Arc<GpuTexture>> {
        self.texture_arrays.try_get(handle, self)
    }

    pub fn render_texture(&self, handle: HRenderTexture2D) -> Option<Arc<GpuTexture>> {
        self.render_textures.try_get(handle, self)
    }

    pub fn render_texture_array(&self, handle: HRenderTexture2DArray) -> Option<Arc<GpuTexture>> {
        self.render_texture_arrays.try_get(handle, self)
    }

    pub fn texture_fallback(&self) -> Arc<GpuTexture> {
        self.textures.get(HTexture2D::FALLBACK_DIFFUSE, self)
    }

    pub fn texture_opt(&self, handle: Option<HTexture2D>, alt: HTexture2D) -> Arc<GpuTexture> {
        match handle {
            None => self.textures.get(alt, self),
            Some(handle) => self.textures.get(handle, self),
        }
    }

    pub fn material(&self, handle: HMaterial) -> Arc<RuntimeMaterial> {
        self.materials.get(handle, self)
    }

    pub fn bgl(&self, handle: HBGL) -> Option<Arc<BindGroupLayout>> {
        self.bgls.try_get(handle, self)
    }

    pub fn bgl_empty(&self) -> Arc<BindGroupLayout> {
        self.bgls
            .try_get(HBGL::EMPTY, self)
            .expect("Light is a default layout")
    }

    pub fn bgl_model(&self) -> Arc<BindGroupLayout> {
        self.bgls
            .try_get(HBGL::MODEL, self)
            .expect("Model is a default layout")
    }

    pub fn bgl_render(&self) -> Arc<BindGroupLayout> {
        self.bgls
            .try_get(HBGL::RENDER, self)
            .expect("Render is a default layout")
    }

    pub fn bgl_light(&self) -> Arc<BindGroupLayout> {
        self.bgls
            .try_get(HBGL::LIGHT, self)
            .expect("Light is a default layout")
    }

    pub fn bgl_shadow(&self) -> Arc<BindGroupLayout> {
        self.bgls
            .try_get(HBGL::SHADOW, self)
            .expect("Shadow is a default layout")
    }

    pub fn bgl_material(&self) -> Arc<BindGroupLayout> {
        self.bgls
            .try_get(HBGL::MATERIAL, self)
            .expect("Material is a default layout")
    }

    pub fn bgl_post_process(&self) -> Arc<BindGroupLayout> {
        self.bgls
            .try_get(HBGL::POST_PROCESS, self)
            .expect("Post Process is a default layout")
    }

    pub fn font(&self, handle: HFont) -> Arc<FontAtlas> {
        self.fonts.get(handle, self)
    }

    pub fn refresh_all(&self) -> usize {
        let mut refreshed_count = 0;

        refreshed_count += self.meshes.refresh_dirty();
        refreshed_count += self.shaders.refresh_dirty();
        refreshed_count += self.materials.refresh_dirty();
        refreshed_count += self.textures.refresh_dirty();
        refreshed_count += self.bgls.refresh_dirty();

        *self.last_refresh.lock().unwrap() = Instant::now();

        refreshed_count
    }

    pub fn last_refresh(&self) -> Instant {
        *self.last_refresh.lock().unwrap()
    }
}

impl AsRef<Store<Mesh>> for AssetCache {
    fn as_ref(&self) -> &Store<Mesh> {
        self.meshes.store()
    }
}

impl AsRef<Store<Shader>> for AssetCache {
    fn as_ref(&self) -> &Store<Shader> {
        self.shaders.store()
    }
}

impl AsRef<Store<Material>> for AssetCache {
    fn as_ref(&self) -> &Store<Material> {
        self.materials.store()
    }
}

impl AsRef<Store<Texture2D>> for AssetCache {
    fn as_ref(&self) -> &Store<Texture2D> {
        self.textures.store()
    }
}

impl AsRef<Store<Texture2DArray>> for AssetCache {
    fn as_ref(&self) -> &Store<Texture2DArray> {
        &self.store.texture_arrays
    }
}

impl AsRef<Store<RenderTexture2D>> for AssetCache {
    fn as_ref(&self) -> &Store<RenderTexture2D> {
        &self.store.render_textures
    }
}

impl AsRef<Store<RenderTexture2DArray>> for AssetCache {
    fn as_ref(&self) -> &Store<RenderTexture2DArray> {
        &self.store.render_texture_arrays
    }
}

impl AsRef<Store<BGL>> for AssetCache {
    fn as_ref(&self) -> &Store<BGL> {
        self.bgls.store()
    }
}
