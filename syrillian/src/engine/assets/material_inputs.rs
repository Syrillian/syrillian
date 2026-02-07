use crate::engine::assets::HTexture2D;
use crate::math::{Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use wgpu::{
    BindGroupLayoutEntry, BindingType, SamplerBindingType, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum MaterialInputType {
    F32,
    U32,
    Bool,
    Vec2,
    Vec3,
    Vec4,
}

#[derive(Debug, Clone)]
pub enum MaterialValue {
    F32(f32),
    U32(u32),
    Bool(bool),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
}

impl MaterialValue {
    pub fn ty(&self) -> MaterialInputType {
        match self {
            MaterialValue::F32(_) => MaterialInputType::F32,
            MaterialValue::U32(_) => MaterialInputType::U32,
            MaterialValue::Bool(_) => MaterialInputType::Bool,
            MaterialValue::Vec2(_) => MaterialInputType::Vec2,
            MaterialValue::Vec3(_) => MaterialInputType::Vec3,
            MaterialValue::Vec4(_) => MaterialInputType::Vec4,
        }
    }

    fn bytes(&self) -> &[u8] {
        match self {
            MaterialValue::F32(v) => bytemuck::bytes_of(v),
            MaterialValue::U32(v) => bytemuck::bytes_of(v),
            MaterialValue::Bool(true) => bytemuck::bytes_of(&1u32),
            MaterialValue::Bool(false) => bytemuck::bytes_of(&0u32),
            MaterialValue::Vec2(v) => bytemuck::bytes_of(v),
            MaterialValue::Vec3(v) => bytemuck::bytes_of(v),
            MaterialValue::Vec4(v) => bytemuck::bytes_of(v),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaterialImmediateDef {
    pub name: String,
    pub ty: MaterialInputType,
    pub default: MaterialValue,
}

#[derive(Debug, Clone)]
pub struct MaterialTextureDef {
    pub name: String,
    pub default: HTexture2D,
}

#[derive(Debug, Clone)]
pub struct MaterialInputLayout {
    pub immediates: Vec<MaterialImmediateDef>,
    pub textures: Vec<MaterialTextureDef>,
}

impl MaterialInputType {
    pub fn align(self) -> usize {
        match self {
            MaterialInputType::F32 | MaterialInputType::U32 | MaterialInputType::Bool => 4,
            MaterialInputType::Vec2 => 8,
            MaterialInputType::Vec3 | MaterialInputType::Vec4 => 16,
        }
    }

    pub fn size(self) -> usize {
        match self {
            MaterialInputType::F32 | MaterialInputType::U32 | MaterialInputType::Bool => 4,
            MaterialInputType::Vec2 => 8,
            MaterialInputType::Vec3 => 12,
            MaterialInputType::Vec4 => 16,
        }
    }

    pub fn wgsl_type(self) -> &'static str {
        match self {
            MaterialInputType::F32 => "f32",
            MaterialInputType::U32 => "u32",
            MaterialInputType::Bool => "u32",
            MaterialInputType::Vec2 => "vec2<f32>",
            MaterialInputType::Vec3 => "vec3<f32>",
            MaterialInputType::Vec4 => "vec4<f32>",
        }
    }
}

fn align_to(offset: usize, align: usize) -> usize {
    if align == 0 {
        return offset;
    }
    (offset + align - 1) & !(align - 1)
}

impl MaterialInputLayout {
    pub fn immediate(&self, name: &str) -> Option<&MaterialImmediateDef> {
        self.immediates.iter().find(|field| field.name == name)
    }

    pub fn texture(&self, name: &str) -> Option<&MaterialTextureDef> {
        self.textures.iter().find(|tex| tex.name == name)
    }

    pub fn default_value(&self, name: &str) -> Option<&MaterialValue> {
        self.immediate(name).map(|field| &field.default)
    }

    pub fn texture_fallback(&self, name: &str) -> Option<HTexture2D> {
        self.texture(name).map(|tex| tex.default)
    }
    pub fn immediate_size(&self) -> u32 {
        let mut offset = 0usize;
        for field in &self.immediates {
            offset = align_to(offset, wgpu::IMMEDIATE_DATA_ALIGNMENT as usize);
            offset += field.ty.size();
        }
        let size = align_to(offset, wgpu::IMMEDIATE_DATA_ALIGNMENT as usize);
        size as u32
    }

    pub fn pack_immediates(&self, values: &HashMap<String, MaterialValue>) -> Vec<u8> {
        let size = self.immediate_size() as usize;
        let mut data = vec![0u8; size];
        let mut offset = 0usize;

        for field in &self.immediates {
            offset = align_to(offset, wgpu::IMMEDIATE_DATA_ALIGNMENT as usize);

            let value = values.get(&field.name).unwrap_or(&field.default);
            debug_assert_eq!(
                value.ty(),
                field.ty,
                "Material value {:?} has wrong type for field {}",
                value.ty(),
                field.name
            );
            let bytes = value.bytes();
            let end = offset + field.ty.size().min(bytes.len());
            data[offset..end].copy_from_slice(&bytes[..(end - offset)]);
            offset += field.ty.size();
        }

        data
    }

    pub fn wgsl_material_group(&self) -> String {
        let mut out = String::new();
        out.push_str("struct Material {\n");
        for field in &self.immediates {
            out.push_str("    ");
            out.push_str(&field.name);
            out.push_str(": ");
            out.push_str(field.ty.wgsl_type());
            out.push_str(",\n");
        }
        out.push_str("};\n");
        out.push_str("var<immediate> material: Material;\n");
        out
    }

    pub fn wgsl_material_textures_group(&self) -> String {
        let mut out = String::new();
        let mut binding = 0u32;
        for tex in &self.textures {
            out.push_str("@group(2) @binding(");
            out.push_str(&binding.to_string());
            out.push_str(") var t_");
            out.push_str(&tex.name);
            out.push_str(": texture_2d<f32>;\n");
            binding += 1;

            out.push_str("@group(2) @binding(");
            out.push_str(&binding.to_string());
            out.push_str(") var s_");
            out.push_str(&tex.name);
            out.push_str(": sampler;\n");
            binding += 1;
        }
        out
    }

    pub fn bgl_entries(&self) -> Vec<BindGroupLayoutEntry> {
        let mut entries = Vec::new();
        let mut binding = 0u32;
        for _tex in &self.textures {
            entries.push(BindGroupLayoutEntry {
                binding,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            binding += 1;
            entries.push(BindGroupLayoutEntry {
                binding,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            });
            binding += 1;
        }
        entries
    }

    pub fn layout_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.textures.len().hash(&mut hasher);
        for tex in &self.textures {
            tex.name.hash(&mut hasher);
        }
        hasher.finish()
    }
}
