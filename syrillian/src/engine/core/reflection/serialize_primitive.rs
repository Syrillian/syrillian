use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    String(String),
    Float(f32),
    Double(f64),
    UInt(u32),
    Int(i32),
    UBigInt(u64),
    BigInt(i64),
    UVeryBigInt(u128),
    VeryBigInt(i128),
    Bool(bool),
    Map(BTreeMap<String, Value>),
    Array(Vec<Value>),
    Serde(serde_json::Value),
}
