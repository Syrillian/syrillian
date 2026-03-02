use slotmap::Key;
use syrillian::World;
use syrillian::engine::core::{GameObjectId, Transform};
use syrillian::math::{Affine3A, Quat, Vec3};

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

#[test]
fn render_affine_inherits_to_children() {
    let (mut world, ..) = World::fresh();

    let mut root = world.new_object("Root");
    let mut child = world.new_object("Child");
    let grandchild = world.new_object("Grandchild");

    world.add_child(root);
    root.add_child(child);
    child.add_child(grandchild);

    let root_affine = Affine3A::from_scale_rotation_translation(
        Vec3::splat(2.0),
        Quat::from_axis_angle(Vec3::Y, 0.5),
        Vec3::new(3.0, 4.0, 5.0),
    );

    root.transform.set_render_affine(Some(root_affine));

    assert_eq!(root.transform.render_affine(), Some(root_affine));
    assert_eq!(child.transform.render_affine(), Some(root_affine));
    assert_eq!(grandchild.transform.render_affine(), Some(root_affine));

    let child_affine = Affine3A::from_translation(Vec3::new(-2.0, 1.0, 6.0));
    child.transform.set_render_affine(Some(child_affine));

    assert_eq!(child.transform.render_affine(), Some(child_affine));
    assert_eq!(grandchild.transform.render_affine(), Some(child_affine));

    child.transform.set_render_affine(None);
    assert_eq!(child.transform.render_affine(), Some(root_affine));
    assert_eq!(grandchild.transform.render_affine(), Some(root_affine));

    root.transform.set_render_affine(None);
    assert_eq!(child.transform.render_affine(), None);
    assert_eq!(grandchild.transform.render_affine(), None);
}
