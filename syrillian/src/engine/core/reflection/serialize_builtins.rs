use crate::core::GameObjectId;
use crate::core::reflection::{ReflectSerialize, Value};

impl ReflectSerialize for GameObjectId {
    fn serialize(this: &Self) -> Value {
        ReflectSerialize::serialize(&**this)
    }
}
