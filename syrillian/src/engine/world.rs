//! The [`World`] struct stores and updates all game objects. Its use is to manage any
//! "raw" data, store and provide access to the objects and behavior, with a focus on ease of use.
//!
//! It maintains the scene graph, input state and physics simulation and
//! offers utilities such as methods to create, find and remove game objects.

use crate::assets::{
    BGL, Material, Mesh, RenderTexture2D, RenderTexture2DArray, Shader, Sound, Store, Texture2D,
    Texture2DArray,
};
use crate::audio::AudioScene;
use crate::components::{CRef, CWeak, CameraComponent, Component};
use crate::core::component_storage::ComponentStorage;
use crate::core::{EventType, GameObject, GameObjectId, GameObjectRef, ObjectHash, Transform};
use crate::engine::assets::AssetStore;
use crate::engine::prefabs::Prefab;
use crate::game_thread::GameAppEvent;
use crate::input::InputManager;
use crate::physics::PhysicsSimulation;
use crate::prefabs::CameraPrefab;
use crate::rendering::message::RenderMsg;
use crate::rendering::picking::PickRequest;
use crate::rendering::picking::PickResult;
use crate::rendering::strobe::StrobeFrame;
use crate::rendering::{CPUDrawCtx, UiContext};
use slotmap::{Key, SlotMap};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::mem;
use std::mem::swap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use web_time::{Duration, Instant};

use crate::ViewportId;
use crate::core::reflection::Value;
use crossbeam_channel::unbounded;
use crossbeam_channel::{Receiver, Sender};
use syrillian_macros::Reflect;
use winit::dpi::PhysicalSize;
use winit::event::MouseButton;

thread_local! {
    static CURRENT_WORLD: Cell<*mut World> = const { Cell::new(std::ptr::null_mut()) };
}

#[derive(Debug)]
struct WorldBinding {
    previous: *mut World,
}

impl WorldBinding {
    fn bind(world: &mut World) -> Self {
        let ptr = world as *mut _;
        let previous = CURRENT_WORLD.with(|slot| {
            let prev = slot.get();
            slot.set(ptr);
            prev
        });

        WorldBinding { previous }
    }
}

impl Drop for WorldBinding {
    fn drop(&mut self) {
        CURRENT_WORLD.with(|slot| {
            slot.set(self.previous);
        });
    }
}

#[derive(Clone)]
pub struct RenderTargets {
    pub active_camera: CWeak<CameraComponent>,
    pub size: PhysicalSize<u32>,
}

#[derive(Clone)]
pub struct WorldChannels {
    pub render_tx: Sender<RenderMsg>,
    pub game_event_tx: Sender<GameAppEvent>,
    pub pick_result_rx: Receiver<PickResult>,
    viewports: HashMap<ViewportId, RenderTargets>,
    next_target_id: u64,
}

impl WorldChannels {
    pub fn new(
        render_tx: Sender<RenderMsg>,
        game_event_tx: Sender<GameAppEvent>,
        pick_result_rx: Receiver<PickResult>,
    ) -> Self {
        let mut targets = HashMap::new();
        targets.insert(
            ViewportId::PRIMARY,
            RenderTargets {
                active_camera: CWeak::null(),
                size: PhysicalSize::new(0, 0),
            },
        );

        Self {
            render_tx,
            game_event_tx,
            pick_result_rx,
            viewports: targets,
            next_target_id: ViewportId::PRIMARY.get() + 1,
        }
    }

    pub fn set_active_camera(&mut self, target: ViewportId, camera: CWeak<CameraComponent>) {
        let entry = self.viewports.entry(target).or_insert(RenderTargets {
            active_camera: CWeak::null(),
            size: PhysicalSize::new(0, 0),
        });
        entry.active_camera = camera;
    }

    pub fn active_camera_for(&self, target: ViewportId) -> CWeak<CameraComponent> {
        self.viewports
            .get(&target)
            .map_or_else(CWeak::null, |t| t.active_camera)
    }

    pub fn set_viewport_size(&mut self, target: ViewportId, size: PhysicalSize<u32>) {
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        let entry = self.viewports.entry(target).or_insert(RenderTargets {
            active_camera: CWeak::null(),
            size,
        });
        entry.size = size;
    }

    pub fn add_window(
        &mut self,
        active_camera: CWeak<CameraComponent>,
        size: PhysicalSize<u32>,
    ) -> ViewportId {
        let target_id = ViewportId(self.next_target_id);
        self.viewports.insert(
            target_id,
            RenderTargets {
                active_camera,
                size,
            },
        );
        self.next_target_id += 1;
        target_id
    }
}

/// Central structure representing the running scene.
///
/// The world keeps track of all [`GameObject`](GameObject)
/// instances and provides access to shared systems like physics and input.
/// Bind a world to the current thread with [`World::bind_thread`] (done automatically in
/// [`World::new`]) to access it through [`World::instance`]. Multiple worlds can live
/// on different threads simultaneously.
#[derive(Reflect)]
pub struct World {
    /// Collection of all game objects indexed by their unique ID
    pub objects: SlotMap<GameObjectId, Box<GameObject>>,
    /// Collection of all components indexed by their unique ID
    pub components: ComponentStorage,
    /// Root-level game objects that have no parent
    #[reflect]
    pub children: Vec<GameObjectId>,
    /// Strong references keeping objects alive
    object_ref_counts: HashMap<GameObjectId, usize>,
    /// Objects that are awaiting final removal once all references drop
    pending_deletions: HashSet<GameObjectId>,
    /// Objects registered for click notifications
    click_listeners: HashSet<GameObjectId>,
    /// Allocated hashes to keep them unique per object
    object_hashes: HashSet<ObjectHash>,
    /// The currently active camera used for rendering
    main_active_camera: CWeak<CameraComponent>,
    /// Physics simulation system
    #[reflect]
    pub physics: PhysicsSimulation,
    /// Input management system
    pub input: InputManager,
    /// Asset storage containing meshes, textures, materials, etc.
    pub assets: Arc<AssetStore>,
    /// Spatial audio
    pub audio: AudioScene,

    /// Time when the world was created
    start_time: Instant,
    /// Time elapsed since the last frame
    delta_time: Duration,
    /// Time when the last frame started
    last_frame_time: Instant,
    /// Sequence id for picking requests
    next_pick_request_id: u64,

    /// Flag indicating whether a shutdown has been requested
    requested_shutdown: bool,
    pub(crate) channels: WorldChannels,
    thread_binding: Option<WorldBinding>,
    pub strobe: StrobeFrame,
}

impl World {
    /// Create a new, empty, clean-slate world with default data.
    fn empty(channels: WorldChannels, assets: Arc<AssetStore>) -> Box<World> {
        Box::new(World {
            objects: SlotMap::with_key(),
            components: ComponentStorage::default(),
            children: vec![],
            object_ref_counts: HashMap::new(),
            pending_deletions: HashSet::new(),
            click_listeners: HashSet::new(),
            object_hashes: HashSet::new(),
            main_active_camera: CWeak::null(),
            physics: PhysicsSimulation::default(),
            input: InputManager::new(channels.game_event_tx.clone()),
            assets,
            audio: AudioScene::default(),

            start_time: Instant::now(),
            delta_time: Duration::default(),
            last_frame_time: Instant::now(),
            next_pick_request_id: 0,

            requested_shutdown: false,
            channels,
            thread_binding: None,
            strobe: StrobeFrame::default(),
        })
    }

    /// Creates a new world through World::empty and binds it to the current thread.
    pub fn new(
        assets: Arc<AssetStore>,
        render_tx: Sender<RenderMsg>,
        game_event_tx: Sender<GameAppEvent>,
        pick_result_rx: Receiver<PickResult>,
    ) -> Box<World> {
        let channels = WorldChannels::new(render_tx, game_event_tx, pick_result_rx);
        World::new_with_channels(assets, channels)
    }

    pub fn new_with_channels(assets: Arc<AssetStore>, channels: WorldChannels) -> Box<World> {
        let mut world = World::empty(channels, assets);
        world.bind_thread();
        world
    }

    /// View [`World::new`]. This function will just set up data structures around the world
    /// needed for initialization. Mostly useful for tests.
    pub fn fresh() -> (
        Box<World>,
        Receiver<RenderMsg>,
        Receiver<GameAppEvent>,
        Sender<PickResult>,
    ) {
        let (tx1, rx1) = unbounded();
        let (tx2, rx2) = unbounded();
        let (pick_tx, pick_rx) = unbounded();
        let store = AssetStore::new();
        let world = World::new(store, tx1, tx2, pick_rx);
        (world, rx1, rx2, pick_tx)
    }

    /// Returns a mutable reference to the global [`World`] instance.
    ///
    /// # Panics
    /// Panics if [`World::new`] has not been called beforehand.
    pub fn instance() -> &'static mut World {
        let ptr = CURRENT_WORLD.get();
        if ptr.is_null() {
            panic!("World has not been bound to this thread yet");
        }

        unsafe { &mut *ptr }
    }

    pub(crate) fn is_thread_loaded() -> bool {
        !CURRENT_WORLD.get().is_null()
    }

    /// Binds this world as the active [`World::instance`] for the current thread.
    ///
    /// The binding persists as long as the world is alive (or until it is rebound). This
    /// enables having multiple worlds active on different threads without relying on a
    /// single global mutable pointer.
    pub fn bind_thread(&mut self) {
        self.thread_binding = Some(WorldBinding::bind(self));
    }

    /// Replace the channels used to communicate with the render and windowing threads.
    ///
    /// This lets a world be moved between render targets without reconstructing it.
    pub fn rewire_channels(&mut self, channels: WorldChannels) {
        self.channels = channels;
        self.input
            .set_game_event_tx(self.channels.game_event_tx.clone());
    }

    /// Retrieves a reference to a game object by its ID
    pub fn get_object(&self, obj: GameObjectId) -> Option<&GameObject> {
        self.objects
            .get(obj)
            .and_then(|o| o.is_alive().then_some(o.as_ref()))
    }

    /// Retrieves a mutable reference to a game object by its ID
    pub fn get_object_mut(&mut self, obj: GameObjectId) -> Option<&mut GameObject> {
        if !self.objects.get(obj).is_some_and(|o| o.is_alive()) {
            return None;
        }
        self.objects.get_mut(obj).map(|o| o.as_mut())
    }

    /// Retrieves a strong reference to a game object by its ID, keeping it alive while the reference exists.
    pub fn get_object_ref(&mut self, obj: GameObjectId) -> Option<GameObjectRef> {
        GameObjectRef::new_in_world(self, obj)
    }

    /// Internal: retain a strong reference to keep a game object alive.
    pub(crate) fn retain_object(&mut self, obj: GameObjectId) -> bool {
        let Some(entry) = self.objects.get(obj) else {
            return false;
        };
        let has_refs = self.object_ref_counts.get(&obj).copied().unwrap_or(0) > 0;
        if !entry.is_alive() && !has_refs {
            return false;
        }

        *self.object_ref_counts.entry(obj).or_insert(0) += 1;
        true
    }

    pub(crate) fn has_object_refs(&self, obj: GameObjectId) -> bool {
        self.object_ref_counts.get(&obj).copied().unwrap_or(0) > 0
    }

    /// Internal: release a strong reference and destroy the object if needed.
    pub(crate) fn release_object(&mut self, obj: GameObjectId) {
        let Some(count) = self.object_ref_counts.get_mut(&obj) else {
            return;
        };

        *count = count.saturating_sub(1);
        if *count != 0 {
            return;
        }

        self.object_ref_counts.remove(&obj);
        if self.pending_deletions.remove(&obj) {
            self.finalize_object_removal(obj);
        }
    }

    fn finalize_object_removal(&mut self, obj: GameObjectId) {
        self.pending_deletions.remove(&obj);
        self.object_ref_counts.remove(&obj);
        if let Some(existing) = self.objects.get(obj) {
            self.click_listeners.remove(&obj);
            self.release_object_hash(existing.hash);
        }
        self.detach_relationships(obj);
        self.objects.remove(obj);
    }

    pub(crate) fn schedule_object_removal(&mut self, obj: GameObjectId) {
        if !self.objects.contains_key(obj) {
            return;
        }

        if !self.pending_deletions.insert(obj) {
            return;
        }

        if self.object_ref_counts.get(&obj).copied().unwrap_or(0) == 0 {
            self.finalize_object_removal(obj);
        }
    }

    fn detach_relationships(&mut self, obj: GameObjectId) {
        let parent = self.objects.get(obj).and_then(|o| o.parent);
        if let Some(parent_id) = parent {
            if let Some(parent_obj) = self.objects.get_mut(parent_id)
                && let Some(pos) = parent_obj.children.iter().position(|c| *c == obj)
            {
                parent_obj.children.remove(pos);
            }
        } else if let Some(pos) = self.children.iter().position(|c| *c == obj) {
            self.children.remove(pos);
        }
    }

    pub(crate) fn update_event_registration(
        &mut self,
        obj: GameObjectId,
        old: EventType,
        new: EventType,
    ) {
        if old.contains(EventType::CLICK) && !new.contains(EventType::CLICK) {
            self.click_listeners.remove(&obj);
        }

        if !old.contains(EventType::CLICK) && new.contains(EventType::CLICK) {
            self.click_listeners.insert(obj);
        }
    }

    fn allocate_object_hash(&mut self, id: GameObjectId) -> ObjectHash {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut seed = id.as_ffi();
        loop {
            let mut hasher = DefaultHasher::default();
            hasher.write_u64(seed);
            let candidate = (hasher.finish() & 0xffff_ffff) as ObjectHash;
            let hash = if candidate == 0 { 1 } else { candidate };

            if self.object_hashes.insert(hash) {
                return hash;
            }

            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
    }

    fn release_object_hash(&mut self, hash: ObjectHash) {
        self.object_hashes.remove(&hash);
    }

    /// Creates a new game object with the given name
    pub fn new_object<S: Into<String>>(&mut self, name: S) -> GameObjectId {
        let obj = GameObject {
            id: GameObjectId::null(),
            name: name.into(),
            alive: Cell::new(true),
            enabled: Cell::new(true),
            children: vec![],
            parent: None,
            owning_world: self,
            transform: Transform::new(GameObjectId::null()),
            components: Vec::new(),
            custom_properties: HashMap::new(),
            event_mask: Cell::new(EventType::empty()),
            hash: 0,
        };

        let id = self.objects.insert(Box::new(obj));
        let hash = self.allocate_object_hash(id);

        let entry = self
            .objects
            .get_mut(id)
            .expect("object was just inserted and must exist");
        entry.id = id;
        entry.transform.owner = id;
        entry.hash = hash;

        id
    }

    /// Creates a new camera game object
    ///
    /// If no active camera exists yet, this camera will be set as the active camera
    /// and added as a child of the world.
    pub fn new_camera(&mut self) -> CRef<CameraComponent> {
        let obj = CameraPrefab.build(self);
        let camera = obj
            .get_component::<CameraComponent>()
            .expect("CameraPrefab should always attach a camera to itself");

        if !self.main_active_camera.exists(self) {
            self.add_child(obj);
            self.set_active_camera(camera.clone());
        }

        camera
    }

    pub fn set_active_camera(&mut self, mut camera: CRef<CameraComponent>) {
        camera.set_render_target(ViewportId::PRIMARY);
        self.main_active_camera = camera.downgrade();
        self.channels
            .set_active_camera(ViewportId::PRIMARY, self.main_active_camera);
    }

    pub fn set_active_camera_for_target(
        &mut self,
        target: ViewportId,
        mut camera: CRef<CameraComponent>,
    ) {
        camera.set_render_target(target);
        self.channels.set_active_camera(target, camera.downgrade());
    }

    fn active_camera_for_target(&self, target: ViewportId) -> Option<CWeak<CameraComponent>> {
        let target_cam = self.channels.active_camera_for(target);
        if target_cam.exists(self) {
            Some(target_cam)
        } else if target == ViewportId::PRIMARY {
            Some(self.main_active_camera)
        } else {
            None
        }
    }

    pub fn set_viewport_size(&mut self, target: ViewportId, size: PhysicalSize<u32>) {
        self.channels.set_viewport_size(target, size);
        if let Some(mut cam) = self
            .active_camera_for_target(target)
            .and_then(|c| c.upgrade(self))
        {
            cam.resize(size.width as f32, size.height as f32);
        }
    }

    pub fn viewport_size(&self, target: ViewportId) -> Option<PhysicalSize<u32>> {
        self.channels.viewports.get(&target).map(|t| t.size)
    }

    pub fn create_window(&mut self) -> ViewportId {
        self.create_window_with_size(PhysicalSize::new(800, 600))
    }

    pub fn create_window_with_size(&mut self, size: PhysicalSize<u32>) -> ViewportId {
        let target_id = self.channels.add_window(CWeak::null(), size);
        let _ = self
            .channels
            .game_event_tx
            .send(GameAppEvent::AddWindow(target_id, size));
        target_id
    }

    pub fn active_camera(&self) -> CWeak<CameraComponent> {
        self.main_active_camera
    }

    pub fn is_listening_for(&self, obj: GameObjectId, event: EventType) -> bool {
        match event {
            EventType::CLICK => self.click_listeners.contains(&obj),
            _ => false,
        }
    }

    fn process_pick_results(&mut self) {
        while let Ok(result) = self.channels.pick_result_rx.try_recv() {
            let Some(obj_hash) = result.hash else {
                continue;
            };

            let Some((obj, _)) = self
                .objects
                .iter()
                .find(|(_, o)| o.object_hash() == obj_hash)
            else {
                continue;
            };

            if !obj.is_alive() || !obj.is_notified_for(EventType::CLICK) {
                continue;
            }

            let components = obj.components.clone();
            let world = self as *mut World;

            for mut comp in components {
                unsafe { comp.on_click(&mut *world) }
            }
        }
    }

    fn maybe_request_pick(&mut self) {
        if self.click_listeners.is_empty() || self.input.is_cursor_locked() {
            return;
        }

        if !self.input.is_button_down(MouseButton::Left) {
            return;
        }

        let target = self.input.active_target();
        let Some(size) = self.viewport_size(target) else {
            return;
        };
        if size.width == 0 || size.height == 0 {
            return;
        }

        let pos = self.input.mouse_position();
        let x = pos.x.max(0.0).floor() as u32;
        let y = pos.y.max(0.0).floor() as u32;
        let clamped_x = x.min(size.width.saturating_sub(1));
        let clamped_y = y.min(size.height.saturating_sub(1));

        let request = PickRequest {
            id: self.next_pick_request_id,
            target,
            position: (clamped_x, clamped_y),
        };
        self.next_pick_request_id = self.next_pick_request_id.wrapping_add(1);

        let _ = self
            .channels
            .render_tx
            .send(RenderMsg::PickRequest(request));
    }

    /// Adds a game object as a child of the world (root level)
    ///
    /// This removes any existing parent relationship the object might have.
    pub fn add_child(&mut self, obj: impl AsRef<GameObjectId>) {
        let mut obj = *obj.as_ref();
        self.children.push(obj);
        obj.parent = None;
    }

    /// Spawns a game object from a prefab
    pub fn spawn<P: Prefab>(&mut self, prefab: &P) -> GameObjectId {
        prefab.spawn(self)
    }

    /// Executes a component function on all components of all game objects
    pub(crate) fn execute_component_func<F>(&mut self, func: F)
    where
        F: Fn(&mut dyn Component, &mut World),
    {
        let world = unsafe { &mut *(self as *mut World) };
        self.objects
            .values()
            .filter(|o| o.enabled.get())
            .flat_map(|o| &o.components)
            .for_each(|c| func(c.get_mut(), world))
    }

    /// Runs possible physics update if the timestep time has elapsed yet
    pub fn fixed_update(&mut self) {
        while self.physics.is_due() {
            self.execute_component_func(Component::fixed_update);

            self.physics.step();

            self.execute_component_func(Component::post_fixed_update);
        }

        let rem = self.physics.current_timepoint.elapsed();
        self.physics.alpha =
            (rem.as_secs_f32() / self.physics.timestep.as_secs_f32()).clamp(0.0, 1.0);
    }

    /// Updates all game objects and their components
    ///
    /// It will tick delta time and update all components
    ///
    /// If you're using the App runtime, this will be handled for you. Only call this function
    /// if you are trying to use a detached world context.
    pub fn update(&mut self) {
        self.process_pick_results();
        self.maybe_request_pick();
        self.execute_component_func(Component::update);
        self.execute_component_func(Component::late_update);
    }

    /// Performs late update operations after the main update
    ///
    /// If you're using the App runtime, this will be handled for you. Only call this function
    /// if you are trying to use a detached world context.
    pub fn post_update(&mut self) {
        let world = self as *mut World;
        self.execute_component_func(Component::post_update);

        for mut comp in self.components.iter_refs() {
            let ctx = UiContext::new(comp.ctx.parent.hash, comp.ctx.tid);
            unsafe {
                comp.on_gui(&mut *world, ctx);
            }
        }

        self.sync_fresh_components();
        self.sync_removed_components();

        let mut command_batch = Vec::with_capacity(self.components.len());

        for (_, obj) in self.objects.iter() {
            if !obj.is_alive() || !obj.transform.is_dirty() {
                continue;
            }
            for comp in obj.components.iter() {
                command_batch.push(RenderMsg::UpdateTransform(
                    comp.typed_id(),
                    obj.transform.affine(),
                ));
            }
        }
        for (ctid, comp) in self.components.iter_mut() {
            let ctx = CPUDrawCtx::new(ctid, &mut command_batch);
            unsafe {
                comp.update_proxy(&*world, ctx);
            }
        }

        for target_id in self.channels.viewports.keys().copied() {
            if let Some(mut camera) = self
                .active_camera_for_target(target_id)
                .and_then(|c| c.upgrade(self))
            {
                Self::push_camera_updates(target_id, &mut command_batch, &mut camera);
            }
        }

        self.strobe.sort();

        self.channels
            .render_tx
            .send(RenderMsg::CommandBatch(command_batch))
            .unwrap();

        let _ = self
            .channels
            .render_tx
            .send(RenderMsg::UpdateStrobe(mem::take(&mut self.strobe)));
    }

    fn push_camera_updates(
        target_id: ViewportId,
        batch: &mut Vec<RenderMsg>,
        active_camera: &mut CRef<CameraComponent>,
    ) {
        let obj = active_camera.parent();
        if obj.transform.is_dirty() {
            let pos = obj.transform.position();
            let view_mat = obj.transform.view_matrix_rigid().to_mat4();
            batch.push(RenderMsg::UpdateActiveCamera(
                target_id,
                Box::new(move |cam| {
                    cam.view_mat = view_mat;
                    cam.proj_view_mat = cam.projection_mat * cam.view_mat;
                    cam.inv_proj_view_mat = cam.proj_view_mat.inverse();
                    cam.pos = pos;
                }),
            ));
        }

        if active_camera.is_projection_dirty() {
            let proj_mat = active_camera.projection;
            let fov = active_camera.fov();
            let near = active_camera.near();
            let far = active_camera.fov();
            let fov_target = active_camera.fov_target();
            let zoom_speed = active_camera.zoom_speed;
            batch.push(RenderMsg::UpdateActiveCamera(
                target_id,
                Box::new(move |cam| {
                    cam.projection_mat = proj_mat;
                    cam.fov = fov;
                    cam.proj_view_mat = cam.projection_mat * cam.view_mat;
                    cam.inv_proj_view_mat = cam.proj_view_mat.inverse();
                    cam.near = near;
                    cam.far = far;
                    cam.fov_target = fov_target;
                    cam.zoom_speed = zoom_speed;
                }),
            ));
            active_camera.clear_projection_dirty();
        }
    }

    /// Internally sync removed components to the Render Thread for proxy deletion
    fn sync_removed_components(&mut self) {
        if self.components.removed.is_empty() {
            return;
        }

        let mut removed = Vec::new();
        swap(&mut removed, &mut self.components.removed);

        for ctid in removed {
            self.channels
                .render_tx
                .send(RenderMsg::RemoveProxy(ctid))
                .unwrap();
        }
    }

    /// Internally sync new components to the Render Thread for proxy creation
    fn sync_fresh_components(&mut self) {
        if self.components.fresh.is_empty() {
            return;
        }

        let mut fresh = Vec::new();
        swap(&mut fresh, &mut self.components.fresh);
        for cid in fresh {
            let Some(mut comp) = self.components.get_dyn(cid) else {
                continue;
            };

            let local_to_world = comp.parent().transform.affine();
            if let Some(proxy) = comp.create_render_proxy(self) {
                self.channels
                    .render_tx
                    .send(RenderMsg::RegisterProxy(
                        cid,
                        comp.parent().object_hash(),
                        proxy,
                        local_to_world,
                    ))
                    .unwrap();
            }
            if let Some(proxy) = comp.create_light_proxy(self) {
                self.channels
                    .render_tx
                    .send(RenderMsg::RegisterLightProxy(cid, proxy))
                    .unwrap();
            }
        }
    }

    /// Prepares for the next frame by resetting the input state
    ///
    /// If you're using the App runtime, this will be handled for you. Only call this function
    /// if you are trying to use a detached world context.
    pub fn next_frame(&mut self) {
        for child in self.objects.values_mut() {
            if child.is_alive() {
                child.transform.clear_dirty();
            }
        }
        self.input.next_frame_all();
        self.tick_delta_time();
    }

    /// Finds a game object by its name
    ///
    /// Note: If multiple objects have the same name, only the first one found will be returned.
    pub fn find_object_by_name(&self, name: &str) -> Option<GameObjectId> {
        self.objects
            .iter()
            .find(|(_, o)| o.is_alive() && o.name == name)
            .map(|o| o.0)
    }

    /// Gets all components of a specific type from all game objects in the world
    ///
    /// This method recursively traverses the entire scene graph to find all components
    /// of the specified type.
    pub fn find_all_components_of_type<C: Component + 'static>(&self) -> Vec<CRef<C>> {
        let mut collection = Vec::new();

        for child in &self.children {
            Self::find_components_of_children(&mut collection, *child);
        }

        collection
    }

    /// Helper method to recursively collect components of a specific type from a game object and its children
    fn find_components_of_children<C: Component + 'static>(
        collection: &mut Vec<CRef<C>>,
        obj: GameObjectId,
    ) {
        if !obj.exists() {
            return;
        }

        for child in &obj.children {
            Self::find_components_of_children(collection, *child);
        }

        collection.extend(obj.iter_components::<C>());
    }

    /// Find all objects that contain a property with the given key
    pub fn find_objects_with_property(&self, key: &str) -> Vec<GameObjectId> {
        self.objects
            .iter()
            .filter_map(|(id, o)| (o.is_alive() && o.has_property(key)).then_some(id))
            .collect()
    }

    /// Find all objects that contain a property with the given key and value
    pub fn find_objects_with_property_value(&self, key: &str, value: &Value) -> Vec<GameObjectId> {
        self.objects
            .iter()
            .filter_map(|(id, o)| (o.is_alive() && o.property(key)? == value).then_some(id))
            .collect()
    }

    pub fn capture_offscreen_textures(&self, target: ViewportId, path: impl Into<PathBuf>) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::CaptureOffscreenTextures(target, path.into()))
            .is_ok()
    }

    pub fn capture_picking_texture(&self, target: ViewportId, path: impl Into<PathBuf>) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::CapturePickingTexture(target, path.into()))
            .is_ok()
    }

    /// Prints information about all game objects in the world to the log
    ///
    /// This method will print out the scene graph to the console and add some information about
    /// components and drawables attached to the objects.
    pub fn print_objects(&self) {
        let alive = self.objects.values().filter(|o| o.is_alive()).count();
        info!("{alive} game objects in world.");
        print_objects_rec(&self.children, 0)
    }

    /// Updates the delta time based on the elapsed time since the last frame
    fn tick_delta_time(&mut self) {
        self.delta_time = self.last_frame_time.elapsed();
        self.last_frame_time = Instant::now();
    }

    /// Returns the time elapsed since the last frame
    pub fn delta_time(&self) -> Duration {
        self.delta_time
    }

    /// Returns the instant in time when the world was created
    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    /// Returns the total time elapsed since the world was created
    pub fn time(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Marks a game object for deletion. This will immediately run the object internal destruction routine
    /// and also clean up any component-specific data.
    pub fn delete_object(&mut self, object: GameObjectId) {
        if let Some(obj) = self.objects.get_mut(object)
            && obj.is_alive()
        {
            obj.delete();
        }
    }

    /// Internal method to unlink and remove a game object from the world
    ///
    /// This method will remove the object from the world's children list if it's a root-level object,
    /// unlinks it from its parent and children and then remove the object from the world's objects collection
    ///
    /// This is an internal method as it's used in form of a callback from the object,
    /// signaling that its internal destruction routine has been done, which includes its components and
    /// can now be safely unlinked.
    #[allow(dead_code)]
    pub(crate) fn unlink_internal(&mut self, caller: GameObjectId) {
        if let Some(obj) = self.objects.get_mut(caller) {
            obj.mark_dead();
        }
        self.schedule_object_removal(caller);
    }

    pub fn set_default_window_title(&mut self, title: String) {
        self.set_window_title(ViewportId::PRIMARY, title);
    }

    pub fn set_window_title(&mut self, target_id: ViewportId, title: String) {
        let _ = self
            .channels
            .game_event_tx
            .send(GameAppEvent::UpdateWindowTitle(target_id, title));
    }

    /// Requests a shutdown of the world
    ///
    /// The world might not shut down immediately as cleanup will be started after this.
    pub fn shutdown(&mut self) {
        if self.requested_shutdown {
            return;
        }
        self.requested_shutdown = true;
        self.teardown();
        let _ = self.channels.game_event_tx.send(GameAppEvent::Shutdown);
    }

    /// `true` if a shutdown has been requested, `false` otherwise
    pub fn is_shutting_down(&self) -> bool {
        self.requested_shutdown
    }

    /// Cleanly tears down all world data. Intended to be used during shutdown.
    pub fn teardown(&mut self) {
        // mark everything dead and remove components so render proxies get torn down
        let ids: Vec<_> = self.objects.keys().collect();
        for id in ids {
            if let Some(obj) = self.objects.get_mut(id) {
                obj.mark_dead();
                let comps: Vec<_> = obj.components.drain(..).collect();
                obj.children.clear();
                obj.parent = None;

                for mut comp in comps {
                    comp.delete(self);
                    self.components.remove(&comp);
                }
            }
            self.pending_deletions.insert(id);
        }

        self.sync_removed_components();
        self.object_ref_counts.clear();
        self.children.clear();
        self.objects.clear();
        self.components = ComponentStorage::default();
        self.click_listeners.clear();
        self.object_hashes.clear();
        self.next_pick_request_id = 0;
        self.pending_deletions.clear();
    }
}

impl AsRef<Store<Mesh>> for World {
    fn as_ref(&self) -> &Store<Mesh> {
        &self.assets.meshes
    }
}

impl AsRef<Store<Shader>> for World {
    fn as_ref(&self) -> &Store<Shader> {
        &self.assets.shaders
    }
}

impl AsRef<Store<Texture2D>> for World {
    fn as_ref(&self) -> &Store<Texture2D> {
        &self.assets.textures
    }
}

impl AsRef<Store<Texture2DArray>> for World {
    fn as_ref(&self) -> &Store<Texture2DArray> {
        &self.assets.texture_arrays
    }
}

impl AsRef<Store<RenderTexture2D>> for World {
    fn as_ref(&self) -> &Store<RenderTexture2D> {
        &self.assets.render_textures
    }
}

impl AsRef<Store<RenderTexture2DArray>> for World {
    fn as_ref(&self) -> &Store<RenderTexture2DArray> {
        &self.assets.render_texture_arrays
    }
}

impl AsRef<Store<Material>> for World {
    fn as_ref(&self) -> &Store<Material> {
        &self.assets.materials
    }
}

impl AsRef<Store<BGL>> for World {
    fn as_ref(&self) -> &Store<BGL> {
        &self.assets.bgls
    }
}

impl AsRef<Store<Sound>> for World {
    fn as_ref(&self) -> &Store<Sound> {
        &self.assets.sounds
    }
}

fn print_objects_rec(children: &Vec<GameObjectId>, i: i32) {
    for child in children {
        if !child.exists() {
            continue;
        }
        info!("{}- {}", "  ".repeat(i as usize), &child.name);
        info!(
            "{}-> Components: {}",
            "  ".repeat(i as usize + 1),
            child.components.len()
        );
        print_objects_rec(&child.children, i + 1);
    }
}
