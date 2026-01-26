use crate::components::{CRef, Component};
use crate::core::GameObjectId;
use crate::core::reflection::ReflectSerialize;
use crate::core::reflection::serialize_primitive::Value;
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
}

reflect_primitive!(String, this => Value::String(this.clone()));
reflect_primitive!(&str, this => Value::String(this.to_string()));
reflect_primitive!(f32, this => Value::Float(*this));
reflect_primitive!(f64, this => Value::Double(*this));
reflect_primitive!(i8, this => Value::Int(*this as i32));
reflect_primitive!(i16, this => Value::Int(*this as i32));
reflect_primitive!(i32, this => Value::Int(*this));
reflect_primitive!(i64, this => Value::BigInt(*this));
reflect_primitive!(isize, this => Value::BigInt(*this as i64));
reflect_primitive!(i128, this => Value::VeryBigInt(*this));
reflect_primitive!(u8, this => Value::UInt(*this as u32));
reflect_primitive!(u16, this => Value::UInt(*this as u32));
reflect_primitive!(u32, this => Value::UInt(*this));
reflect_primitive!(u64, this => Value::UBigInt(*this));
reflect_primitive!(usize, this => Value::UBigInt(*this as u64));
reflect_primitive!(u128, this => Value::UVeryBigInt(*this));
reflect_primitive!(Value, this => this.clone());
reflect_primitive!(Duration, this => Value::UVeryBigInt(this.as_millis()));

register_primitive_type!(Vec<GameObjectId>);
register_primitive_type!(Vec<CRef<dyn Component>>);
