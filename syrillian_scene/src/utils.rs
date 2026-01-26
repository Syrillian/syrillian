use serde_json::Value as JsonValue;
use syrillian::core::reflection::Value;

pub fn json_to_syrillian_value(json: JsonValue) -> Value {
    match json {
        JsonValue::Null => Value::None,
        JsonValue::Bool(bool) => Value::Bool(bool),

        JsonValue::Number(a) if a.is_i64() => Value::BigInt(a.as_i64().unwrap()),
        JsonValue::Number(a) if a.is_u64() => Value::BigUInt(a.as_u64().unwrap()),
        JsonValue::Number(a) => Value::Double(a.as_f64().unwrap()), // fallback. this does fit all other cases.

        JsonValue::String(s) => Value::String(s),

        JsonValue::Array(a) => Value::Array(a.into_iter().map(json_to_syrillian_value).collect()),

        JsonValue::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, v)| (k, json_to_syrillian_value(v)))
                .collect(),
        ),
    }
}
