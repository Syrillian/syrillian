use crate::{ReflectSerialize, Value};

pub struct JsonSerializer;

impl JsonSerializer {
    pub fn serialize_to_string<S: ReflectSerialize>(value: &S) -> String {
        let val = ReflectSerialize::serialize(value);
        Self::value_to_string(&val)
    }

    pub fn value_to_string(value: &Value) -> String {
        let mut json = String::new();
        Self::append_value_to_string(value, &mut json);
        json
    }

    fn append_value_to_string(value: &Value, json: &mut String) {
        match value {
            Value::String(value) => {
                Self::append_quoted_string(value, json);
            }
            Value::Float(value) => *json += &value.to_string(),
            Value::Double(value) => *json += &value.to_string(),
            Value::UInt(value) => *json += &value.to_string(),
            Value::Int(value) => *json += &value.to_string(),
            Value::BigUInt(value) => *json += &value.to_string(),
            Value::BigInt(value) => *json += &value.to_string(),
            Value::VeryBigUInt(value) => *json += &value.to_string(),
            Value::VeryBigInt(value) => *json += &value.to_string(),
            Value::Object(map) => {
                json.push('{');
                let mut first = true;
                for (key, value) in map {
                    if first {
                        first = false;
                    } else {
                        json.push(',');
                    }
                    Self::append_quoted_string(key, json);
                    json.push(':');
                    Self::append_value_to_string(value, json);
                }
                json.push('}');
            }
            Value::Array(values) => {
                json.push('[');
                let mut first = true;
                for value in values {
                    if first {
                        first = false;
                    } else {
                        json.push(',');
                    }
                    Self::append_value_to_string(value, json);
                }
                json.push(']');
            }
            Value::None => *json += "null",
            Value::Bool(true) => *json += "true",
            Value::Bool(false) => *json += "false",
        }
    }

    fn append_quoted_string(value: &str, json: &mut String) {
        json.push('"');
        for ch in value.chars() {
            match ch {
                '"' => json.push_str("\\\""),
                '\\' => json.push_str("\\\\"),
                '\n' => json.push_str("\\n"),
                '\r' => json.push_str("\\r"),
                '\t' => json.push_str("\\t"),
                '\u{08}' => json.push_str("\\b"),
                '\u{0C}' => json.push_str("\\f"),
                ch if ch <= '\u{1F}' => {
                    json.push_str("\\u");
                    let code = ch as u32;
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    json.push(HEX[((code >> 12) & 0xF) as usize] as char);
                    json.push(HEX[((code >> 8) & 0xF) as usize] as char);
                    json.push(HEX[((code >> 4) & 0xF) as usize] as char);
                    json.push(HEX[(code & 0xF) as usize] as char);
                }
                _ => json.push(ch),
            }
        }
        json.push('"');
    }
}
