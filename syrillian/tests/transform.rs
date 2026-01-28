use slotmap::Key;
use syrillian::engine::core::{GameObjectId, Transform};
use syrillian::math::{Quat, Vec3};

#[test]
fn local_position_and_translation() {
    let mut t = Transform::new(GameObjectId::null());
    assert_eq!(*t.local_position(), Vec3::ZERO);
    t.set_local_position_vec(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(*t.local_position(), Vec3::new(1.0, 2.0, 3.0));
    t.translate(Vec3::new(1.0, -1.0, 0.5));
    assert_eq!(*t.local_position(), Vec3::new(2.0, 1.0, 3.5));
}

#[test]
fn rotation_and_scale() {
    let mut t = Transform::new(GameObjectId::null());
    let rot = Quat::from_axis_angle(Vec3::Y, 1.0);
    t.set_local_rotation(rot);
    assert_eq!(*t.local_rotation(), rot);
    t.set_uniform_local_scale(2.0);
    assert_eq!(*t.local_scale(), Vec3::splat(2.0));
}
