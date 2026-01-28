use syrillian::engine::core::Vertex3D;
use syrillian::math::{Vec2, Vec3};

#[test]
fn vertex_creation_pads_indices() {
    let v = Vertex3D::new(
        Vec3::ZERO,
        Vec2::ZERO,
        Vec3::Z,
        Vec3::X,
        &[1, 2],
        &[0.5, 0.5],
    );
    assert_eq!(v.bone_indices, [1, 2, 0, 0]);
    assert_eq!(v.bone_weights, [0.5, 0.5, 0.0, 0.0]);
}
