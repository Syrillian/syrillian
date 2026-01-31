use crate::assets::HShader;
use crate::engine::assets::{HTexture2D, Material};
use crate::engine::rendering::cache::{AssetCache, CacheType};
use crate::engine::rendering::uniform::ShaderUniform;
use crate::ensure_aligned;
use crate::math::Vec3;
use bitflags::bitflags;
use syrillian_macros::UniformIndex;
use wgpu::{Device, Queue, TextureFormat};

#[repr(u8)]
#[derive(Debug, Copy, Clone, UniformIndex)]
pub(crate) enum MaterialUniformIndex {
    Material = 0,
    DiffuseView = 1,
    DiffuseSampler = 2,
    NormalView = 3,
    NormalSampler = 4,
    RoughnessView = 5,
    RoughnessSampler = 6,
}

bitflags! {
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct MaterialParams: u32 {
        const use_diffuse_texture   = 1;
        const use_normal_texture    = 1 << 1;
        const use_roughness_texture = 1 << 2;
        const lit                   = 1 << 3;
        const cast_shadows          = 1 << 4;
        const grayscale_diffuse     = 1 << 5;
        const has_transparency      = 1 << 6;
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub diffuse: Vec3,
    pub roughness: f32,
    pub metallic: f32,
    pub alpha: f32,
    pub params: MaterialParams,
    pub _padding: u32,
}

ensure_aligned!(MaterialUniform { diffuse }, align <= 16 * 2 => size);

#[allow(dead_code)]
#[derive(Debug)]
pub struct RuntimeMaterial {
    pub(crate) data: MaterialUniform,
    pub(crate) uniform: ShaderUniform<MaterialUniformIndex>,
    pub(crate) shader: HShader,
}

#[derive(Debug)]
pub enum MaterialError {
    MaterialNotFound,
    DeviceNotInitialized,
    QueueNotInitialized,
}

impl CacheType for Material {
    type Hot = RuntimeMaterial;

    fn upload(self, device: &Device, _queue: &Queue, cache: &AssetCache) -> Self::Hot {
        let mut params = MaterialParams::empty();
        if self.diffuse_texture.is_some() {
            params |= MaterialParams::use_diffuse_texture;
        }
        if self.normal_texture.is_some() {
            params |= MaterialParams::use_normal_texture;
        }
        if self.roughness_texture.is_some() {
            params |= MaterialParams::use_roughness_texture;
        }

        let diffuse = cache.texture_opt(self.diffuse_texture, HTexture2D::FALLBACK_DIFFUSE);

        let is_grayscale = diffuse.format == TextureFormat::Rg8Unorm;
        if is_grayscale {
            params |= MaterialParams::grayscale_diffuse;
        }
        if !is_grayscale && self.lit {
            params |= MaterialParams::lit;
        }
        if !is_grayscale && self.cast_shadows {
            params |= MaterialParams::cast_shadows;
        }
        if is_grayscale || self.has_transparency || diffuse.has_transparency {
            params |= MaterialParams::has_transparency;
        }

        let data = MaterialUniform {
            diffuse: self.color,
            roughness: self.roughness,
            metallic: self.metallic,
            alpha: self.alpha,
            params,
            _padding: 0x0,
        };

        let mat_bgl = cache.bgl_material();
        let normal = cache.texture_opt(self.normal_texture, HTexture2D::FALLBACK_NORMAL);
        let roughness = cache.texture_opt(self.roughness_texture, HTexture2D::FALLBACK_ROUGHNESS);

        // TODO: Add additional material mapping properties and such
        let uniform = ShaderUniform::<MaterialUniformIndex>::builder((*mat_bgl).clone())
            .with_buffer_data(&data)
            .with_texture(diffuse.view.clone())
            .with_sampler(diffuse.sampler.clone())
            .with_texture(normal.view.clone())
            .with_sampler(normal.sampler.clone())
            .with_texture(roughness.view.clone())
            .with_sampler(roughness.sampler.clone())
            .build(device);

        RuntimeMaterial {
            data,
            uniform,
            shader: self.shader,
        }
    }
}

impl MaterialUniform {
    pub fn has_diffuse_texture(&self) -> bool {
        self.params.contains(MaterialParams::use_diffuse_texture)
    }

    pub fn has_normal_texture(&self) -> bool {
        self.params.contains(MaterialParams::use_normal_texture)
    }

    pub fn has_roughness_texture(&self) -> bool {
        self.params.contains(MaterialParams::use_roughness_texture)
    }

    pub fn is_lit(&self) -> bool {
        self.params.contains(MaterialParams::lit)
    }

    pub fn has_cast_shadows(&self) -> bool {
        self.params.contains(MaterialParams::cast_shadows)
    }

    pub fn has_transparency(&self) -> bool {
        self.params.contains(MaterialParams::has_transparency) || self.alpha < 1.0
    }
}
