use glam::Vec4;
use glamx::{Vec2, Vec3};

pub trait Vertex: Clone {
    fn position(&self) -> Vec3;
    fn uv(&self) -> Vec2;
}

pub trait Vertex3D: Vertex {
    fn normal(&self) -> Vec3;
    fn tangent(&self) -> Vec4;
}

pub trait SkinnedVertex: Vertex3D {
    fn bone_weights(&self) -> [f32; 4];
    fn bone_indices(&self) -> [u32; 4];
}
