use crate::components::{CRef, Component};
use crate::core::GameObjectId;
use crate::core::reflection::ReflectSerialize;
use crate::core::reflection::serialize_primitive::Value;
use std::cell::Cell;
use std::collections::HashMap;
use web_time::Duration;

macro_rules! register_primitive_type {
    ($primitive:ty) => {
        ::syrillian::register_type!({ ::syrillian::reflect_type_info!(primitive, $primitive) });
    };
}

macro_rules! reflect_primitive {
    ($primitive:ty, $name:ident => $data:expr) => {
        impl ReflectSerialize for $primitive {
            fn serialize($name: &Self) -> Value {
                $data
            }
        }

        register_primitive_type!($primitive);
    };
    ($primitive:ty, $name:ident => $data:expr, cell) => {
        impl ReflectSerialize for Cell<$primitive> {
            fn serialize($name: &Self) -> Value {
                let $name = &$name.get();
                $data
            }
        }

        register_primitive_type!(Cell<$primitive>);

        reflect_primitive!($primitive, $name => $data);
    }
}

reflect_primitive!(String, this => Value::String(this.clone()));
reflect_primitive!(&str, this => Value::String(this.to_string()), cell);
reflect_primitive!(f32, this => Value::Float(*this), cell);
reflect_primitive!(f64, this => Value::Double(*this), cell);
reflect_primitive!(i8, this => Value::Int(*this as i32), cell);
reflect_primitive!(i16, this => Value::Int(*this as i32), cell);
reflect_primitive!(i32, this => Value::Int(*this), cell);
reflect_primitive!(i64, this => Value::BigInt(*this), cell);
reflect_primitive!(isize, this => Value::BigInt(*this as i64), cell);
reflect_primitive!(i128, this => Value::VeryBigInt(*this), cell);
reflect_primitive!(u8, this => Value::UInt(*this as u32), cell);
reflect_primitive!(u16, this => Value::UInt(*this as u32), cell);
reflect_primitive!(u32, this => Value::UInt(*this), cell);
reflect_primitive!(u64, this => Value::UBigInt(*this), cell);
reflect_primitive!(usize, this => Value::UBigInt(*this as u64), cell);
reflect_primitive!(u128, this => Value::UVeryBigInt(*this), cell);
reflect_primitive!(bool, this => Value::Bool(*this), cell);
reflect_primitive!(Value, this => this.clone());
reflect_primitive!(Duration, this => Value::UVeryBigInt(this.as_millis()), cell);

register_primitive_type!(Vec<GameObjectId>);
register_primitive_type!(Vec<CRef<dyn Component>>);
register_primitive_type!(HashMap<String, Value>);
