use glamx::{Vec2, Vec3, Vec4};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum MaterialValueType {
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

#[derive(Clone, Copy, Debug)]
pub enum MaterialBindingType {
    Texture2D,
}

impl MaterialValue {
    pub fn ty(&self) -> MaterialValueType {
        match self {
            MaterialValue::F32(_) => MaterialValueType::F32,
            MaterialValue::U32(_) => MaterialValueType::U32,
            MaterialValue::Bool(_) => MaterialValueType::Bool,
            MaterialValue::Vec2(_) => MaterialValueType::Vec2,
            MaterialValue::Vec3(_) => MaterialValueType::Vec3,
            MaterialValue::Vec4(_) => MaterialValueType::Vec4,
        }
    }

    pub fn bytes(&self) -> &[u8] {
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

impl MaterialValueType {
    pub fn align(self) -> usize {
        match self {
            MaterialValueType::F32 | MaterialValueType::U32 | MaterialValueType::Bool => 4,
            MaterialValueType::Vec2 => 8,
            MaterialValueType::Vec3 | MaterialValueType::Vec4 => 16,
        }
    }

    pub fn size(self) -> usize {
        match self {
            MaterialValueType::F32 | MaterialValueType::U32 | MaterialValueType::Bool => 4,
            MaterialValueType::Vec2 => 8,
            MaterialValueType::Vec3 => 12,
            MaterialValueType::Vec4 => 16,
        }
    }

    pub fn wgsl_type(self) -> &'static str {
        match self {
            MaterialValueType::F32 => "f32",
            MaterialValueType::U32 => "u32",
            MaterialValueType::Bool => "u32",
            MaterialValueType::Vec2 => "vec2<f32>",
            MaterialValueType::Vec3 => "vec3<f32>",
            MaterialValueType::Vec4 => "vec4<f32>",
        }
    }
}
