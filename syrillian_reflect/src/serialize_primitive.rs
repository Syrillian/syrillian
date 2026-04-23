use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    String(String),
    Float(f32),
    Double(f64),
    UInt(u32),
    Int(i32),
    BigUInt(u64),
    BigInt(i64),
    VeryBigUInt(u128),
    VeryBigInt(i128),
    Bool(bool),
    Object(BTreeMap<String, Value>),
    Array(Vec<Value>),
}

impl Value {
    /// Extract an f64 from any numeric Value variant.
    pub fn to_f64(&self) -> Option<f64> {
        match self {
            Value::Float(v) => Some(*v as f64),
            Value::Double(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            Value::UInt(v) => Some(*v as f64),
            Value::BigInt(v) => Some(*v as f64),
            Value::BigUInt(v) => Some(*v as f64),
            Value::VeryBigInt(v) => Some(*v as f64),
            Value::VeryBigUInt(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Extract an i128 from any numeric Value variant.
    pub fn to_i128(&self) -> Option<i128> {
        match self {
            Value::Int(v) => Some(*v as i128),
            Value::UInt(v) => Some(*v as i128),
            Value::BigInt(v) => Some(*v as i128),
            Value::BigUInt(v) => Some(*v as i128),
            Value::VeryBigInt(v) => Some(*v),
            Value::VeryBigUInt(v) => Some(*v as i128),
            Value::Float(v) => Some(*v as i128),
            Value::Double(v) => Some(*v as i128),
            _ => None,
        }
    }

    /// Extract a u128 from any numeric Value variant.
    pub fn to_u128(&self) -> Option<u128> {
        match self {
            Value::UInt(v) => Some(*v as u128),
            Value::BigUInt(v) => Some(*v as u128),
            Value::VeryBigUInt(v) => Some(*v),
            Value::Int(v) => Some(*v as u128),
            Value::BigInt(v) => Some(*v as u128),
            Value::VeryBigInt(v) => Some(*v as u128),
            Value::Float(v) => Some(*v as u128),
            Value::Double(v) => Some(*v as u128),
            _ => None,
        }
    }
}
