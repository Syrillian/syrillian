use crate::{ReflectSerialize, Value};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

impl<T: ReflectSerialize> ReflectSerialize for Vec<T> {
    fn serialize(this: &Self) -> Value {
        let list = this.iter().map(T::serialize).collect();
        Value::Array(list)
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

impl<T: ReflectSerialize> ReflectSerialize for Range<T> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("start".to_string(), T::serialize(&this.start)),
            ("end".to_string(), T::serialize(&this.end)),
        ]))
    }
}
