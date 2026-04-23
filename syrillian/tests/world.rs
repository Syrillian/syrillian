use std::cell::Cell;
use syrillian::World;
use syrillian::components::{CameraComponent, Component};
use syrillian::core::EventType;
use syrillian::core::GameObjectId;
use syrillian::math::{Quat, Vec3};
use syrillian::winit::dpi::PhysicalSize;
use syrillian_render::rendering::message::RenderMsg;
use syrillian_render::rendering::picking::PickResult;
use syrillian_render::rendering::viewport::ViewportId;
use web_time::Duration;

thread_local! {
    static TEST_WORLD_LOOP: Cell<u32> = const { Cell::new(0) };
    static TOUCH_PARENT_UPDATES: Cell<u32> = const { Cell::new(0) };
    static ADDED_COMPONENT_UPDATES: Cell<u32> = const { Cell::new(0) };
    static REMOVABLE_COMPONENT_UPDATES: Cell<u32> = const { Cell::new(0) };
    static PICK_CLICK_COUNT: Cell<u32> = const { Cell::new(0) };
}

fn set_test_world_loop(loop_idx: u32) {
    TEST_WORLD_LOOP.with(|slot| slot.set(loop_idx));
}

fn current_test_world_loop() -> u32 {
    TEST_WORLD_LOOP.with(Cell::get)
}

fn reset_touch_parent_updates() {
    TOUCH_PARENT_UPDATES.with(|slot| slot.set(0));
}

fn touch_parent_updates() -> u32 {
    TOUCH_PARENT_UPDATES.with(Cell::get)
}

fn reset_added_component_updates() {
    ADDED_COMPONENT_UPDATES.with(|slot| slot.set(0));
}

fn added_component_updates() -> u32 {
    ADDED_COMPONENT_UPDATES.with(Cell::get)
}

fn reset_removable_component_updates() {
    REMOVABLE_COMPONENT_UPDATES.with(|slot| slot.set(0));
}

fn removable_component_updates() -> u32 {
    REMOVABLE_COMPONENT_UPDATES.with(Cell::get)
}

fn reset_pick_click_count() {
    PICK_CLICK_COUNT.with(|slot| slot.set(0));
}

fn pick_click_count() -> u32 {
    PICK_CLICK_COUNT.with(Cell::get)
}

#[derive(Default)]
struct TestRotateComponent;

impl Component for TestRotateComponent {
    fn update(&mut self, _world: &mut World) {
        self.parent()
            .transform
            .rotate(Quat::from_axis_angle(Vec3::Y, 0.01));
    }
}

#[derive(Default)]
struct DeleteParentInInitComponent;

impl Component for DeleteParentInInitComponent {
    fn init(&mut self, _world: &mut World) {
        self.parent().delete();
    }
}

#[derive(Default)]
struct DeleteParentInUpdateComponent;

impl Component for DeleteParentInUpdateComponent {
    fn update(&mut self, _world: &mut World) {
        if current_test_world_loop() == 1 {
            self.parent().delete();
        }
    }
}

#[derive(Default)]
struct DeleteParentInLateUpdateComponent;

impl Component for DeleteParentInLateUpdateComponent {
    fn late_update(&mut self, _world: &mut World) {
        if current_test_world_loop() == 2 {
            self.parent().delete();
        }
    }
}

#[derive(Default)]
struct DeleteParentInPostUpdateComponent;

impl Component for DeleteParentInPostUpdateComponent {
    fn post_update(&mut self, _world: &mut World) {
        if current_test_world_loop() == 2 {
            self.parent().delete();
        }
    }
}

#[derive(Default)]
struct DeleteSelfInUpdateFirstComponent;

impl Component for DeleteSelfInUpdateFirstComponent {
    fn update(&mut self, _world: &mut World) {
        self.parent().delete();
    }
}

#[derive(Default)]
struct TouchParentInUpdateComponent;

impl Component for TouchParentInUpdateComponent {
    fn update(&mut self, _world: &mut World) {
        let _ = self.parent().name.len();
        TOUCH_PARENT_UPDATES.with(|slot| slot.set(slot.get() + 1));
    }
}

#[derive(Default)]
struct AddedInUpdateComponent;

impl Component for AddedInUpdateComponent {
    fn update(&mut self, _world: &mut World) {
        ADDED_COMPONENT_UPDATES.with(|slot| slot.set(slot.get() + 1));
    }
}

#[derive(Default)]
struct AddComponentInUpdateComponent {
    added: bool,
}

impl Component for AddComponentInUpdateComponent {
    fn update(&mut self, _world: &mut World) {
        if self.added {
            return;
        }

        self.parent().add_component::<AddedInUpdateComponent>();
        self.added = true;
    }
}

#[derive(Default)]
struct ClickCountingComponent;

impl Component for ClickCountingComponent {
    fn on_click(&mut self, _world: &mut World) {
        PICK_CLICK_COUNT.with(|slot| slot.set(slot.get() + 1));
    }
}

#[derive(Default)]
struct RemovableInUpdateComponent;

impl Component for RemovableInUpdateComponent {
    fn update(&mut self, _world: &mut World) {
        REMOVABLE_COMPONENT_UPDATES.with(|slot| slot.set(slot.get() + 1));
    }
}

#[derive(Default)]
struct RemoveSiblingInUpdateComponent {
    removed: bool,
}

impl Component for RemoveSiblingInUpdateComponent {
    fn update(&mut self, world: &mut World) {
        if self.removed {
            return;
        }

        let mut parent = self.parent();
        if let Some(removable) = parent.get_component::<RemovableInUpdateComponent>() {
            parent.remove_component(removable, world);
            self.removed = true;
        }
    }
}

fn run_world_loop_normal(world: &mut World) {
    world.fixed_update();
    world.update();
    world.post_update();
    world.next_frame();
}

fn run_world_loop_post_then_update(world: &mut World) {
    world.fixed_update();
    world.post_update();
    world.update();
    world.next_frame();
}

fn alive_objects(world: &World) -> usize {
    world.objects.values().filter(|obj| obj.is_alive()).count()
}

fn component_count<C: Component + 'static>(world: &World) -> usize {
    world
        .components
        .values_of_type::<C>()
        .map(|v| v.count())
        .unwrap_or(0)
}

fn assert_all_alive(ids: &[GameObjectId]) {
    for id in ids {
        assert!(id.exists(), "expected object {:?} to be alive", id);
    }
}

fn assert_all_deleted(ids: &[GameObjectId]) {
    for id in ids {
        assert!(!id.exists(), "expected object {:?} to be deleted", id);
    }
}

#[test]
fn new_object_add_find_delete() {
    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let id = world.new_object("TestObject");
    world.add_child(id);
    assert!(world.find_object_by_name("TestObject").is_some());
    assert_eq!(world.children.len(), 1);
    assert!(world.get_object(id).is_some());
    world.delete_object(id);
    assert!(world.get_object(id).is_none());
    assert_eq!(world.children.len(), 0);
}

#[test]
fn delta_time_advances() {
    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    std::thread::sleep(Duration::from_millis(1));
    world.update();
    world.next_frame();
    assert!(world.delta_time() > Duration::ZERO);
}

#[test]
fn strong_refs_keep_objects_alive_until_drop() {
    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let id = world.new_object("KeepAlive");
    let handle = world.get_object_ref(id).expect("object should exist");

    world.delete_object(id);

    assert!(world.get_object(id).is_none(), "deleted objects are hidden");
    assert!(
        world.objects.contains_key(id),
        "object storage is kept alive while strong refs exist"
    );

    drop(handle);
    assert!(
        !world.objects.contains_key(id),
        "object storage is freed once the last reference drops"
    );
}

#[test]
fn weak_refs_upgrade_only_when_alive() {
    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let id = world.new_object("WeakSubject");
    let weak = id.downgrade();

    assert!(weak.upgrade().is_some());
    world.delete_object(id);
    assert!(
        weak.upgrade().is_none(),
        "weak refs should not revive deleted objects"
    );
}

#[test]
fn shutdown_cleans_world_state() {
    let (mut world, _rx1, _rx2, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let id = world.new_object("ToDelete");
    world.add_child(id);
    world.shutdown();

    assert!(world.objects.is_empty());
    assert!(world.children.is_empty());
}

#[test]
fn objects_have_unique_hashes() {
    let (mut world, ..) = World::fresh();
    let a = world.new_object("A");
    let b = world.new_object("B");

    assert_ne!(a.object_hash(), 0);
    assert_ne!(b.object_hash(), 0);
    assert_ne!(a.object_hash(), b.object_hash());
}

#[test]
fn click_registration_toggles() {
    let (mut world, ..) = World::fresh();
    let obj = world.new_object("Clickable");

    assert!(!world.is_listening_for(obj, EventType::CLICK));
    obj.notify_for(&mut world, EventType::CLICK);
    assert!(world.is_listening_for(obj, EventType::CLICK));
    obj.stop_notify_for(&mut world, EventType::CLICK);
    assert!(!world.is_listening_for(obj, EventType::CLICK));
}

#[test]
fn lifecycle_parent_deletion_across_loops() {
    let (mut world, _render_rx, _event_rx, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let mut objects = Vec::with_capacity(100);

    for index in 0..100 {
        let mut obj = world.new_object(format!("Lifecycle_{index}"));
        world.add_child(obj);
        obj.add_component::<TestRotateComponent>();
        obj.add_component::<CameraComponent>();
        objects.push(obj);
    }

    let init_delete = &objects[0..20];
    let update_delete = &objects[20..40];
    let late_delete = &objects[40..60];
    let post_delete = &objects[60..80];
    let survivors = &objects[80..100];

    for mut obj in init_delete.iter().copied() {
        obj.add_component::<DeleteParentInInitComponent>();
    }
    for mut obj in update_delete.iter().copied() {
        obj.add_component::<DeleteParentInUpdateComponent>();
    }
    for mut obj in late_delete.iter().copied() {
        obj.add_component::<DeleteParentInLateUpdateComponent>();
    }
    for mut obj in post_delete.iter().copied() {
        obj.add_component::<DeleteParentInPostUpdateComponent>();
    }

    assert_eq!(alive_objects(&world), 80);
    assert_eq!(component_count::<TestRotateComponent>(&world), 80);
    assert_eq!(component_count::<CameraComponent>(&world), 80);
    assert_all_deleted(init_delete);
    assert_all_alive(update_delete);
    assert_all_alive(late_delete);
    assert_all_alive(post_delete);
    assert_all_alive(survivors);

    set_test_world_loop(0);
    run_world_loop_normal(&mut world);

    assert_eq!(alive_objects(&world), 80);
    assert_all_alive(update_delete);
    assert_all_alive(late_delete);
    assert_all_alive(post_delete);
    assert_all_alive(survivors);

    let rotate_trim = &survivors[0..10];
    let camera_trim = &survivors[10..15];

    for mut obj in rotate_trim.iter().copied() {
        let rotate = obj
            .get_component::<TestRotateComponent>()
            .expect("rotate component should exist before trim");
        obj.remove_component(rotate, &mut world);
    }

    for mut obj in camera_trim.iter().copied() {
        let camera = obj
            .get_component::<CameraComponent>()
            .expect("camera component should exist before trim");
        obj.remove_component(camera, &mut world);
    }

    for obj in rotate_trim.iter() {
        assert!(obj.get_component::<TestRotateComponent>().is_none());
        assert!(obj.get_component::<CameraComponent>().is_some());
    }

    for obj in camera_trim.iter() {
        assert!(obj.get_component::<TestRotateComponent>().is_some());
        assert!(obj.get_component::<CameraComponent>().is_none());
    }

    assert_eq!(alive_objects(&world), 80);
    assert_eq!(component_count::<TestRotateComponent>(&world), 70);
    assert_eq!(component_count::<CameraComponent>(&world), 75);

    set_test_world_loop(1);
    run_world_loop_normal(&mut world);

    assert_eq!(alive_objects(&world), 60);
    assert_eq!(component_count::<TestRotateComponent>(&world), 50);
    assert_eq!(component_count::<CameraComponent>(&world), 55);
    assert_all_deleted(update_delete);
    assert_all_alive(late_delete);
    assert_all_alive(post_delete);
    assert_all_alive(survivors);

    set_test_world_loop(2);
    run_world_loop_post_then_update(&mut world);

    assert_eq!(alive_objects(&world), 20);
    assert_eq!(component_count::<TestRotateComponent>(&world), 10);
    assert_eq!(component_count::<CameraComponent>(&world), 15);
    assert_all_deleted(late_delete);
    assert_all_deleted(post_delete);
    assert_all_alive(survivors);
}

#[test]
fn repro_delete_during_component_iteration() {
    let (mut world, _render_rx, _event_rx, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    reset_touch_parent_updates();

    let mut victim = world.new_object("Victim");
    victim.add_component::<DeleteSelfInUpdateFirstComponent>();
    victim.add_component::<TouchParentInUpdateComponent>();

    world.update();

    assert_eq!(touch_parent_updates(), 1);
    assert!(world.get_object(victim).is_none());
    assert!(!world.objects.contains_key(victim));
}

#[test]
fn component_added_in_update_starts_next_phase() {
    let (mut world, ..) = World::fresh();
    reset_added_component_updates();

    let mut obj = world.new_object("Adder");
    obj.add_component::<AddComponentInUpdateComponent>();

    world.update();
    assert_eq!(added_component_updates(), 0);

    world.update();
    assert_eq!(added_component_updates(), 1);
}

#[test]
fn removing_sibling_component_mid_phase_is_safe() {
    let (mut world, ..) = World::fresh();
    reset_removable_component_updates();

    let mut obj = world.new_object("Remover");
    obj.add_component::<RemoveSiblingInUpdateComponent>();
    obj.add_component::<RemovableInUpdateComponent>();

    world.update();

    assert_eq!(removable_component_updates(), 0);
    assert_eq!(component_count::<RemovableInUpdateComponent>(&world), 0);
}

#[test]
fn pick_results_are_stored_by_request_id_and_consumed_once() {
    let (mut world, _render_rx, _event_rx, _assets_rx, pick_tx, _hit_rect_tx) = World::fresh();
    reset_pick_click_count();

    let mut picked = world.new_object("Pickable");
    picked.add_component::<ClickCountingComponent>();
    picked.notify_for(&mut world, EventType::CLICK);
    let hash = picked.object_hash();
    let empty_id = world.request_pick(ViewportId::PRIMARY, (1, 1)).unwrap();
    let hit_id = world.request_pick(ViewportId::PRIMARY, (2, 2)).unwrap();

    pick_tx
        .send(PickResult {
            id: empty_id,
            target: ViewportId::PRIMARY,
            hash: None,
        })
        .unwrap();
    pick_tx
        .send(PickResult {
            id: hit_id,
            target: ViewportId::PRIMARY,
            hash: Some(hash),
        })
        .unwrap();
    pick_tx
        .send(PickResult {
            id: 999,
            target: ViewportId::PRIMARY,
            hash: Some(hash),
        })
        .unwrap();
    world.update();

    let hit = world.take_pick_result(hit_id).unwrap();
    assert_eq!(hit.request_id, hit_id);
    assert_eq!(hit.target, ViewportId::PRIMARY);
    assert_eq!(hit.hash, Some(hash));
    assert_eq!(hit.object, Some(picked));
    assert_eq!(world.take_pick_result(hit_id), None);

    let empty = world.take_pick_result(empty_id).unwrap();
    assert_eq!(empty.request_id, empty_id);
    assert_eq!(empty.target, ViewportId::PRIMARY);
    assert_eq!(empty.hash, None);
    assert_eq!(empty.object, None);
    assert_eq!(world.take_pick_result(empty_id), None);
    assert_eq!(world.take_pick_result(999), None);
    assert_eq!(pick_click_count(), 0);
}

#[test]
fn request_pick_at_sends_clamped_request() {
    let (mut world, render_rx, _event_rx, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    world.set_viewport_size(ViewportId::PRIMARY, PhysicalSize::new(100, 50));

    let id = world
        .request_pick_at(ViewportId::PRIMARY, 200.0, -5.0)
        .unwrap();

    assert_eq!(id, 0);
    let RenderMsg::PickRequest(request) = render_rx.try_recv().unwrap() else {
        panic!("expected pick request");
    };
    assert_eq!(request.id, 0);
    assert_eq!(request.target, ViewportId::PRIMARY);
    assert_eq!(request.position, (99, 0));
}
