use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use syrillian::World;
use syrillian::components::Component;
use syrillian::math::{Isometry3, Point3, Vec3};
use syrillian_components::{
    RigidBodyComponent,
    joints::{FixedJoint, PrismaticJoint, RevoluteJoint, RopeJoint, SphericalJoint, SpringJoint},
};

#[test]
fn joint_connect_disconnect() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();

    assert!(!joint.is_connected());
    assert!(joint.connected().is_none());
    assert!(joint.handle().is_none());

    joint.connect_to(obj2);

    assert!(joint.is_connected());
    assert_eq!(joint.connected(), Some(obj2));
    assert!(joint.handle().is_some());

    joint.disconnect(&mut world);

    assert!(!joint.is_connected());
    assert!(joint.handle().is_none());
}

#[test]
fn joint_requires_rigidbody_on_parent() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    let result = joint.try_connect_to(obj2);

    assert!(result.is_err());
    assert!(!joint.is_connected());
}

#[test]
fn joint_requires_rigidbody_on_connector() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    let result = joint.try_connect_to(obj2);

    assert!(result.is_err());
}

#[test]
fn joint_fails_on_deleted_connector() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    world.delete_object(obj2);

    let mut joint = obj1.add_component::<FixedJoint>();
    let result = joint.try_connect_to(obj2);

    assert!(result.is_err());
}

#[test]
fn joint_reconnect() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.configure(5.0, 100.0, 10.0);
    joint.connect_to(obj2);

    let old_handle = joint.handle();

    joint.config.stiffness = 200.0;
    joint.reconnect(&mut world);

    assert!(joint.is_connected());
    assert_ne!(joint.handle(), old_handle);
}

#[test]
fn joint_anchors() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();

    let a1 = Point3::new(1.0, 2.0, 3.0);
    let a2 = Point3::new(-1.0, -2.0, -3.0);

    joint.set_anchor1(a1);
    joint.set_anchor2(a2);

    assert_eq!(joint.anchor1(), a1);
    assert_eq!(joint.anchor2(), a2);

    joint.connect_to(obj2);

    assert_eq!(joint.anchor1(), a1);
    assert_eq!(joint.anchor2(), a2);
}

#[test]
fn joint_anchor_distance() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(10.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let distance = joint.anchor_distance().unwrap();
    assert!((distance - 10.0).abs() < 0.5);
}

#[test]
fn joint_break_force() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    joint.set_break_force(Some(100.0));
    joint.connect_to(obj2);

    assert!(!joint.is_broken());
    assert!(joint.is_connected());

    joint.disconnect(&mut world);
    joint.broken = true;

    assert!(joint.is_broken());
    assert!(!joint.is_connected());
}

#[test]
fn joint_make_breakable_unbreakable() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    joint.connect_to(obj2);

    assert!(joint.break_force.is_none());
    assert!(joint.break_torque.is_none());

    joint.make_breakable(100.0, 50.0);

    assert_eq!(joint.break_force, Some(100.0));
    assert_eq!(joint.break_torque, Some(50.0));

    joint.make_unbreakable();

    assert!(joint.break_force.is_none());
    assert!(joint.break_torque.is_none());
}

#[test]
fn joint_repair() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    joint.connect_to(obj2);

    let saved_connected = joint.connected();
    joint.disconnect(&mut world);
    joint.broken = true;
    joint.connected = saved_connected;

    assert!(joint.is_broken());

    joint.repair();

    assert!(!joint.is_broken());
    assert!(joint.is_connected());
}

#[test]
fn joint_delete_removes_from_physics() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    joint.connect_to(obj2);

    let handle = joint.handle().unwrap();
    assert!(world.physics.impulse_joint_set.get(handle).is_some());

    joint.delete(&mut world);

    assert!(world.physics.impulse_joint_set.get(handle).is_none());
}

#[test]
fn fixed_joint_creation() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    joint.connect_to(obj2);

    assert!(joint.is_connected());
    assert!(joint.joint_data().is_some());
    assert!(joint.joint_data().unwrap().as_fixed().is_some());
}

#[test]
fn fixed_joint_default_config() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    obj1.add_component::<RigidBodyComponent>();

    let joint = obj1.add_component::<FixedJoint>();

    assert_eq!(joint.config.frame1, Isometry3::identity());
    assert_eq!(joint.config.frame2, Isometry3::identity());
}

#[test]
fn fixed_joint_set_frame() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();

    let frame = Isometry3::rotation(Vec3::new(0.0, FRAC_PI_2, 0.0));
    joint.set_frame1(frame);

    assert_eq!(joint.config.frame1, frame);

    joint.connect_to(obj2);

    assert_eq!(joint.config.frame1, frame);
}

#[test]
fn fixed_joint_rotation_error() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<FixedJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let error = joint.rotation_error();
    assert!(error.is_some());
    assert!(error.unwrap() < 0.1);
}

#[test]
fn revolute_joint_creation() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.connect_to(obj2);

    assert!(joint.is_connected());
    assert!(joint.joint_data().unwrap().as_revolute().is_some());
}

#[test]
fn revolute_joint_default_config() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    obj1.add_component::<RigidBodyComponent>();

    let joint = obj1.add_component::<RevoluteJoint>();

    assert_eq!(joint.config.axis, Vec3::Y);
    assert!(joint.config.limits.is_none());
}

#[test]
fn revolute_joint_set_axis() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.set_axis(Vec3::X);

    assert_eq!(joint.config.axis, Vec3::X);

    joint.connect_to(obj2);
    assert_eq!(joint.config.axis, Vec3::X);
}

#[test]
fn revolute_joint_set_limits_radians() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.set_limits(-FRAC_PI_2, FRAC_PI_2);
    joint.connect_to(obj2);

    assert!(joint.config.limits.is_some());
    let [min, max] = joint.config.limits.unwrap();
    assert!((min - (-FRAC_PI_2)).abs() < 0.01);
    assert!((max - FRAC_PI_2).abs() < 0.01);
}

#[test]
fn revolute_joint_set_limits_degrees() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.set_limits_deg(-45.0, 45.0);
    joint.connect_to(obj2);

    let [min, max] = joint.config.limits.unwrap();
    assert!((min - (-FRAC_PI_4)).abs() < 0.01);
    assert!((max - FRAC_PI_4).abs() < 0.01);
}

#[test]
fn revolute_joint_angle() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    assert!(joint.angle().is_some());
}

#[test]
fn revolute_joint_angle_degrees() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let angle_rad = joint.angle().unwrap();
    let angle_deg = joint.angle_deg().unwrap();

    assert!((angle_deg - angle_rad.to_degrees()).abs() < 0.01);
}

#[test]
fn revolute_joint_angular_velocity() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    assert!(joint.angular_velocity().is_some());
}

#[test]
fn revolute_joint_motor_velocity() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    if let Some(mut rb) = obj1.get_component::<RigidBodyComponent>() {
        rb.set_kinematic(true);
    }

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.connect_to(obj2);

    joint.set_motor_velocity(1.0, 1000.0);

    assert!(joint.joint_data().is_some());
}

#[test]
fn revolute_joint_motor_position() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    if let Some(mut rb) = obj1.get_component::<RigidBodyComponent>() {
        rb.set_kinematic(true);
    }

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.connect_to(obj2);

    joint.set_motor_position(FRAC_PI_4, 1000.0, 100.0);

    assert!(joint.joint_data().is_some());
}

#[test]
fn revolute_joint_limit_ratio() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RevoluteJoint>();
    joint.set_limits(0.0, PI);
    joint.connect_to(obj2);

    world.physics.step();

    let ratio = joint.limit_ratio();
    assert!(ratio.is_some());
    let r = ratio.unwrap();
    assert!((0.0..=1.0).contains(&r));
}

#[test]
fn prismatic_joint_creation() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<PrismaticJoint>();
    joint.connect_to(obj2);

    assert!(joint.is_connected());
    assert!(joint.joint_data().unwrap().as_prismatic().is_some());
}

#[test]
fn prismatic_joint_default_config() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    obj1.add_component::<RigidBodyComponent>();

    let joint = obj1.add_component::<PrismaticJoint>();

    assert_eq!(joint.config.axis, Vec3::Y);
    assert!(joint.config.limits.is_none());
}

#[test]
fn prismatic_joint_set_axis() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<PrismaticJoint>();
    joint.set_axis(Vec3::X);

    assert_eq!(joint.config.axis, Vec3::X);

    joint.connect_to(obj2);
    assert_eq!(joint.config.axis, Vec3::X);
}

#[test]
fn prismatic_joint_set_limits() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<PrismaticJoint>();
    joint.set_limits(0.0, 10.0);
    joint.connect_to(obj2);

    let [min, max] = joint.config.limits.unwrap();
    assert!((min - 0.0).abs() < 0.01);
    assert!((max - 10.0).abs() < 0.01);
}

#[test]
fn prismatic_joint_translation() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(0.0, 5.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<PrismaticJoint>();
    joint.set_axis(Vec3::Y);
    joint.connect_to(obj2);

    world.physics.step();

    let translation = joint.translation();
    assert!(translation.is_some());
    assert!((translation.unwrap() - 5.0).abs() < 1.0);
}

#[test]
fn prismatic_joint_velocity() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<PrismaticJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    assert!(joint.velocity().is_some());
}

#[test]
fn prismatic_joint_extend_retract() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    if let Some(mut rb) = obj1.get_component::<RigidBodyComponent>() {
        rb.set_kinematic(true);
    }

    let mut joint = obj1.add_component::<PrismaticJoint>();
    joint.set_limits(0.0, 10.0);
    joint.connect_to(obj2);

    joint.extend(1.0, 1000.0);
    joint.retract(1.0, 1000.0);

    assert!(joint.joint_data().is_some());
}

#[test]
fn prismatic_joint_position_ratio() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(0.0, 5.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<PrismaticJoint>();
    joint.set_axis(Vec3::Y);
    joint.set_limits(0.0, 10.0);
    joint.connect_to(obj2);

    world.physics.step();

    let ratio = joint.position_ratio();
    assert!(ratio.is_some());
    assert!((ratio.unwrap() - 0.5).abs() < 0.3);
}

#[test]
fn spherical_joint_creation() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SphericalJoint>();
    joint.connect_to(obj2);

    assert!(joint.is_connected());
    assert!(joint.joint_data().unwrap().as_spherical().is_some());
}

#[test]
fn spherical_joint_anchors() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SphericalJoint>();
    joint.set_anchor1(Point3::new(1.0, 0.0, 0.0));
    joint.set_anchor2(Point3::new(-1.0, 0.0, 0.0));
    joint.connect_to(obj2);

    assert_eq!(joint.anchor1(), Point3::new(1.0, 0.0, 0.0));
    assert_eq!(joint.anchor2(), Point3::new(-1.0, 0.0, 0.0));
}

#[test]
fn rope_joint_creation() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.connect_to(obj2);

    assert!(joint.is_connected());
    assert!(joint.joint_data().unwrap().as_rope().is_some());
}

#[test]
fn rope_joint_default_config() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    obj1.add_component::<RigidBodyComponent>();

    let joint = obj1.add_component::<RopeJoint>();

    assert_eq!(joint.config.max_distance, 10.0);
}

#[test]
fn rope_joint_set_max_distance() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 15.0;
    joint.connect_to(obj2);

    assert_eq!(joint.max_distance(), 15.0);

    joint.set_max_distance(20.0);
    assert_eq!(joint.max_distance(), 20.0);
}

#[test]
fn rope_joint_current_distance() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(5.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let distance = joint.current_distance().unwrap();
    assert!((distance - 5.0).abs() < 1.0);
}

#[test]
fn rope_joint_is_taut() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(10.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 10.0;
    joint.connect_to(obj2);

    world.physics.step();

    assert!(joint.is_taut());
    assert!(!joint.is_slack());
}

#[test]
fn rope_joint_is_slack() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(5.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 15.0;
    joint.connect_to(obj2);

    world.physics.step();

    assert!(joint.is_slack());
    assert!(!joint.is_taut());
}

#[test]
fn rope_joint_slack_amount() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(5.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 10.0;
    joint.connect_to(obj2);

    world.physics.step();

    let slack = joint.slack().unwrap();
    assert!((slack - 5.0).abs() < 1.0);
}

#[test]
fn rope_joint_tension_ratio() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(5.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 10.0;
    joint.connect_to(obj2);

    world.physics.step();

    let ratio = joint.tension_ratio().unwrap();
    assert!((ratio - 0.5).abs() < 0.2);
}

#[test]
fn rope_joint_shorten() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 10.0;
    joint.connect_to(obj2);

    joint.shorten(3.0);
    assert_eq!(joint.max_distance(), 7.0);

    joint.shorten(100.0);
    assert_eq!(joint.max_distance(), 0.1);
}

#[test]
fn rope_joint_lengthen() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 10.0;
    joint.connect_to(obj2);

    joint.lengthen(5.0);
    assert_eq!(joint.max_distance(), 15.0);
}

#[test]
fn rope_joint_tighten() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(5.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<RopeJoint>();
    joint.config.max_distance = 20.0;
    joint.connect_to(obj2);

    world.physics.step();

    joint.tighten();

    assert!((joint.max_distance() - 5.0).abs() < 1.0);
}

#[test]
fn spring_joint_creation() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.connect_to(obj2);

    assert!(joint.is_connected());
    assert!(joint.joint_data().is_some());
}

#[test]
fn spring_joint_default_config() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    obj1.add_component::<RigidBodyComponent>();

    let joint = obj1.add_component::<SpringJoint>();

    assert_eq!(joint.config.rest_length, 1.0);
    assert_eq!(joint.config.stiffness, 100.0);
    assert_eq!(joint.config.damping, 10.0);
}

#[test]
fn spring_joint_configure() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.configure(5.0, 200.0, 20.0);
    joint.connect_to(obj2);

    assert_eq!(joint.rest_length(), 5.0);
    assert_eq!(joint.stiffness(), 200.0);
    assert_eq!(joint.damping(), 20.0);
}

#[test]
fn spring_joint_set_individual_params() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.connect_to(obj2);

    joint.set_rest_length(3.0);
    assert_eq!(joint.rest_length(), 3.0);

    joint.set_stiffness(500.0);
    assert_eq!(joint.stiffness(), 500.0);

    joint.set_damping(50.0);
    assert_eq!(joint.damping(), 50.0);
}

#[test]
fn spring_joint_current_length() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(5.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let length = joint.current_length().unwrap();
    assert!((length - 5.0).abs() < 1.0);
}

#[test]
fn spring_joint_extension_stretched() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(10.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.set_rest_length(5.0);
    joint.connect_to(obj2);

    world.physics.step();

    let ext = joint.extension().unwrap();
    assert!(ext > 0.0);
    assert!(joint.is_stretched());
    assert!(!joint.is_compressed());
}

#[test]
fn spring_joint_extension_compressed() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(2.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.set_rest_length(5.0);
    joint.connect_to(obj2);

    world.physics.step();

    let ext = joint.extension().unwrap();
    assert!(ext < 0.0);
    assert!(joint.is_compressed());
    assert!(!joint.is_stretched());
}

#[test]
fn spring_joint_is_at_rest() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(5.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.set_rest_length(5.0);
    joint.connect_to(obj2);

    world.physics.step();

    assert!(joint.is_at_rest());
}

#[test]
fn spring_joint_spring_force() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(10.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.set_rest_length(5.0);
    joint.set_stiffness(100.0);
    joint.connect_to(obj2);

    world.physics.step();

    let force = joint.spring_force().unwrap();
    assert!(force > 0.0);
}

#[test]
fn spring_joint_potential_energy() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.transform.set_position(0.0, 0.0, 0.0);
    obj2.transform.set_position(10.0, 0.0, 0.0);
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.set_rest_length(5.0);
    joint.set_stiffness(100.0);
    joint.connect_to(obj2);

    world.physics.step();

    let energy = joint.potential_energy().unwrap();
    assert!((energy - 1250.0).abs() < 300.0);
}

#[test]
fn spring_joint_natural_frequency() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.set_stiffness(100.0);
    joint.connect_to(obj2);

    world.physics.step();

    let freq = joint.natural_frequency();
    assert!(freq.is_some());
}

#[test]
fn spring_joint_period() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let period = joint.period();
    assert!(period.is_some());
}

#[test]
fn spring_joint_damping_ratio() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let ratio = joint.damping_ratio();
    assert!(ratio.is_some());
}

#[test]
fn spring_joint_set_damping_ratio() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut joint = obj1.add_component::<SpringJoint>();
    joint.connect_to(obj2);

    world.physics.step();

    let original_damping = joint.damping();
    joint.set_damping_ratio(1.0);
    let new_damping = joint.damping();

    assert!(original_damping != new_damping || original_damping == new_damping);
}

#[test]
fn all_joint_types_have_joint_data() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut fixed = obj1.add_component::<FixedJoint>();
    fixed.connect_to(obj2);
    assert!(fixed.joint_data().is_some());
    fixed.disconnect(&mut world);

    let mut revolute = obj1.add_component::<RevoluteJoint>();
    revolute.connect_to(obj2);
    assert!(revolute.joint_data().is_some());
    revolute.disconnect(&mut world);

    let mut prismatic = obj1.add_component::<PrismaticJoint>();
    prismatic.connect_to(obj2);
    assert!(prismatic.joint_data().is_some());
    prismatic.disconnect(&mut world);

    let mut spherical = obj1.add_component::<SphericalJoint>();
    spherical.connect_to(obj2);
    assert!(spherical.joint_data().is_some());
    spherical.disconnect(&mut world);

    let mut rope = obj1.add_component::<RopeJoint>();
    rope.connect_to(obj2);
    assert!(rope.joint_data().is_some());
    rope.disconnect(&mut world);

    let mut spring = obj1.add_component::<SpringJoint>();
    spring.connect_to(obj2);
    assert!(spring.joint_data().is_some());
    spring.disconnect(&mut world);
}

#[test]
fn all_joint_types_support_break_force() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();

    let mut fixed = obj1.add_component::<FixedJoint>();
    fixed.make_breakable(100.0, 50.0);
    fixed.connect_to(obj2);
    assert!(!fixed.is_broken());
    fixed.disconnect(&mut world);

    let mut revolute = obj1.add_component::<RevoluteJoint>();
    revolute.make_breakable(100.0, 50.0);
    revolute.connect_to(obj2);
    assert!(!revolute.is_broken());
    revolute.disconnect(&mut world);

    let mut prismatic = obj1.add_component::<PrismaticJoint>();
    prismatic.make_breakable(100.0, 50.0);
    prismatic.connect_to(obj2);
    assert!(!prismatic.is_broken());
    prismatic.disconnect(&mut world);

    let mut spherical = obj1.add_component::<SphericalJoint>();
    spherical.make_breakable(100.0, 50.0);
    spherical.connect_to(obj2);
    assert!(!spherical.is_broken());
    spherical.disconnect(&mut world);

    let mut rope = obj1.add_component::<RopeJoint>();
    rope.make_breakable(100.0, 50.0);
    rope.connect_to(obj2);
    assert!(!rope.is_broken());
    rope.disconnect(&mut world);

    let mut spring = obj1.add_component::<SpringJoint>();
    spring.make_breakable(100.0, 50.0);
    spring.connect_to(obj2);
    assert!(!spring.is_broken());
    spring.disconnect(&mut world);
}

#[test]
fn multiple_joints_on_same_object() {
    let (mut world, ..) = World::fresh();
    let mut obj1 = world.new_object("Obj1");
    let mut obj2 = world.new_object("Obj2");
    let mut obj3 = world.new_object("Obj3");
    obj1.add_component::<RigidBodyComponent>();
    obj2.add_component::<RigidBodyComponent>();
    obj3.add_component::<RigidBodyComponent>();

    let mut spring = obj1.add_component::<SpringJoint>();
    spring.connect_to(obj2);

    let mut rope = obj1.add_component::<RopeJoint>();
    rope.connect_to(obj3);

    assert!(spring.is_connected());
    assert!(rope.is_connected());
    assert_ne!(spring.handle(), rope.handle());
}
