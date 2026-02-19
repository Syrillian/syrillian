use syrillian::assets::{HMaterial, HTexture2D, MaterialInstance};
use syrillian::math::Vec3;
use syrillian_asset::PrefabMaterial;

pub struct PrefabMaterialInstantiation;

impl PrefabMaterialInstantiation {
    pub fn instantiate<F>(material: &PrefabMaterial, mut resolve_texture: F) -> MaterialInstance
    where
        F: FnMut(&str) -> Option<HTexture2D>,
    {
        let diffuse_texture = material
            .base_color_texture
            .as_deref()
            .and_then(&mut resolve_texture);
        let normal_texture = material
            .normal_texture
            .as_deref()
            .and_then(&mut resolve_texture);
        let roughness_texture = material
            .metallic_roughness_texture
            .as_deref()
            .and_then(&mut resolve_texture);

        let mut builder = MaterialInstance::builder()
            .name(material.name.clone())
            .material(HMaterial::DEFAULT)
            .diffuse(Vec3::new(
                material.base_color.x,
                material.base_color.y,
                material.base_color.z,
            ))
            .alpha(material.base_color.w)
            .metallic(material.metallic)
            .roughness(material.roughness)
            .diffuse_texture(diffuse_texture)
            .normal_texture(normal_texture)
            .roughness_texture(roughness_texture)
            .lit(!material.unlit);

        let has_transparency =
            material.alpha_mode.eq_ignore_ascii_case("Blend") || material.base_color.w < 1.0;
        builder = builder.has_transparency(has_transparency);

        builder.build()
    }
}
