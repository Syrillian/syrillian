use crate::GltfScene;
use gltf::Material;
use std::collections::HashMap;
use syrillian::math::{Vec3, Vec4};
use syrillian_asset::PrefabMaterial;

impl GltfScene {
    pub fn decode_material(
        &self,
        material: &Material,
        index: u32,
        texture_path_of: &HashMap<usize, String>,
    ) -> PrefabMaterial {
        let pbr = material.pbr_metallic_roughness();
        let base = pbr.base_color_factor();
        let emissive = material.emissive_factor();

        PrefabMaterial {
            name: material
                .name()
                .map(str::to_string)
                .unwrap_or_else(|| format!("material_{index}")),
            base_color: Vec4::from(base),
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            alpha_cutoff: material.alpha_cutoff(),
            alpha_mode: format!("{:?}", material.alpha_mode()),
            double_sided: material.double_sided(),
            unlit: material.unlit(),
            emissive_factor: Vec3::from(emissive),
            base_color_texture: pbr
                .base_color_texture()
                .and_then(|info| texture_path_of.get(&info.texture().index()).cloned()),
            normal_texture: material
                .normal_texture()
                .and_then(|info| texture_path_of.get(&info.texture().index()).cloned()),
            metallic_roughness_texture: pbr
                .metallic_roughness_texture()
                .and_then(|info| texture_path_of.get(&info.texture().index()).cloned()),
            emissive_texture: material
                .emissive_texture()
                .and_then(|info| texture_path_of.get(&info.texture().index()).cloned()),
            occlusion_texture: material
                .occlusion_texture()
                .and_then(|info| texture_path_of.get(&info.texture().index()).cloned()),
        }
    }
}
