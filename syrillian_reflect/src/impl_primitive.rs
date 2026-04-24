use crate::{ReflectDeserialize, ReflectSerialize, Value};
use std::cell::Cell;
use std::collections::HashMap;
use web_time::Duration;

macro_rules! register_primitive_type {
    ($primitive:ty) => {
        ::syrillian_reflect::register_type!({
            ::syrillian_reflect::reflect_type_info!(primitive, $primitive)
        });
    };
}

macro_rules! register_default_primitive_type {
    ($primitive:ty) => {
        ::syrillian_reflect::register_type!({
            ::syrillian_reflect::reflect_type_info!(primitive, $primitive, default)
        });
    };
}

macro_rules! reflect_primitive {
    ($primitive:ty, $name:ident => $data:expr, $deser_name:ident, $deser_val:ident => $deser_body:expr) => {
        impl ReflectSerialize for $primitive {
            fn serialize($name: &Self) -> Value {
                $data
            }
        }

        impl ReflectDeserialize for $primitive {
            fn apply($deser_name: &mut Self, $deser_val: &Value) {
                $deser_body
            }
        }

        register_default_primitive_type!($primitive);
    };
    ($primitive:ty, $name:ident => $data:expr, $deser_name:ident, $deser_val:ident => $deser_body:expr, cell) => {
        impl ReflectSerialize for Cell<$primitive> {
            fn serialize($name: &Self) -> Value {
                let $name = &$name.get();
                $data
            }
        }

        impl ReflectDeserialize for Cell<$primitive> {
            fn apply(target: &mut Self, value: &Value) {
                let mut inner = target.get();
                <$primitive as ReflectDeserialize>::apply(&mut inner, value);
                target.set(inner);
            }
        }

        register_default_primitive_type!(Cell<$primitive>);

        reflect_primitive!($primitive, $name => $data, $deser_name, $deser_val => $deser_body);
    };
}

reflect_primitive!(String, this => Value::String(this.clone()),
    target, value => if let Value::String(s) = value { *target = s.clone(); });

reflect_primitive!(&str, this => Value::String(this.to_string()),
    _target, _value => { /* &str cannot be deserialized into */ }, cell);

reflect_primitive!(f32, this => Value::Float(*this),
    target, value => if let Some(v) = value.to_f64() { *target = v as f32; }, cell);

reflect_primitive!(f64, this => Value::Double(*this),
    target, value => if let Some(v) = value.to_f64() { *target = v; }, cell);

reflect_primitive!(i8, this => Value::Int(*this as i32),
    target, value => if let Some(v) = value.to_i128() { *target = v as i8; }, cell);

reflect_primitive!(i16, this => Value::Int(*this as i32),
    target, value => if let Some(v) = value.to_i128() { *target = v as i16; }, cell);

reflect_primitive!(i32, this => Value::Int(*this),
    target, value => if let Some(v) = value.to_i128() { *target = v as i32; }, cell);

reflect_primitive!(i64, this => Value::BigInt(*this),
    target, value => if let Some(v) = value.to_i128() { *target = v as i64; }, cell);

reflect_primitive!(isize, this => Value::BigInt(*this as i64),
    target, value => if let Some(v) = value.to_i128() { *target = v as isize; }, cell);

reflect_primitive!(i128, this => Value::VeryBigInt(*this),
    target, value => if let Some(v) = value.to_i128() { *target = v; }, cell);

reflect_primitive!(u8, this => Value::UInt(*this as u32),
    target, value => if let Some(v) = value.to_u128() { *target = v as u8; }, cell);

reflect_primitive!(u16, this => Value::UInt(*this as u32),
    target, value => if let Some(v) = value.to_u128() { *target = v as u16; }, cell);

reflect_primitive!(u32, this => Value::UInt(*this),
    target, value => if let Some(v) = value.to_u128() { *target = v as u32; }, cell);

reflect_primitive!(u64, this => Value::BigUInt(*this),
    target, value => if let Some(v) = value.to_u128() { *target = v as u64; }, cell);

reflect_primitive!(usize, this => Value::BigUInt(*this as u64),
    target, value => if let Some(v) = value.to_u128() { *target = v as usize; }, cell);

reflect_primitive!(u128, this => Value::VeryBigUInt(*this),
    target, value => if let Some(v) = value.to_u128() { *target = v; }, cell);

reflect_primitive!(bool, this => Value::Bool(*this),
    target, value => if let Value::Bool(b) = value { *target = *b; }, cell);

reflect_primitive!(Duration, this => Value::VeryBigUInt(this.as_millis()),
    target, value => {
        if let Some(ms) = value.to_u128() {
            *target = Duration::from_millis(ms as u64);
        }
    }, cell);

impl ReflectSerialize for Value {
    fn serialize(this: &Self) -> Value {
        this.clone()
    }
}

impl ReflectDeserialize for Value {
    fn apply(target: &mut Self, value: &Value) {
        *target = value.clone();
    }
}

register_primitive_type!(Value);

impl ReflectDeserialize for HashMap<String, Value> {
    fn apply(target: &mut Self, value: &Value) {
        if let Value::Object(map) = value {
            target.clear();
            for (k, v) in map {
                target.insert(k.clone(), v.clone());
            }
        }
    }
}

register_default_primitive_type!(HashMap<String, Value>);
