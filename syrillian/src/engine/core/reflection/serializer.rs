use crate::core::reflection::ReflectSerialize;
use crate::core::reflection::serialize_primitive::Value;

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
            Value::String(str) => {
                json.push('\"');
                *json += str;
                json.push('\"');
            }
            Value::Float(f) => *json += &f.to_string(),
            Value::Double(d) => *json += &d.to_string(),
            Value::UInt(i) => *json += &i.to_string(),
            Value::Int(i) => *json += &i.to_string(),
            Value::UBigInt(i) => *json += &i.to_string(),
            Value::BigInt(i) => *json += &i.to_string(),
            Value::UVeryBigInt(i) => *json += &i.to_string(),
            Value::VeryBigInt(i) => *json += &i.to_string(),
            Value::Map(m) => {
                json.push('{');
                let mut first = true;
                for (k, v) in m {
                    if first {
                        first = false;
                    } else {
                        json.push(',');
                    }
                    json.push('"');
                    *json += k;
                    *json += "\":";
                    Self::append_value_to_string(v, json);
                }
                json.push('}');
            }
            Value::Array(a) => {
                json.push('[');
                let mut first = true;
                for elem in a {
                    if first {
                        first = false;
                    } else {
                        json.push(',');
                    }
                    Self::append_value_to_string(elem, json);
                }
                json.push(']');
            }
            Value::None => *json += "null",
            Value::Bool(true) => *json += "true",
            Value::Bool(false) => *json += "false",
            Value::Serde(_) => {} // FIXME
        }
    }
}
