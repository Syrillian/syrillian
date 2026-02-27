//! The [`AssetStore`] is used to store "raw" data, like meshes, images (textures) etc.
//!
//! It exists to cleanly differentiate between GPU state and plain-old-data.
//! You can safely add stuff to any store as you wish and then request to use it
//! when rendering. The [`AssetCache`](syrillian::engine::rendering::cache::asset_cache::AssetCache) is the other side of this component
//! which you will interact with to retrieve the instantiated hot GPU data.
//!
//! See module level documentation for more info.

use crate::assets::*;
use crate::material_inputs::MaterialInputLayout;
use crate::store::streaming::asset_store::AssetType;
use crate::store::streaming::decode::StreamingState;
use crate::store::{AssetKey, Store, StoreType};
use crossbeam_channel::{Receiver, Sender};
use std::sync::Arc;
use syrillian_shadergen::MaterialCompiler;
use syrillian_shadergen::function::MaterialExpression;
use syrillian_shadergen::generator::MaterialShaderSetCode;

pub struct AssetStore {
    pub meshes: Arc<Store<Mesh>>,
    pub skinned_meshes: Arc<Store<SkinnedMesh>>,
    pub shaders: Arc<Store<Shader>>,
    pub compute_shaders: Arc<Store<ComputeShader>>,
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
    pub animation_clips: Arc<Store<AnimationClip>>,
    pub prefab_materials: Arc<Store<PrefabMaterial>>,
    pub prefabs: Arc<Store<PrefabAsset>>,
    pub(crate) streaming: StreamingState,

    assets_tx: Sender<AssetRefreshMessage>,
}

pub enum AssetRefreshMessage {
    Updated(AssetKey, UpdateAssetMessage),
    Deleted(AssetKey, AssetType),
}

pub enum UpdateAssetMessage {
    UpdateMesh(Mesh),
    UpdateSkinnedMesh(SkinnedMesh),
    UpdateShader(Shader),
    UpdateComputeShader(ComputeShader),
    UpdateTexture2D(Texture2D),
    UpdateTexture2DArray(Texture2DArray),
    UpdateCubemap(Cubemap),
    UpdateRenderTexture2D(RenderTexture2D),
    UpdateRenderTexture2DArray(RenderTexture2DArray),
    UpdateRenderCubemap(RenderCubemap),
    UpdateMaterial(Material),
    UpdateMaterialInstance(MaterialInstance),
    UpdateBGL(BGL),
    UpdateFont(Font),
}

impl AssetStore {
    pub fn new() -> (Arc<AssetStore>, Receiver<AssetRefreshMessage>) {
        let (assets_tx, assets_rx) = crossbeam_channel::unbounded();
        let store = Arc::new(AssetStore {
            meshes: Arc::new(Store::populated()),
            skinned_meshes: Arc::new(Store::empty()),
            shaders: Arc::new(Store::populated()),
            compute_shaders: Arc::new(Store::populated()),
            textures: Arc::new(Store::populated()),
            texture_arrays: Arc::new(Store::empty()),
            cubemaps: Arc::new(Store::populated()),
            render_textures: Arc::new(Store::empty()),
            render_texture_arrays: Arc::new(Store::empty()),
            render_cubemaps: Arc::new(Store::empty()),
            materials: Arc::new(Store::populated()),
            material_instances: Arc::new(Store::populated()),
            bgls: Arc::new(Store::populated()),
            fonts: Arc::new(Store::populated()),
            sounds: Arc::new(Store::empty()),
            animation_clips: Arc::new(Store::empty()),
            prefab_materials: Arc::new(Store::empty()),
            prefabs: Arc::new(Store::empty()),
            streaming: StreamingState::new(),
            assets_tx,
        });

        store.refresh_dirty();

        (store, assets_rx)
    }

    pub fn refresh_dirty(&self) -> usize {
        let mut refreshed = 0;
        refreshed += self.bgls.refresh_dirty(&self.assets_tx);
        refreshed += self.textures.refresh_dirty(&self.assets_tx);
        refreshed += self.texture_arrays.refresh_dirty(&self.assets_tx);
        refreshed += self.cubemaps.refresh_dirty(&self.assets_tx);
        refreshed += self.render_textures.refresh_dirty(&self.assets_tx);
        refreshed += self.render_texture_arrays.refresh_dirty(&self.assets_tx);
        refreshed += self.render_cubemaps.refresh_dirty(&self.assets_tx);
        refreshed += self.shaders.refresh_dirty(&self.assets_tx);
        refreshed += self.compute_shaders.refresh_dirty(&self.assets_tx);
        refreshed += self.materials.refresh_dirty(&self.assets_tx);
        refreshed += self.material_instances.refresh_dirty(&self.assets_tx);
        refreshed += self.meshes.refresh_dirty(&self.assets_tx);
        refreshed += self.skinned_meshes.refresh_dirty(&self.assets_tx);
        refreshed += self.fonts.refresh_dirty(&self.assets_tx);
        refreshed += self.sounds.refresh_dirty(&self.assets_tx);
        refreshed += self.animation_clips.refresh_dirty(&self.assets_tx);
        refreshed += self.prefab_materials.refresh_dirty(&self.assets_tx);
        refreshed += self.prefabs.refresh_dirty(&self.assets_tx);
        refreshed
    }

    pub fn register_custom_material<M: MaterialExpression>(
        &self,
        name: impl Into<String>,
        material_expr: M,
    ) -> HMaterial {
        self.register_custom_material_with_layout(name, material_expr, Material::default_layout())
    }

    pub fn register_custom_material_with_layout<M: MaterialExpression>(
        &self,
        name: impl Into<String>,
        mut material_expr: M,
        layout: MaterialInputLayout,
    ) -> HMaterial {
        let name = name.into();

        let shader_code = MaterialCompiler::compile_shader_set(&mut material_expr);
        let shader_set = self.store_shader_set(&name, shader_code, &layout);

        let material = Material::Custom(CustomMaterial::new(name, layout, shader_set));
        self.materials.add(material)
    }

    fn store_shader_set(
        &self,
        base_name: &str,
        set: MaterialShaderSetCode,
        layout: &MaterialInputLayout,
    ) -> MaterialShaderSet {
        let groups = MaterialShaderGroups {
            material: layout.wgsl_material_group(),
            material_textures: layout.wgsl_material_textures_group(),
        };
        let imm_size = layout.immediate_size();

        let base = Shader::builder()
            .name(format!("{} (Base)", base_name))
            .shader_type(ShaderType::Custom)
            .code(ShaderCode::Full(set.base))
            .material_layout(layout.clone())
            .material_groups(groups.clone())
            .immediate_size(imm_size)
            .build()
            .store(self);

        let picking = Shader::builder()
            .name(format!("{} (Picking)", base_name))
            .shader_type(ShaderType::Custom)
            .code(ShaderCode::Full(set.picking))
            .build()
            .store(self);

        let shadow = Shader::builder()
            .name(format!("{} (Shadow)", base_name))
            .shader_type(ShaderType::Custom)
            .code(ShaderCode::Full(set.shadow))
            .material_layout(layout.clone())
            .material_groups(groups)
            .immediate_size(imm_size)
            .build()
            .store(self);

        MaterialShaderSet {
            base,
            picking,
            shadow,
        }
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

impl AsRef<Store<ComputeShader>> for AssetStore {
    fn as_ref(&self) -> &Store<ComputeShader> {
        &self.compute_shaders
    }
}

impl AsRef<Store<Texture2D>> for AssetStore {
    fn as_ref(&self) -> &Store<Texture2D> {
        &self.textures
    }
}

impl AsRef<Store<PrefabMaterial>> for AssetStore {
    fn as_ref(&self) -> &Store<PrefabMaterial> {
        &self.prefab_materials
    }
}

impl AsRef<Store<AnimationClip>> for AssetStore {
    fn as_ref(&self) -> &Store<AnimationClip> {
        &self.animation_clips
    }
}

impl AsRef<Store<PrefabAsset>> for AssetStore {
    fn as_ref(&self) -> &Store<PrefabAsset> {
        &self.prefabs
    }
}
