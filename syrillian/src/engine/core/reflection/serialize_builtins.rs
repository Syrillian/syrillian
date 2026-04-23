use crate::core::GameObjectId;
use crate::core::reflection::{ReflectDeserialize, ReflectSerialize, Value};

impl ReflectSerialize for GameObjectId {
    fn serialize(this: &Self) -> Value {
        ReflectSerialize::serialize(&**this)
    }
}

impl ReflectDeserialize for GameObjectId {
    fn apply(_target: &mut Self, _value: &Value) {
        // GameObjectIds are transient and cannot be meaningfully deserialized.
    }
}
