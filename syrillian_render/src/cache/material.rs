use crate::cache::{AssetCache, CacheType, GpuTexture};
use std::collections::HashMap;
use std::sync::Arc;
use syrillian_asset::MaterialInstance;
use syrillian_asset::material_inputs::MaterialInputLayout;
use syrillian_asset::{HMaterial, MaterialShaderSet};
use syrillian_shadergen::generator::MeshSkinning;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, Device, Queue, TextureFormat,
};

#[derive(Debug)]
pub struct RuntimeMaterial {
    pub immediates: Vec<u8>,
    pub bind_group: BindGroup,
    pub shader_unskinned: MaterialShaderSet,
    pub shader_skinned: MaterialShaderSet,
    pub transparent: bool,
    pub cast_shadows: bool,
}

#[derive(Debug)]
pub enum MaterialError {
    MaterialNotFound,
    DeviceNotInitialized,
    QueueNotInitialized,
}

fn material_textures(
    instance: &MaterialInstance,
    layout: &MaterialInputLayout,
    cache: &AssetCache,
) -> (Vec<Arc<GpuTexture>>, HashMap<String, Arc<GpuTexture>>) {
    let mut ordered = Vec::new();
    let mut by_name = HashMap::new();

    for tex in &layout.textures {
        let handle = instance
            .textures
            .get(&tex.name)
            .and_then(|v| *v)
            .unwrap_or(tex.default);
        let gpu = cache.texture(handle);
        ordered.push(gpu.clone());
        by_name.insert(tex.name.clone(), gpu);
    }

    (ordered, by_name)
}

impl CacheType for MaterialInstance {
    type Hot = Arc<RuntimeMaterial>;

    fn upload(mut self, device: &Device, _queue: &Queue, cache: &AssetCache) -> Self::Hot {
        let material_def = cache
            .store()
            .materials
            .try_get(self.material)
            .map(|m| m.clone())
            .unwrap_or_else(|| cache.store().materials.get(HMaterial::FALLBACK).clone());

        let layout = material_def.layout().clone();
        let shader_unskinned = material_def.shader_set(MeshSkinning::Unskinned);
        let shader_skinned = material_def.shader_set(MeshSkinning::Skinned);

        let (textures, texture_map) = material_textures(&self, &layout, cache);

        for tex in &layout.textures {
            let flag = format!("use_{}_texture", tex.name);
            let use_tex = self.textures.get(&tex.name).and_then(|v| *v).is_some();
            self.set_bool(&flag, use_tex, &layout);
        }

        if let Some(tex) = texture_map.get("diffuse") {
            let grayscale = tex.format == TextureFormat::Rg8Unorm;
            self.set_bool("grayscale_diffuse", grayscale, &layout);
        }

        let cast_shadows = self.value_bool("cast_shadows").unwrap_or(true);

        let alpha = self.value_f32("alpha").unwrap_or(1.0);
        let has_transparency_flag = self.value_bool("has_transparency").unwrap_or(true);
        let diffuse_has_transparency = texture_map
            .get("diffuse")
            .is_some_and(|t| t.has_transparency);
        let transparent = alpha < 1.0 || has_transparency_flag || diffuse_has_transparency;

        let immediates = layout.pack_immediates(&self.values);

        let bgl = cache.material_layout(&layout);
        let mut entries: Vec<BindGroupEntry> = Vec::new();
        let mut binding = 0u32;
        for tex in &textures {
            entries.push(BindGroupEntry {
                binding,
                resource: BindingResource::TextureView(&tex.view),
            });
            binding += 1;
            entries.push(BindGroupEntry {
                binding,
                resource: BindingResource::Sampler(&tex.sampler),
            });
            binding += 1;
        }

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &bgl,
            entries: &entries,
        });

        Arc::new(RuntimeMaterial {
            immediates,
            bind_group,
            shader_unskinned,
            shader_skinned,
            transparent,
            cast_shadows,
        })
    }
}
