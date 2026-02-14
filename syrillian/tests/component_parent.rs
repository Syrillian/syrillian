use syrillian::{World, components::Component};

#[derive(Debug, Default)]
struct ChildComponent;

impl Component for ChildComponent {}

#[derive(Debug, Default)]
struct MyComponent;

impl Component for MyComponent {
    fn init(&mut self, _world: &mut World) {
        self.parent().add_component::<ChildComponent>();
    }
    fn update(&mut self, _world: &mut World) {
        self.parent().delete();
    }
}

#[test]
fn component_parent_delete() {
    let (mut world, _rx1, _rx2, _pick_tx) = World::fresh();
    let mut obj = world.new_object("Test");
    obj.add_component::<MyComponent>();
    world.update();
    assert!(!obj.is_alive());
}
