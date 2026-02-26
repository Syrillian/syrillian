use glamx::{Vec2, Vec3};
use std::fmt::Debug;

pub trait Vertex: Debug + Clone {
    fn position(&self) -> Vec3;
    fn uv(&self) -> Vec2;
}

pub trait Vertex3D: Vertex {
    fn normal(&self) -> Vec3;
    fn tangent(&self) -> Vec3;
}

pub trait SkinnedVertex: Vertex3D {
    fn bone_weights(&self) -> [f32; 4];
    fn bone_indices(&self) -> [u32; 4];
}
