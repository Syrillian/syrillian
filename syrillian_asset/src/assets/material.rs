use crate::assets::{HMaterial, HShader, HTexture2D};
use crate::material_inputs::{MaterialImmediateDef, MaterialInputLayout, MaterialTextureDef};
use crate::store::{H, HandleName, Store, StoreDefaults, StoreType, StoreTypeFallback};
use crate::{MaterialShaderSet, store_add_checked};
use glamx::Vec3;
use syrillian_shadergen::generator::MeshSkinning;
use syrillian_shadergen::value::{MaterialValue, MaterialValueType};

#[derive(Debug, Clone)]
pub struct DefaultMaterial {
    name: String,
    layout: MaterialInputLayout,
}

#[derive(Debug, Clone)]
pub struct FallbackMaterial {
    name: String,
    layout: MaterialInputLayout,
}

#[derive(Debug, Clone)]
pub struct CustomMaterial {
    name: String,
    layout: MaterialInputLayout,
    shader_unskinned: MaterialShaderSet,
    shader_skinned: MaterialShaderSet,
}

#[derive(Debug, Clone)]
pub enum Material {
    Default(DefaultMaterial),
    Fallback(FallbackMaterial),
    Custom(CustomMaterial),
}

impl DefaultMaterial {
    pub fn new(name: impl Into<String>, layout: MaterialInputLayout) -> Self {
        Self {
            name: name.into(),
            layout,
        }
    }
}

impl FallbackMaterial {
    pub fn new(name: impl Into<String>, layout: MaterialInputLayout) -> Self {
        Self {
            name: name.into(),
            layout,
        }
    }
}

impl CustomMaterial {
    pub fn new(
        name: impl Into<String>,
        layout: MaterialInputLayout,
        shader_unskinned: MaterialShaderSet,
        shader_skinned: MaterialShaderSet,
    ) -> Self {
        Self {
            name: name.into(),
            layout,
            shader_unskinned,
            shader_skinned,
        }
    }

    pub fn single(
        name: impl Into<String>,
        layout: MaterialInputLayout,
        shader_set: MaterialShaderSet,
    ) -> Self {
        Self {
            name: name.into(),
            layout,
            shader_unskinned: shader_set,
            shader_skinned: shader_set,
        }
    }
}

impl Material {
    pub fn name(&self) -> &str {
        match self {
            Material::Default(m) => &m.name,
            Material::Fallback(m) => &m.name,
            Material::Custom(m) => &m.name,
        }
    }

    pub fn layout(&self) -> &MaterialInputLayout {
        match self {
            Material::Default(m) => &m.layout,
            Material::Fallback(m) => &m.layout,
            Material::Custom(m) => &m.layout,
        }
    }

    pub fn shader_set(&self, skinning: MeshSkinning) -> MaterialShaderSet {
        match self {
            Material::Default(_) => match skinning {
                MeshSkinning::Skinned => MaterialShaderSet {
                    base: HShader::DIM3_SKINNED,
                    picking: HShader::DIM3_PICKING_SKINNED,
                    shadow: HShader::DIM3_SHADOW_SKINNED,
                },
                MeshSkinning::Unskinned => MaterialShaderSet {
                    base: HShader::DIM3,
                    picking: HShader::DIM3_PICKING,
                    shadow: HShader::DIM3_SHADOW,
                },
            },
            Material::Fallback(_) => MaterialShaderSet {
                base: HShader::FALLBACK,
                picking: HShader::DIM3_PICKING,
                shadow: HShader::DIM3_SHADOW,
            },
            Material::Custom(m) => match skinning {
                MeshSkinning::Skinned => m.shader_skinned,
                MeshSkinning::Unskinned => m.shader_unskinned,
            },
        }
    }

    pub fn default_layout() -> MaterialInputLayout {
        MaterialInputLayout {
            immediates: vec![
                MaterialImmediateDef {
                    name: "diffuse".to_string(),
                    ty: MaterialValueType::Vec3,
                    default: MaterialValue::Vec3(Vec3::splat(0.7)),
                },
                MaterialImmediateDef {
                    name: "roughness".to_string(),
                    ty: MaterialValueType::F32,
                    default: MaterialValue::F32(0.5),
                },
                MaterialImmediateDef {
                    name: "metallic".to_string(),
                    ty: MaterialValueType::F32,
                    default: MaterialValue::F32(0.4),
                },
                MaterialImmediateDef {
                    name: "alpha".to_string(),
                    ty: MaterialValueType::F32,
                    default: MaterialValue::F32(1.0),
                },
                MaterialImmediateDef {
                    name: "use_diffuse_texture".to_string(),
                    ty: MaterialValueType::Bool,
                    default: MaterialValue::Bool(false),
                },
                MaterialImmediateDef {
                    name: "use_normal_texture".to_string(),
                    ty: MaterialValueType::Bool,
                    default: MaterialValue::Bool(false),
                },
                MaterialImmediateDef {
                    name: "use_roughness_texture".to_string(),
                    ty: MaterialValueType::Bool,
                    default: MaterialValue::Bool(false),
                },
                MaterialImmediateDef {
                    name: "lit".to_string(),
                    ty: MaterialValueType::Bool,
                    default: MaterialValue::Bool(true),
                },
                MaterialImmediateDef {
                    name: "cast_shadows".to_string(),
                    ty: MaterialValueType::Bool,
                    default: MaterialValue::Bool(true),
                },
                MaterialImmediateDef {
                    name: "grayscale_diffuse".to_string(),
                    ty: MaterialValueType::Bool,
                    default: MaterialValue::Bool(false),
                },
            ],
            textures: vec![
                MaterialTextureDef {
                    name: "diffuse".to_string(),
                    default: HTexture2D::FALLBACK_DIFFUSE,
                },
                MaterialTextureDef {
                    name: "normal".to_string(),
                    default: HTexture2D::FALLBACK_NORMAL,
                },
                MaterialTextureDef {
                    name: "roughness".to_string(),
                    default: HTexture2D::FALLBACK_ROUGHNESS,
                },
            ],
        }
    }
}

impl StoreDefaults for Material {
    fn populate(store: &mut Store<Self>) {
        let layout = Material::default_layout();
        let fallback =
            Material::Fallback(FallbackMaterial::new("Fallback Material", layout.clone()));
        store_add_checked!(store, HMaterial::FALLBACK_ID, fallback);

        let default = Material::Default(DefaultMaterial::new("Default Material", layout));
        store_add_checked!(store, HMaterial::DEFAULT_ID, default);
    }
}

impl H<Material> {
    pub const FALLBACK_ID: u32 = 0;
    pub const DEFAULT_ID: u32 = 1;
    pub const MAX_BUILTIN_ID: u32 = 1;

    pub const FALLBACK: H<Material> = H::new(Self::FALLBACK_ID);
    pub const DEFAULT: H<Material> = H::new(Self::DEFAULT_ID);
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
