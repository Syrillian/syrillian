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
