use crate::{ReflectDeserialize, ReflectSerialize, Value};
use glamx::{Affine3A, Mat2, Mat3, Mat3A, Mat4, Quat, Vec2, Vec3, Vec4};

fn extract_f32(value: &Value) -> f32 {
    match value {
        Value::Float(v) => *v,
        Value::Double(v) => *v as f32,
        Value::Int(v) => *v as f32,
        Value::UInt(v) => *v as f32,
        Value::BigInt(v) => *v as f32,
        Value::BigUInt(v) => *v as f32,
        _ => 0.0,
    }
}

fn extract_array_f32(value: &Value) -> Option<&[Value]> {
    match value {
        Value::Array(arr) => Some(arr.as_slice()),
        _ => None,
    }
}

fn extract_vec2(value: &Value) -> Option<Vec2> {
    let arr = extract_array_f32(value)?;
    if arr.len() < 2 {
        return None;
    }
    Some(Vec2::new(extract_f32(&arr[0]), extract_f32(&arr[1])))
}

fn extract_vec3(value: &Value) -> Option<Vec3> {
    let arr = extract_array_f32(value)?;
    if arr.len() < 3 {
        return None;
    }
    Some(Vec3::new(
        extract_f32(&arr[0]),
        extract_f32(&arr[1]),
        extract_f32(&arr[2]),
    ))
}

fn extract_vec4(value: &Value) -> Option<Vec4> {
    let arr = extract_array_f32(value)?;
    if arr.len() < 4 {
        return None;
    }
    Some(Vec4::new(
        extract_f32(&arr[0]),
        extract_f32(&arr[1]),
        extract_f32(&arr[2]),
        extract_f32(&arr[3]),
    ))
}

impl ReflectSerialize for Vec2 {
    fn serialize(this: &Self) -> Value {
        Value::Array(vec![Value::Float(this.x), Value::Float(this.y)])
    }
}

impl ReflectDeserialize for Vec2 {
    fn apply(target: &mut Self, value: &Value) {
        if let Some(vec) = extract_vec2(value) {
            *target = vec;
        }
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

impl ReflectDeserialize for Vec3 {
    fn apply(target: &mut Self, value: &Value) {
        if let Some(vec) = extract_vec3(value) {
            *target = vec;
        }
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

impl ReflectDeserialize for Vec4 {
    fn apply(target: &mut Self, value: &Value) {
        if let Some(vec) = extract_vec4(value) {
            *target = vec;
        }
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

impl ReflectDeserialize for Quat {
    fn apply(target: &mut Self, value: &Value) {
        if let Some(vec) = extract_vec4(value) {
            *target = Quat::from_xyzw(vec.x, vec.y, vec.z, vec.w);
        }
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

impl ReflectDeserialize for Mat2 {
    fn apply(target: &mut Self, value: &Value) {
        let Some(cols) = extract_array_f32(value) else {
            return;
        };
        if cols.len() < 2 {
            return;
        }
        let (Some(c0), Some(c1)) = (extract_vec2(&cols[0]), extract_vec2(&cols[1])) else {
            return;
        };
        *target = Mat2::from_cols(c0, c1);
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

impl ReflectDeserialize for Mat3 {
    fn apply(target: &mut Self, value: &Value) {
        let Some(cols) = extract_array_f32(value) else {
            return;
        };
        if cols.len() < 3 {
            return;
        }

        let mut vecs = [Vec3::ZERO; 3];
        for (i, v) in vecs.iter_mut().enumerate() {
            if let Some(col) = extract_vec3(&cols[i]) {
                *v = col;
            }
        }
        *target = Mat3::from_cols(vecs[0], vecs[1], vecs[2]);
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

impl ReflectDeserialize for Mat3A {
    fn apply(target: &mut Self, value: &Value) {
        let mut m = Mat3::from_cols(
            Vec3::new(target.x_axis.x, target.x_axis.y, target.x_axis.z),
            Vec3::new(target.y_axis.x, target.y_axis.y, target.y_axis.z),
            Vec3::new(target.z_axis.x, target.z_axis.y, target.z_axis.z),
        );
        Mat3::apply(&mut m, value);
        *target = Mat3A::from(m);
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

impl ReflectDeserialize for Mat4 {
    fn apply(target: &mut Self, value: &Value) {
        let Some(cols) = extract_array_f32(value) else {
            return;
        };
        if cols.len() < 4 {
            return;
        }

        let mut vecs = [Vec4::ZERO; 4];
        for (i, v) in vecs.iter_mut().enumerate() {
            if let Some(col) = extract_vec4(&cols[i]) {
                *v = col;
            }
        }
        *target = Mat4::from_cols(vecs[0], vecs[1], vecs[2], vecs[3]);
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

impl ReflectDeserialize for Affine3A {
    fn apply(target: &mut Self, value: &Value) {
        let Some(cols) = extract_array_f32(value) else {
            return;
        };
        if cols.len() < 4 {
            return;
        }

        let mut arr = [[0.0f32; 3]; 4];
        for (i, col) in arr.iter_mut().enumerate() {
            if let Some(values) = extract_array_f32(&cols[i]) {
                for (val, value) in col.iter_mut().zip(values) {
                    *val = extract_f32(value);
                }
            }
        }
        *target = Affine3A::from_cols_array_2d(&arr);
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
