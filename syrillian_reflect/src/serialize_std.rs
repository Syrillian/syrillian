use crate::{ReflectDeserialize, ReflectSerialize, Value};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

impl<T: ReflectSerialize> ReflectSerialize for Vec<T> {
    fn serialize(this: &Self) -> Value {
        let list = this.iter().map(T::serialize).collect();
        Value::Array(list)
    }
}

impl<T: ReflectDeserialize + Default> ReflectDeserialize for Vec<T> {
    fn apply(target: &mut Self, value: &Value) {
        if let Value::Array(arr) = value {
            target.clear();
            for item_value in arr {
                let mut item = T::default();
                T::apply(&mut item, item_value);
                target.push(item);
            }
        }
    }
}

impl<T: ReflectSerialize> ReflectSerialize for Option<T> {
    fn serialize(this: &Self) -> Value {
        match this {
            Some(value) => T::serialize(value),
            None => Value::None,
        }
    }
}

impl<T: ReflectDeserialize + Default> ReflectDeserialize for Option<T> {
    fn apply(target: &mut Self, value: &Value) {
        match value {
            Value::None => *target = None,
            _ => {
                let mut inner = T::default();
                T::apply(&mut inner, value);
                *target = Some(inner);
            }
        }
    }
}

impl<K, V: ReflectSerialize> ReflectSerialize for HashMap<K, V>
where
    for<'a> String: From<&'a K>,
{
    fn serialize(this: &Self) -> Value {
        let map = this
            .iter()
            .map(|(k, v)| (k.into(), V::serialize(v)))
            .collect();
        Value::Object(map)
    }
}

impl<V: ReflectDeserialize + Default> ReflectDeserialize for HashMap<String, V> {
    fn apply(target: &mut Self, value: &Value) {
        if let Value::Object(map) = value {
            target.clear();
            for (k, v) in map {
                let mut item = V::default();
                V::apply(&mut item, v);
                target.insert(k.clone(), item);
            }
        }
    }
}

impl<K, V: ReflectSerialize> ReflectSerialize for BTreeMap<K, V>
where
    for<'a> String: From<&'a K>,
{
    fn serialize(this: &Self) -> Value {
        let map = this
            .iter()
            .map(|(k, v)| (k.into(), V::serialize(v)))
            .collect();
        Value::Object(map)
    }
}

impl<V: ReflectDeserialize + Default> ReflectDeserialize for BTreeMap<String, V> {
    fn apply(target: &mut Self, value: &Value) {
        if let Value::Object(map) = value {
            target.clear();
            for (k, v) in map {
                let mut item = V::default();
                V::apply(&mut item, v);
                target.insert(k.clone(), item);
            }
        }
    }
}

impl<T: ReflectSerialize> ReflectSerialize for Range<T> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("start".to_string(), T::serialize(&this.start)),
            ("end".to_string(), T::serialize(&this.end)),
        ]))
    }
}

impl<T: ReflectDeserialize + Default> ReflectDeserialize for Range<T> {
    fn apply(target: &mut Self, value: &Value) {
        if let Value::Object(map) = value {
            if let Some(start) = map.get("start") {
                T::apply(&mut target.start, start);
            }
            if let Some(end) = map.get("end") {
                T::apply(&mut target.end, end);
            }
        }
    }
}
