use crate::engine::assets::generic_store::{HandleName, Store, StoreDefaults, StoreType};
use crate::engine::assets::*;
use crate::math::Vec3;
use crate::store_add_checked;
use bon::Builder;

#[derive(Debug, Clone, Builder)]
pub struct Material {
    #[builder(into)]
    pub name: String,
    #[builder(default = Vec3::splat(0.7))]
    pub color: Vec3,
    pub diffuse_texture: Option<HTexture2D>,
    pub normal_texture: Option<HTexture2D>,
    pub roughness_texture: Option<HTexture2D>,
    #[builder(default = 0.5)]
    pub roughness: f32,
    #[builder(default = 0.0)]
    pub metallic: f32,
    #[builder(default = 1.0)]
    pub alpha: f32,
    #[builder(default = true)]
    pub lit: bool,
    #[builder(default = true)]
    pub cast_shadows: bool,
    #[builder(default = false)]
    pub has_transparency: bool,
    #[builder(default = HShader::DIM3)]
    pub shader: HShader,
}

impl Material {
    pub fn is_transparent(&self) -> bool {
        self.alpha < 1.0
    }
}

impl<S: material_builder::State> MaterialBuilder<S>
where
    S: material_builder::IsComplete,
{
    pub fn store<A: AsRef<Store<Material>>>(self, store: &A) -> HMaterial {
        store.as_ref().add(self.build())
    }
}

impl StoreDefaults for Material {
    fn populate(store: &mut Store<Self>) {
        let fallback = Material {
            name: "Fallback Material".to_string(),
            color: Vec3::splat(1.0),
            diffuse_texture: None,
            normal_texture: None,
            roughness_texture: None,
            roughness: 0.5,
            metallic: 0.0,
            shader: HShader::FALLBACK,
            alpha: 1.0,
            lit: true,
            cast_shadows: true,
            has_transparency: false,
        };

        store_add_checked!(store, HMaterial::FALLBACK_ID, fallback);

        let default = Material {
            name: "Default Material".to_string(),
            color: Vec3::splat(0.7),
            diffuse_texture: None,
            normal_texture: None,
            roughness_texture: None,
            roughness: 0.5,
            metallic: 0.4,
            shader: HShader::DIM3,
            alpha: 1.0,
            lit: true,
            cast_shadows: true,
            has_transparency: false,
        };

        store_add_checked!(store, HMaterial::DEFAULT_ID, default);
    }
}

impl HMaterial {
    const FALLBACK_ID: u32 = 0;
    const DEFAULT_ID: u32 = 1;
    const MAX_BUILTIN_ID: u32 = 1;

    pub const FALLBACK: HMaterial = HMaterial::new(Self::FALLBACK_ID);
    pub const DEFAULT: HMaterial = HMaterial::new(Self::DEFAULT_ID);
}

impl StoreType for Material {
    fn name() -> &'static str {
        "Material"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HMaterial::FALLBACK_ID => HandleName::Static("Fallback Material"),
            HMaterial::DEFAULT_ID => HandleName::Static("Default Material"),
            _ => HandleName::Id(handle),
        }
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<Self>::MAX_BUILTIN_ID
    }
}

impl StoreTypeFallback for Material {
    fn fallback() -> H<Self> {
        HMaterial::FALLBACK
    }
}
