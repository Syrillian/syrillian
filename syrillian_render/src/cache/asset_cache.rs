//! A cache of hot GPU Runtime Data, uploaded from the [`AssetStore`]
//!
//! For more information please see module level documentation.

use crate::cache::generic_cache::Cache;
use crate::cache::{
    FontAtlas, GpuTexture, RuntimeComputeShader, RuntimeMaterial, RuntimeMesh, RuntimeShader,
};
use crate::rendering::state::State;
use crossbeam_channel::Receiver;
use dashmap::DashMap;
use std::sync::Arc;
use syrillian_asset::material_inputs::MaterialInputLayout;
use syrillian_asset::store::{AssetKey, UpdateAssetMessage};
use syrillian_asset::*;
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, Device};

pub struct AssetCache {
    pub meshes: Cache<Mesh>,
    pub skinned_meshes: Cache<SkinnedMesh>,
    pub shaders: Cache<Shader>,
    pub compute_shaders: Cache<ComputeShader>,
    pub textures: Cache<Texture2D>,
    pub texture_arrays: Cache<Texture2DArray>,
    pub cubemaps: Cache<Cubemap>,
    pub render_textures: Cache<RenderTexture2D>,
    pub render_texture_arrays: Cache<RenderTexture2DArray>,
    pub render_cubemaps: Cache<RenderCubemap>,
    pub materials: Cache<Material>,
    pub material_instances: Cache<MaterialInstance>,
    pub bgls: Cache<BGL>,
    pub fonts: Cache<Font>,

    device: Device,
    assets_rx: Receiver<(AssetKey, UpdateAssetMessage)>,

    material_layouts: DashMap<u64, BindGroupLayout>,
}

impl AssetCache {
    pub fn new(state: &State, assets_rx: Receiver<(AssetKey, UpdateAssetMessage)>) -> Self {
        let device = &state.device;
        let queue = &state.queue;
        Self {
            meshes: Cache::new(device.clone(), queue.clone()),
            skinned_meshes: Cache::new(device.clone(), queue.clone()),
            shaders: Cache::new(device.clone(), queue.clone()),
            compute_shaders: Cache::new(device.clone(), queue.clone()),
            textures: Cache::new(device.clone(), queue.clone()),
            texture_arrays: Cache::new(device.clone(), queue.clone()),
            cubemaps: Cache::new(device.clone(), queue.clone()),
            render_textures: Cache::new(device.clone(), queue.clone()),
            render_texture_arrays: Cache::new(device.clone(), queue.clone()),
            render_cubemaps: Cache::new(device.clone(), queue.clone()),
            materials: Cache::new(device.clone(), queue.clone()),
            material_instances: Cache::new(device.clone(), queue.clone()),
            bgls: Cache::new(device.clone(), queue.clone()),
            fonts: Cache::new(device.clone(), queue.clone()),
            device: device.clone(),
            assets_rx,
            material_layouts: DashMap::new(),
        }
    }

    pub fn mesh(&self, handle: HMesh) -> Option<Arc<RuntimeMesh>> {
        self.meshes.try_get(handle)
    }

    pub fn mesh_unit_square(&self) -> Arc<RuntimeMesh> {
        self.meshes
            .try_get(HMesh::UNIT_SQUARE)
            .expect("Unit square is a default mesh")
    }

    pub fn skinned_mesh(&self, handle: HSkinnedMesh) -> Option<Arc<RuntimeMesh>> {
        self.skinned_meshes.try_get(handle)
    }

    pub fn shader(&self, handle: HShader) -> Arc<RuntimeShader> {
        self.shaders.get(handle).clone()
    }

    pub fn compute_shader(&self, handle: HComputeShader) -> Arc<RuntimeComputeShader> {
        self.compute_shaders.get(handle)
    }

    pub fn shader_3d(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::DIM3)
    }

    pub fn shader_2d(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::DIM2)
    }

    pub fn shader_post_process(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::POST_PROCESS)
    }

    pub fn shader_post_process_fxaa(&self) -> Arc<RuntimeShader> {
        self.shaders.get(HShader::POST_PROCESS_FXAA)
    }

    pub fn texture(&self, handle: HTexture2D) -> Arc<GpuTexture> {
        self.textures.get(handle)
    }

    pub fn texture_array(&self, handle: HTexture2DArray) -> Option<Arc<GpuTexture>> {
        self.texture_arrays.try_get(handle)
    }

    pub fn cubemap(&self, handle: HCubemap) -> Option<Arc<GpuTexture>> {
        self.cubemaps.try_get(handle)
    }

    pub fn cubemap_fallback(&self) -> Arc<GpuTexture> {
        self.cubemaps.get(HCubemap::FALLBACK)
    }

    pub fn render_texture(&self, handle: HRenderTexture2D) -> Option<Arc<GpuTexture>> {
        self.render_textures.try_get(handle)
    }

    pub fn render_texture_array(&self, handle: HRenderTexture2DArray) -> Option<Arc<GpuTexture>> {
        self.render_texture_arrays.try_get(handle)
    }

    pub fn render_cubemap(&self, handle: HRenderCubemap) -> Option<Arc<GpuTexture>> {
        self.render_cubemaps.try_get(handle)
    }

    pub fn texture_fallback(&self) -> Arc<GpuTexture> {
        self.textures.get(HTexture2D::FALLBACK_DIFFUSE)
    }

    pub fn texture_opt(&self, handle: Option<HTexture2D>, alt: HTexture2D) -> Arc<GpuTexture> {
        match handle {
            None => self.textures.get(alt),
            Some(handle) => self.textures.get(handle),
        }
    }

    pub fn material_instance(&self, handle: HMaterialInstance) -> Arc<RuntimeMaterial> {
        self.material_instances.get(handle)
    }

    pub fn bgl(&self, handle: HBGL) -> Option<BindGroupLayout> {
        self.bgls.try_get(handle)
    }

    pub fn bgl_empty(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::EMPTY)
            .expect("Light is a default layout")
    }

    pub fn bgl_model(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::MODEL)
            .expect("Model is a default layout")
    }

    pub fn bgl_render(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::RENDER)
            .expect("Render is a default layout")
    }

    pub fn bgl_light(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::LIGHT)
            .expect("Light is a default layout")
    }

    pub fn bgl_shadow(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::SHADOW)
            .expect("Shadow is a default layout")
    }

    pub fn bgl_material(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::MATERIAL)
            .expect("Material is a default layout")
    }

    pub fn bgl_post_process(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::POST_PROCESS)
            .expect("Post Process is a default layout")
    }

    pub fn bgl_post_process_compute(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::POST_PROCESS_COMPUTE)
            .expect("Post Process Compute is a default layout")
    }

    pub fn bgl_mesh_skinning_compute(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::MESH_SKINNING_COMPUTE)
            .expect("Mesh Skinning Compute is a default layout")
    }

    pub fn bgl_particle_compute(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::PARTICLE_COMPUTE)
            .expect("Particle Compute is a default layout")
    }

    pub fn bgl_bloom_compute(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::BLOOM_COMPUTE)
            .expect("Bloom Compute is a default layout")
    }

    pub fn bgl_ssao_compute(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::SSAO_COMPUTE)
            .expect("SSAO Compute is a default layout")
    }

    pub fn bgl_ssao_apply_compute(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::SSAO_APPLY_COMPUTE)
            .expect("SSAO Apply Compute is a default layout")
    }

    pub fn bgl_font_atlas(&self) -> BindGroupLayout {
        self.bgls
            .try_get(HBGL::FONT_ATLAS)
            .expect("Font Atlas is a default layout")
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
        self.fonts.get(handle)
    }

    pub fn refresh_dirty(&self) {
        for (key, msg) in self.assets_rx.try_iter() {
            match msg {
                UpdateAssetMessage::UpdateMesh(mesh) => self.meshes.refresh_item(key, mesh, self),
                UpdateAssetMessage::UpdateSkinnedMesh(mesh) => {
                    self.skinned_meshes.refresh_item(key, mesh, self)
                }
                UpdateAssetMessage::UpdateShader(shader) => {
                    self.shaders.refresh_item(key, shader, self)
                }
                UpdateAssetMessage::UpdateComputeShader(shader) => {
                    self.compute_shaders.refresh_item(key, shader, self)
                }
                UpdateAssetMessage::UpdateTexture2D(texture) => {
                    self.textures.refresh_item(key, texture, self)
                }
                UpdateAssetMessage::UpdateTexture2DArray(texture) => {
                    self.texture_arrays.refresh_item(key, texture, self)
                }
                UpdateAssetMessage::UpdateCubemap(texture) => {
                    self.cubemaps.refresh_item(key, texture, self)
                }
                UpdateAssetMessage::UpdateRenderTexture2D(texture) => {
                    self.render_textures.refresh_item(key, texture, self)
                }
                UpdateAssetMessage::UpdateRenderTexture2DArray(texture) => {
                    self.render_texture_arrays.refresh_item(key, texture, self)
                }
                UpdateAssetMessage::UpdateRenderCubemap(texture) => {
                    self.render_cubemaps.refresh_item(key, texture, self)
                }
                UpdateAssetMessage::UpdateMaterial(material) => {
                    self.materials.refresh_item(key, material, self)
                }
                UpdateAssetMessage::UpdateMaterialInstance(material) => {
                    self.material_instances.refresh_item(key, material, self)
                }
                UpdateAssetMessage::UpdateBGL(layout) => self.bgls.refresh_item(key, layout, self),
                UpdateAssetMessage::UpdateFont(font) => self.fonts.refresh_item(key, font, self),
            }
        }
    }
}
