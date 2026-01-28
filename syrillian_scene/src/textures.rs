use super::gltf_loader::GltfScene;
use gltf::image::Format;
use std::collections::HashMap;
use syrillian::World;
use syrillian::assets::{HMaterial, HShader, HTexture2D, Material, StoreType, Texture2D};
use syrillian::math::Vector3;
use syrillian::rendering::TextureFormat;
use syrillian::utils::debug_panic;

/// Loads all materials defined in the glTF scene and stores them in the asset store.
pub(super) fn load_materials(scene: &GltfScene, world: &mut World) -> HashMap<u32, HMaterial> {
    let mut map = HashMap::new();

    for (i, mat) in scene.doc.materials().enumerate() {
        let name = mat.name().unwrap_or("Material").to_string();
        let pbr = mat.pbr_metallic_roughness();

        let base = pbr.base_color_factor();
        let color = Vector3::new(base[0], base[1], base[2]);
        let alpha = base[3];
        let metallic = pbr.metallic_factor();
        let roughness = pbr.roughness_factor();

        let diffuse_texture = load_texture(scene, world, pbr.base_color_texture());
        let normal_texture = load_texture(scene, world, mat.normal_texture());
        let roughness_texture = load_texture(scene, world, pbr.metallic_roughness_texture());

        let lit = !mat.unlit();

        let material = Material {
            name,
            color,
            roughness,
            metallic,
            diffuse_texture,
            normal_texture,
            roughness_texture,
            alpha,
            lit,
            cast_shadows: true,
            shader: HShader::DIM3,
            has_transparency: false,
        };
        map.insert(i as u32, world.assets.materials.add(material));
    }

    map
}

/// Converts a glTF texture reference into an engine texture handle.
pub(super) fn load_texture<'a, T>(
    scene: &'a GltfScene,
    world: &mut World,
    info: Option<T>,
) -> Option<HTexture2D>
where
    T: AsRef<gltf::texture::Texture<'a>>,
{
    let tex = info.as_ref()?.as_ref();
    let image = tex.source();
    let index = image.index();

    let pixels = &scene.images[index].pixels;
    let mut data = Vec::new();
    let (width, height) = (scene.images[index].width, scene.images[index].height);
    let original_format = scene.images[index].format;

    let format = match original_format {
        Format::R8 => TextureFormat::R8Unorm,
        Format::R8G8 => TextureFormat::Rg8Unorm,
        Format::R8G8B8 => TextureFormat::Rgba8UnormSrgb,
        Format::R8G8B8A8 => TextureFormat::Rgba8UnormSrgb,
        Format::R16 => TextureFormat::R16Unorm,
        Format::R16G16 => TextureFormat::Rg16Snorm,
        Format::R16G16B16 => {
            debug_panic!("Cannot use RGB16 (no alpha) Texture");
            return None;
        }
        Format::R16G16B16A16 => TextureFormat::Rgba16Unorm,
        Format::R32G32B32FLOAT => {
            debug_panic!("Cannot use RGB32 (no alpha) Texture");
            return None;
        }
        Format::R32G32B32A32FLOAT => TextureFormat::Rgba32Float,
    };

    if original_format == Format::R8G8B8 {
        for rgb in pixels.chunks(3) {
            data.extend(rgb);
            data.push(255);
        }
    } else {
        data.clone_from(pixels);
    }

    debug_assert_eq!(
        data.len(),
        width as usize * height as usize * format.block_copy_size(None).unwrap() as usize
    );

    Some(Texture2D::load_pixels(data, width, height, format).store(world))
}
