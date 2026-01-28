use crate::core::reflection::Value;
use crate::math::{Mat2, Mat3, Mat4, Vec2, Vec3, Vec4};

impl syrillian::core::reflection::ReflectSerialize for Vec2 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![Value::Float(this.x), Value::Float(this.y)])
    }
}

impl syrillian::core::reflection::ReflectSerialize for Vec3 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Float(this.x),
            Value::Float(this.y),
            Value::Float(this.z),
        ])
    }
}

impl syrillian::core::reflection::ReflectSerialize for Vec4 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Float(this.x),
            Value::Float(this.y),
            Value::Float(this.z),
            Value::Float(this.w),
        ])
    }
}

impl syrillian::core::reflection::ReflectSerialize for Mat2 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Array(vec![
                Value::Float(this.x_axis.x),
                Value::Float(this.x_axis.y),
            ]),
            Value::Array(vec![
                Value::Float(this.y_axis.x),
                Value::Float(this.y_axis.y),
            ]),
        ])
    }
}

impl syrillian::core::reflection::ReflectSerialize for Mat3 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Array(vec![
                Value::Float(this.x_axis.x),
                Value::Float(this.x_axis.y),
                Value::Float(this.x_axis.z),
            ]),
            Value::Array(vec![
                Value::Float(this.y_axis.x),
                Value::Float(this.y_axis.y),
                Value::Float(this.y_axis.z),
            ]),
            Value::Array(vec![
                Value::Float(this.z_axis.x),
                Value::Float(this.z_axis.y),
                Value::Float(this.z_axis.z),
            ]),
        ])
    }
}

impl syrillian::core::reflection::ReflectSerialize for Mat4 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Array(vec![
                Value::Float(this.x_axis.x),
                Value::Float(this.x_axis.y),
                Value::Float(this.x_axis.z),
                Value::Float(this.x_axis.w),
            ]),
            Value::Array(vec![
                Value::Float(this.y_axis.x),
                Value::Float(this.y_axis.y),
                Value::Float(this.y_axis.z),
                Value::Float(this.y_axis.w),
            ]),
            Value::Array(vec![
                Value::Float(this.z_axis.x),
                Value::Float(this.z_axis.y),
                Value::Float(this.z_axis.z),
                Value::Float(this.z_axis.w),
            ]),
            Value::Array(vec![
                Value::Float(this.w_axis.x),
                Value::Float(this.w_axis.y),
                Value::Float(this.w_axis.z),
                Value::Float(this.w_axis.w),
            ]),
        ])
    }
}

syrillian::register_type!(syrillian::reflect_type_info!(syrillian::math, Vec2, &[]));
syrillian::register_type!(syrillian::reflect_type_info!(syrillian::math, Vec3, &[]));
syrillian::register_type!(syrillian::reflect_type_info!(syrillian::math, Vec4, &[]));
syrillian::register_type!(syrillian::reflect_type_info!(syrillian::math, Mat2, &[]));
syrillian::register_type!(syrillian::reflect_type_info!(syrillian::math, Mat3, &[]));
syrillian::register_type!(syrillian::reflect_type_info!(syrillian::math, Mat4, &[]));
