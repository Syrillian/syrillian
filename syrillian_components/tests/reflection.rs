use syrillian::World;
use syrillian::core::reflection::Reflect;
use syrillian_components::RopeJoint;
use syrillian_components::joints::RopeConfig;

#[test]
fn field_reflection() {
    let (mut world, ..) = World::fresh();
    let mut obj = world.new_object("Something");

    let mut joint = obj.add_component::<RopeJoint>();

    assert_ne!(joint.config.max_distance, 5.0);

    let config: &mut RopeConfig = Reflect::field_mut(&mut joint, "config").unwrap();
    config.max_distance = 5.0;

    assert_eq!(joint.config.max_distance, 5.0);

    let config: &RopeConfig = Reflect::field_ref(&joint, "config").unwrap();
    assert_eq!(config.max_distance, 5.0);
}
