use crate::core::reflection::{ReflectSerialize, Value};
use crate::core::{GameObject, GameObjectId};
use std::borrow::Borrow;
use std::collections::HashMap;

impl ReflectSerialize for HashMap<GameObjectId, Box<GameObject>> {
    fn serialize(this: &Self) -> Value {
        let mut list = Vec::new();
        for obj in this.values() {
            list.push(GameObject::serialize(obj));
        }
        Value::Array(list)
    }
}

impl ReflectSerialize for GameObjectId {
    fn serialize(this: &Self) -> Value {
        GameObject::serialize(this.borrow())
    }
}
