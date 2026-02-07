use crate::engine::assets::generic_store::{HandleName, Store, StoreDefaults, StoreType};
use crate::engine::assets::{
    H, HMaterial, HMaterialInstance, HTexture2D, MaterialInputLayout, MaterialValue,
    StoreTypeFallback,
};
use crate::math::Vec3;
use crate::store_add_checked;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MaterialInstance {
    pub name: String,
    pub material: HMaterial,
    pub values: HashMap<String, MaterialValue>,
    pub textures: HashMap<String, Option<HTexture2D>>,
}

impl MaterialInstance {
    pub fn builder() -> MaterialInstanceBuilder {
        MaterialInstanceBuilder::default()
    }

    pub fn value(&self, name: &str) -> Option<&MaterialValue> {
        self.values.get(name)
    }

    pub fn texture(&self, name: &str) -> Option<Option<HTexture2D>> {
        self.textures.get(name).copied()
    }

    pub fn apply_defaults(&mut self, layout: &MaterialInputLayout) {
        for field in &layout.immediates {
            self.values
                .entry(field.name.clone())
                .or_insert_with(|| field.default.clone());
        }
    }

    pub fn value_u32(&self, name: &str) -> Option<u32> {
        match self.value(name) {
            Some(MaterialValue::U32(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn value_f32(&self, name: &str) -> Option<f32> {
        match self.value(name) {
            Some(MaterialValue::F32(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn value_bool(&self, name: &str) -> Option<bool> {
        match self.value(name) {
            Some(MaterialValue::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn set_bool(&mut self, name: &str, value: bool, layout: &MaterialInputLayout) {
        if layout.immediates.iter().any(|field| field.name == name) {
            self.values
                .insert(name.to_string(), MaterialValue::Bool(value));
        }
    }
}

#[derive(Debug)]
pub struct MaterialInstanceBuilder {
    name: Option<String>,
    material: HMaterial,
    values: HashMap<String, MaterialValue>,
    textures: HashMap<String, Option<HTexture2D>>,
}

impl Default for MaterialInstanceBuilder {
    fn default() -> Self {
        Self {
            name: None,
            material: HMaterial::DEFAULT,
            values: HashMap::default(),
            textures: HashMap::default(),
        }
    }
}

impl MaterialInstanceBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn material(mut self, material: HMaterial) -> Self {
        self.material = material;
        self
    }

    pub fn values(mut self, values: HashMap<String, MaterialValue>) -> Self {
        self.values = values;
        self
    }

    pub fn textures(mut self, textures: HashMap<String, Option<HTexture2D>>) -> Self {
        self.textures = textures;
        self
    }

    pub fn value(mut self, name: impl Into<String>, value: MaterialValue) -> Self {
        self.values.insert(name.into(), value);
        self
    }

    pub fn texture(
        mut self,
        name: impl Into<String>,
        texture: impl Into<Option<HTexture2D>>,
    ) -> Self {
        self.textures.insert(name.into(), texture.into());
        self
    }

    pub fn diffuse(mut self, color: Vec3) -> Self {
        self.values
            .insert("diffuse".to_string(), MaterialValue::Vec3(color));
        self
    }

    pub fn color(self, color: Vec3) -> Self {
        self.diffuse(color)
    }

    pub fn roughness(mut self, roughness: f32) -> Self {
        self.values
            .insert("roughness".to_string(), MaterialValue::F32(roughness));
        self
    }

    pub fn metallic(mut self, metallic: f32) -> Self {
        self.values
            .insert("metallic".to_string(), MaterialValue::F32(metallic));
        self
    }

    pub fn alpha(mut self, alpha: f32) -> Self {
        self.values
            .insert("alpha".to_string(), MaterialValue::F32(alpha));
        self
    }

    pub fn lit(mut self, lit: bool) -> Self {
        self.values
            .insert("lit".to_string(), MaterialValue::Bool(lit));
        self
    }

    pub fn cast_shadows(mut self, cast_shadows: bool) -> Self {
        self.values.insert(
            "cast_shadows".to_string(),
            MaterialValue::Bool(cast_shadows),
        );
        self
    }

    pub fn has_transparency(mut self, has_transparency: bool) -> Self {
        self.values.insert(
            "has_transparency".to_string(),
            MaterialValue::Bool(has_transparency),
        );
        self
    }

    pub fn diffuse_texture(self, texture: impl Into<Option<HTexture2D>>) -> Self {
        self.texture("diffuse", texture)
    }

    pub fn normal_texture(self, texture: impl Into<Option<HTexture2D>>) -> Self {
        self.texture("normal", texture)
    }

    pub fn roughness_texture(self, texture: impl Into<Option<HTexture2D>>) -> Self {
        self.texture("roughness", texture)
    }

    pub fn build(self) -> MaterialInstance {
        MaterialInstance {
            name: self.name.unwrap_or_else(|| "Material Instance".to_string()),
            material: self.material,
            values: self.values,
            textures: self.textures,
        }
    }

    pub fn store<A: AsRef<Store<MaterialInstance>>>(self, store: &A) -> HMaterialInstance {
        store.as_ref().add(self.build())
    }
}

impl H<MaterialInstance> {
    pub const FALLBACK_ID: u32 = 0;
    pub const DEFAULT_ID: u32 = 1;
    pub const MAX_BUILTIN_ID: u32 = 1;

    pub const FALLBACK: H<MaterialInstance> = H::new(Self::FALLBACK_ID);
    pub const DEFAULT: H<MaterialInstance> = H::new(Self::DEFAULT_ID);
}

impl StoreDefaults for MaterialInstance {
    fn populate(store: &mut Store<Self>) {
        let fallback = MaterialInstance {
            name: "Fallback Material Instance".to_string(),
            material: HMaterial::FALLBACK,
            values: HashMap::new(),
            textures: HashMap::new(),
        };
        store_add_checked!(store, H::<MaterialInstance>::FALLBACK_ID, fallback);

        let default = MaterialInstance {
            name: "Default Material Instance".to_string(),
            material: HMaterial::DEFAULT,
            values: HashMap::new(),
            textures: HashMap::new(),
        };
        store_add_checked!(store, H::<MaterialInstance>::DEFAULT_ID, default);
    }
}

impl StoreType for MaterialInstance {
    fn name() -> &'static str {
        "MaterialInstance"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HMaterialInstance::FALLBACK_ID => HandleName::Static("Fallback Material Instance"),
            HMaterialInstance::DEFAULT_ID => HandleName::Static("Default Material Instance"),
            _ => HandleName::Id(handle),
        }
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<MaterialInstance>::MAX_BUILTIN_ID
    }
}

impl StoreTypeFallback for MaterialInstance {
    fn fallback() -> H<Self> {
        HMaterialInstance::FALLBACK
    }
}
