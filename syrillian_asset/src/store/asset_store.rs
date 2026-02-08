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
use crate::store::{Store, StoreType};
use std::sync::Arc;
use syrillian_shadergen::MaterialCompiler;
use syrillian_shadergen::function::MaterialExpression;
use syrillian_shadergen::generator::{MaterialShaderSetCode, MeshSkinning};

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
        material_expr: M,
        layout: MaterialInputLayout,
    ) -> HMaterial {
        let name = name.into();

        let unskinned =
            MaterialCompiler::compile_shader_set(&material_expr, MeshSkinning::Unskinned);
        let skinned = MaterialCompiler::compile_shader_set(&material_expr, MeshSkinning::Skinned);

        let unskinned_set = self.store_shader_set(&name, unskinned, &layout);
        let skinned_set = self.store_shader_set(&name, skinned, &layout);

        let material = Material::Custom(CustomMaterial::new(
            name,
            layout,
            unskinned_set,
            skinned_set,
        ));
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

impl AsRef<Store<Texture2D>> for AssetStore {
    fn as_ref(&self) -> &Store<Texture2D> {
        &self.textures
    }
}
