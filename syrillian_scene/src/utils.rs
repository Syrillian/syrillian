use serde_json::Value as JsonValue;
use std::collections::HashSet;
use syrillian::core::reflection::Value;
use syrillian_asset::store::streaming_asset_store::normalize_asset_path;

pub fn json_to_reflection_value(json: JsonValue) -> Value {
    match json {
        JsonValue::Null => Value::None,
        JsonValue::Bool(bool) => Value::Bool(bool),

        JsonValue::Number(a) if a.is_i64() => Value::BigInt(a.as_i64().unwrap()),
        JsonValue::Number(a) if a.is_u64() => Value::BigUInt(a.as_u64().unwrap()),
        JsonValue::Number(a) => Value::Double(a.as_f64().unwrap()), // fallback. this does fit all other cases.

        JsonValue::String(s) => Value::String(s),

        JsonValue::Array(a) => Value::Array(a.into_iter().map(json_to_reflection_value).collect()),

        JsonValue::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, v)| (k, json_to_reflection_value(v)))
                .collect(),
        ),
    }
}

pub fn sanitize_name(name: &str) -> Option<String> {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn virtual_root_from_path(path: &str) -> String {
    let normalized = normalize_asset_path(path);
    if normalized.is_empty() {
        "scene".to_string()
    } else {
        normalized
    }
}

pub fn unique_name<F: Fn() -> String>(
    preferred: Option<&str>,
    fallback: F,
    used_names: &mut HashSet<String>,
) -> String {
    let base = preferred.and_then(sanitize_name).unwrap_or_else(fallback);

    if used_names.insert(base.clone()) {
        return base;
    }

    let mut index = 1usize;
    loop {
        let candidate = format!("{base}_{index}");
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}
