use crate::core::reflection::ReflectSerialize;
use crate::core::reflection::serialize_primitive::Value;
use std::collections::HashMap;

impl<T: ReflectSerialize> ReflectSerialize for Vec<T> {
    fn serialize(this: &Self) -> Value {
        let list = this.iter().map(|v| T::serialize(v)).collect();
        Value::Array(list)
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
        Value::Map(map)
    }
}
