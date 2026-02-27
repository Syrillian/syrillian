use glamx::{Vec2, Vec3, Vec4};
use zerocopy::IntoBytes;

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
    const BOOL_TRUE_U32: u32 = true as u32;
    const BOOL_FALSE_U32: u32 = 0;

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
            MaterialValue::F32(v) => v.as_bytes(),
            MaterialValue::U32(v) => v.as_bytes(),
            MaterialValue::Bool(true) => Self::BOOL_TRUE_U32.as_bytes(),
            MaterialValue::Bool(false) => Self::BOOL_FALSE_U32.as_bytes(),
            MaterialValue::Vec2(v) => v.as_bytes(),
            MaterialValue::Vec3(v) => v.as_bytes(),
            MaterialValue::Vec4(v) => v.as_bytes(),
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
