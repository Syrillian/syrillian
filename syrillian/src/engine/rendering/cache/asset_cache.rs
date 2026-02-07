//! A cache of hot GPU Runtime Data, uploaded from the [`AssetStore`]
//!
//! For more information please see module level documentation.

use crate::engine::assets::MaterialInputLayout;
use crate::engine::assets::*;
use crate::engine::rendering::State;
use crate::engine::rendering::cache::generic_cache::Cache;
use crate::rendering::cache::GpuTexture;
use crate::rendering::{FontAtlas, RuntimeMaterial, RuntimeMesh, RuntimeShader};
use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use web_time::Instant;
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, Device};

pub struct AssetCache {
    pub meshes: Cache<Mesh>,
    pub shaders: Cache<Shader>,
    pub textures: Cache<Texture2D>,
    pub texture_arrays: Cache<Texture2DArray>,
    pub cubemaps: Cache<Cubemap>,
    pub render_textures: Cache<RenderTexture2D>,
    pub render_texture_arrays: Cache<RenderTexture2DArray>,
    pub render_cubemaps: Cache<RenderCubemap>,
    pub material_instances: Cache<MaterialInstance>,
    pub bgls: Cache<BGL>,
    pub fonts: Cache<Font>,

    store: Arc<AssetStore>,
    device: Device,

    last_refresh: Mutex<Instant>,
    material_layouts: DashMap<u64, BindGroupLayout>,
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
            cubemaps: Cache::new(store.cubemaps.clone(), device.clone(), queue.clone()),
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
            render_cubemaps: Cache::new(
                store.render_cubemaps.clone(),
                device.clone(),
                queue.clone(),
            ),
            material_instances: Cache::new(
                store.material_instances.clone(),
                device.clone(),
                queue.clone(),
            ),
            bgls: Cache::new(store.bgls.clone(), device.clone(), queue.clone()),
            fonts: Cache::new(store.fonts.clone(), device.clone(), queue.clone()),
            store,
            device: device.clone(),
            last_refresh: Mutex::new(Instant::now()),
            material_layouts: DashMap::new(),
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

    pub fn cubemap(&self, handle: HCubemap) -> Option<Arc<GpuTexture>> {
        self.cubemaps.try_get(handle, self)
    }

    pub fn render_texture(&self, handle: HRenderTexture2D) -> Option<Arc<GpuTexture>> {
        self.render_textures.try_get(handle, self)
    }

    pub fn render_texture_array(&self, handle: HRenderTexture2DArray) -> Option<Arc<GpuTexture>> {
        self.render_texture_arrays.try_get(handle, self)
    }

    pub fn render_cubemap(&self, handle: HRenderCubemap) -> Option<Arc<GpuTexture>> {
        self.render_cubemaps.try_get(handle, self)
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

    pub fn material_instance(&self, handle: HMaterialInstance) -> Arc<RuntimeMaterial> {
        self.material_instances.get(handle, self)
    }

    pub fn bgl(&self, handle: HBGL) -> Option<BindGroupLayout> {
        self.bgls.try_get(handle, self)
    }

    pub fn bgl_empty(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::EMPTY, self)
            .expect("Light is a default layout")
    }

    pub fn bgl_model(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::MODEL, self)
            .expect("Model is a default layout")
    }

    pub fn bgl_render(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::RENDER, self)
            .expect("Render is a default layout")
    }

    pub fn bgl_light(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::LIGHT, self)
            .expect("Light is a default layout")
    }

    pub fn bgl_shadow(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::SHADOW, self)
            .expect("Shadow is a default layout")
    }

    pub fn bgl_material(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::MATERIAL, self)
            .expect("Material is a default layout")
    }

    pub fn bgl_post_process(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::POST_PROCESS, self)
            .expect("Post Process is a default layout")
    }

    pub fn material_layout(&self, layout: &MaterialInputLayout) -> BindGroupLayout {
        let key = layout.layout_key();
        if let Some(existing) = self.material_layouts.get(&key) {
            return existing.clone();
        }

        let entries = layout.bgl_entries();
        let bgl = self
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Material Dynamic Bind Group Layout"),
                entries: &entries,
            });
        self.material_layouts.insert(key, bgl.clone());
        bgl
    }

    pub fn font(&self, handle: HFont) -> Arc<FontAtlas> {
        self.fonts.get(handle, self)
    }

    pub fn refresh_all(&self) -> usize {
        let mut refreshed_count = 0;

        refreshed_count += self.meshes.refresh_dirty();
        refreshed_count += self.shaders.refresh_dirty();
        refreshed_count += self.material_instances.refresh_dirty();
        refreshed_count += self.textures.refresh_dirty();
        refreshed_count += self.texture_arrays.refresh_dirty();
        refreshed_count += self.cubemaps.refresh_dirty();
        refreshed_count += self.render_textures.refresh_dirty();
        refreshed_count += self.render_texture_arrays.refresh_dirty();
        refreshed_count += self.render_cubemaps.refresh_dirty();
        refreshed_count += self.bgls.refresh_dirty();

        *self.last_refresh.lock() = Instant::now();

        refreshed_count
    }

    pub fn last_refresh(&self) -> Instant {
        *self.last_refresh.lock()
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

impl AsRef<Store<MaterialInstance>> for AssetCache {
    fn as_ref(&self) -> &Store<MaterialInstance> {
        self.material_instances.store()
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

impl AsRef<Store<Cubemap>> for AssetCache {
    fn as_ref(&self) -> &Store<Cubemap> {
        &self.store.cubemaps
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

impl AsRef<Store<RenderCubemap>> for AssetCache {
    fn as_ref(&self) -> &Store<RenderCubemap> {
        &self.store.render_cubemaps
    }
}

impl AsRef<Store<BGL>> for AssetCache {
    fn as_ref(&self) -> &Store<BGL> {
        self.bgls.store()
    }
}
