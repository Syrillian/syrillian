use crate::{ReflectSerialize, Value};
use glamx::{Affine3A, Mat2, Mat3, Mat3A, Mat4, Quat, Vec2, Vec3, Vec4};

impl ReflectSerialize for Vec2 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![Value::Float(this.x), Value::Float(this.y)])
    }
}

impl ReflectSerialize for Vec3 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Float(this.x),
            Value::Float(this.y),
            Value::Float(this.z),
        ])
    }
}

impl ReflectSerialize for Vec4 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Float(this.x),
            Value::Float(this.y),
            Value::Float(this.z),
            Value::Float(this.w),
        ])
    }
}

impl ReflectSerialize for Mat2 {
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

impl ReflectSerialize for Mat3 {
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

impl ReflectSerialize for Mat3A {
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

impl ReflectSerialize for Mat4 {
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

impl ReflectSerialize for Quat {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![
            Value::Float(this.x),
            Value::Float(this.y),
            Value::Float(this.z),
            Value::Float(this.w),
        ])
    }
}

impl ReflectSerialize for Affine3A {
    fn serialize(this: &Self) -> Value {
        let mat = this.to_cols_array_2d();
        Value::Array(vec![
            Value::Array(vec![
                Value::Float(mat[0][0]),
                Value::Float(mat[0][1]),
                Value::Float(mat[0][2]),
            ]),
            Value::Array(vec![
                Value::Float(mat[1][0]),
                Value::Float(mat[1][1]),
                Value::Float(mat[1][2]),
            ]),
            Value::Array(vec![
                Value::Float(mat[2][0]),
                Value::Float(mat[2][1]),
                Value::Float(mat[2][2]),
            ]),
            Value::Array(vec![
                Value::Float(mat[3][0]),
                Value::Float(mat[3][1]),
                Value::Float(mat[3][2]),
            ]),
        ])
    }
}

crate::register_type!(crate::reflect_type_info!(glamx, Vec2, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Vec3, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Vec4, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Mat2, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Mat3, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Mat3A, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Mat4, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Quat, &[]));
crate::register_type!(crate::reflect_type_info!(glamx, Affine3A, &[]));
