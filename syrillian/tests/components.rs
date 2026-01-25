use nalgebra::Vector3;
use std::any::TypeId;
use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;

#[derive(Debug, Default, Reflect)]
struct MyComponent;

impl Component for MyComponent {
    fn init(&mut self, _world: &mut World) {
        self.parent()
            .transform
            .translate(Vector3::new(1.0, 0.0, 0.0));
    }
}

#[test]
fn component() {
    let (mut world, _rx1, _rx2, _pick_tx) = World::fresh();
    let mut obj = world.new_object("Test");

    let comp = obj.add_component::<MyComponent>();
    assert_eq!(obj.transform.position(), Vector3::new(1.0, 0.0, 0.0));

    let comp2 = obj.add_component::<MyComponent>();
    assert_eq!(obj.transform.position(), Vector3::new(2.0, 0.0, 0.0));

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
    let (mut world, _rx1, _rx2, _pick_tx) = World::fresh();
    let mut obj = world.new_object("Test");

    let comp = obj.add_component::<MyComponent>();
    let typed = comp.typed_id();

    assert_eq!(typed.type_id(), TypeId::of::<MyComponent>());

    obj.remove_component(comp, &mut world);

    assert_eq!(world.components.values().count(), 0);
}

#[test]
fn component_reflection() {
    let info_pre = syrillian::components::component_type_info(TypeId::of::<MyComponent>())
        .expect("component type should be registered");
    assert_eq!(info_pre.type_id, TypeId::of::<MyComponent>());
    assert_eq!(info_pre.type_name, std::any::type_name::<MyComponent>());
    assert_eq!(info_pre.short_name, "MyComponent");

    let (mut world, _rx1, _rx2, _pick_tx) = World::fresh();
    let mut obj = world.new_object("Test");

    let comp = obj.add_component::<MyComponent>();
    let info = comp.type_info();

    assert_eq!(info.type_id, TypeId::of::<MyComponent>());
    assert_eq!(info.type_name, std::any::type_name::<MyComponent>());
    assert_eq!(info.short_name, "MyComponent");
    assert_eq!(comp.type_name(), info.type_name);
    assert_eq!(info, info_pre);

    let typed = comp.typed_id();
    assert_eq!(typed.type_name(), Some(info.type_name));

    let registry = syrillian::components::component_type_info(TypeId::of::<MyComponent>())
        .expect("component type should be registered");
    assert_eq!(registry, info);
}
