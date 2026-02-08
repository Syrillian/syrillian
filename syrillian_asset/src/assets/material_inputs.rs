use crate::HTexture2D;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use syrillian_shadergen::value::{MaterialValue, MaterialValueType};
use wgpu::{
    BindGroupLayoutEntry, BindingType, SamplerBindingType, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

#[derive(Debug, Clone)]
pub struct MaterialImmediateDef {
    pub name: String,
    pub ty: MaterialValueType,
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
