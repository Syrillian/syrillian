use std::collections::HashMap;

use nalgebra::Matrix2;
use syrillian::core::reflection::serializer::JsonSerializer;
use syrillian::core::reflection::{Reflect, ReflectSerialize, ReflectedField, Value, type_info_of};

#[derive(Debug)]
struct Demo {
    a: u32,
    b: f32,
}

impl Reflect for Demo {}

const DEMO_FIELDS: &[ReflectedField] = &[
    syrillian::reflect_field!(Demo, a, u32),
    syrillian::reflect_field!(Demo, b, f32),
];

syrillian::register_type!(syrillian::reflect_type_info!(
    demo_reflection,
    Demo,
    DEMO_FIELDS
));

#[test]
fn primitive_type_info_and_serialize() {
    let info = type_info_of::<u32>().expect("u32 should be registered");
    assert_eq!(info.name, "u32");
    assert_eq!(info.full_path, "u32");
    assert_eq!(ReflectSerialize::serialize(&42u32), Value::UInt(42));
}

#[test]
fn std_container_serialization() {
    let values = vec![1u32, 2u32];
    let serialized = JsonSerializer::serialize_to_string(&values);
    assert_eq!(serialized, "[1,2]");

    let mut map = HashMap::new();
    map.insert("b".to_string(), 2u32);
    map.insert("a".to_string(), 1u32);
    let serialized = JsonSerializer::serialize_to_string(&map);
    assert_eq!(serialized, "{\"a\":1,\"b\":2}");
}

#[test]
fn nalgebra_matrix_serialization() {
    let matrix = Matrix2::<f32>::identity();
    let value = ReflectSerialize::serialize(&matrix);
    let expected = Value::Array(vec![
        Value::Array(vec![Value::Float(1.0), Value::Float(0.0)]),
        Value::Array(vec![Value::Float(0.0), Value::Float(1.0)]),
    ]);
    assert_eq!(value, expected);
    assert!(type_info_of::<Matrix2<f32>>().is_some());
}

#[test]
fn reflected_fields_and_struct_serialization() {
    let mut demo = Demo { a: 10, b: 1.5 };

    let info = type_info_of::<Demo>().expect("Demo should be registered");
    assert_eq!(info.name, "Demo");
    assert_eq!(info.full_path, "demo_reflection::Demo");
    assert_eq!(info.fields.len(), 2);

    let a = <Demo as Reflect>::field_ref::<u32>(&demo, "a").copied();
    assert_eq!(a, Some(10));
    let b = <Demo as Reflect>::field_ref::<f32>(&demo, "b").copied();
    assert_eq!(b, Some(1.5));

    if let Some(a_mut) = <Demo as Reflect>::field_mut::<u32>(&mut demo, "a") {
        *a_mut = 12;
    }
    assert_eq!(demo.a, 12);

    let serialized = JsonSerializer::serialize_to_string(&demo);
    assert_eq!(serialized, "{\"a\":12,\"b\":1.5}");
}
