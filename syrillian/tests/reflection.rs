use syrillian::World;
use syrillian::components::RopeJoint;
use syrillian::components::joints::RopeConfig;
use syrillian::core::reflection::Reflect;
use syrillian::reflection::{function_info, function_infos};

#[syrillian::reflect_fn]
#[allow(unused)]
fn reflected_function(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn function_reflection() {
    let full_name = concat!(module_path!(), "::", "reflected_function");
    let info = function_info(full_name).expect("function should be registered");

    assert_eq!(info.name, "reflected_function");
    assert_eq!(info.module_path, module_path!());
    assert_eq!(info.full_name, full_name);
    assert!(info.signature.contains("fn reflected_function"));

    assert!(
        function_infos()
            .iter()
            .any(|entry| entry.full_name == full_name)
    );
}

#[test]
fn field_reflection() {
    let (mut world, ..) = World::fresh();
    let mut obj = world.new_object("Something");

    let mut joint = obj.add_component::<RopeJoint>();

    assert_ne!(joint.config.max_distance, 5.0);

    let config: &mut RopeConfig = Reflect::field_mut(&mut joint, "config").unwrap();
    config.max_distance = 5.0;

    assert_eq!(joint.config.max_distance, 5.0);

    let config: &RopeConfig = Reflect::field_ref(&mut joint, "config").unwrap();
    assert_eq!(config.max_distance, 5.0);
}
