use std::any::TypeId;
use syrillian::Reflect;
use syrillian::World;
use syrillian::components::{CameraComponent, Component};
use syrillian::math::{Quat, Vec3};
use syrillian::utils::TypedComponentHelper;

#[derive(Debug, Default, Reflect)]
struct MyComponent;

impl Component for MyComponent {
    fn init(&mut self, _world: &mut World) {
        self.parent().transform.translate(Vec3::X);
    }
}

#[test]
fn component() {
    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let mut obj = world.new_object("Test");

    let comp = obj.add_component::<MyComponent>();
    assert_eq!(obj.transform.position(), Vec3::X);

    let comp2 = obj.add_component::<MyComponent>();
    assert_eq!(obj.transform.position(), Vec3::X * 2.0);

    assert_eq!(comp.parent(), obj);
    assert_eq!(comp2.parent(), obj);

    assert_eq!(world.components.values().count(), 2);
    assert_eq!(
        world
            .components
            .values_of_type::<MyComponent>()
            .unwrap()
            .count(),
        2
    );

    obj.remove_component(&comp2, &mut world);
    assert_eq!(obj.iter_components::<MyComponent>().count(), 1);
    assert_eq!(world.components.values().count(), 1);
    assert_eq!(
        world
            .components
            .values_of_type::<MyComponent>()
            .unwrap()
            .count(),
        1
    );

    let comp2 = comp2.downgrade();
    assert_eq!(comp2.upgrade(&world), None);

    obj.delete();
    let comp = comp.downgrade();
    assert_eq!(comp.upgrade(&world), None);
}

#[test]
fn check_typed() {
    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let mut obj = world.new_object("Test");

    let comp = obj.add_component::<MyComponent>();
    let typed = comp.typed_id();

    assert_eq!(typed.type_id(), TypeId::of::<MyComponent>());

    obj.remove_component(comp, &mut world);

    assert_eq!(world.components.values().count(), 0);
}

#[test]
fn component_reflection() {
    let info_pre = syrillian::core::reflection::type_info_of::<MyComponent>()
        .expect("component type should be registered");
    assert_eq!(info_pre.type_id, TypeId::of::<MyComponent>());
    assert_eq!(info_pre.full_path, std::any::type_name::<MyComponent>());
    assert_eq!(info_pre.name, "MyComponent");

    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let mut obj = world.new_object("Test");

    let comp = obj.add_component::<MyComponent>();
    let info = comp
        .type_info()
        .expect("component type should be registered");

    assert_eq!(info.type_id, TypeId::of::<MyComponent>());
    assert_eq!(info.full_path, std::any::type_name::<MyComponent>());
    assert_eq!(info.name, "MyComponent");

    let typed = comp.typed_id();
    assert_eq!(typed.type_name(), Some(info.full_path));
}

#[test]
fn camera_click_ray_uses_camera_world_transform() {
    let (mut world, ..) = World::fresh();
    let mut obj = world.new_object("Camera");
    let camera = obj.add_component::<CameraComponent>();

    let position = Vec3::new(10.0, 2.0, -3.0);
    let rotation = Quat::from_rotation_y(0.7);
    obj.transform.set_position_vec(position);
    obj.transform.set_rotation(rotation);

    let ray = camera.click_ray(400.0, 300.0);
    let expected_dir = rotation * Vec3::NEG_Z;

    assert!((ray.origin - position).length() < 1e-4);
    assert!((ray.dir - expected_dir).length() < 1e-4);
}
