//! The [`World`] struct stores and updates all game objects. Its use is to manage any
//! "raw" data, store and provide access to the objects and behavior, with a focus on ease of use.
//!
//! It maintains the scene graph, input state and physics simulation and
//! offers utilities such as methods to create, find and remove game objects.

use crate::audio::AudioScene;
use crate::components::{CRef, CWeak, CameraComponent, Component, UiContext};
use crate::core::component_storage::ComponentStorage;
use crate::core::{EventType, GameObject, GameObjectId, GameObjectRef, ObjectHash, Transform};
use crate::engine::prefabs::Prefab;
use crate::game_thread::GameAppEvent;
use crate::input::InputManager;
use crate::physics::PhysicsSimulation;
use crate::prefabs::CameraPrefab;
use slotmap::{Key, SlotMap};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::mem::{swap, take};
use std::path::PathBuf;
use std::sync::Arc;
use syrillian_asset::store::{AssetRefreshMessage, Store};
use syrillian_asset::{AssetStore, ComputeShader};
use syrillian_asset::{
    BGL, Cubemap, HCubemap, Material, MaterialInstance, Mesh, RenderCubemap, RenderTexture2D,
    RenderTexture2DArray, Shader, Sound, Texture2D, Texture2DArray,
};
use syrillian_render::strobe::StrobeFrame;
use syrillian_render::strobe::input::{HitRect, StrobeInputState};
use tracing::info;
use web_time::{Duration, Instant};

use crate::core::reflection::Value;
use crate::math::UVec2;
use crossbeam_channel::unbounded;
use crossbeam_channel::{Receiver, Sender};
use syrillian_macros::Reflect;
use syrillian_reflect::type_info;
use syrillian_render::rendering::CPUDrawCtx;
use syrillian_render::rendering::message::{GBufferDebugTargets, RenderMsg};
use syrillian_render::rendering::picking::{PickRequest, PickResult};
use syrillian_render::rendering::render_data::{SkyAtmosphereSettings, SkyboxMode};
use syrillian_render::rendering::viewport::ViewportId;
use syrillian_utils::{EngineArgs, TypedComponentId};
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

pub type FreshWorldChannels = (
    Box<World>,
    Receiver<RenderMsg>,
    Receiver<GameAppEvent>,
    Receiver<AssetRefreshMessage>,
    Sender<PickResult>,
    Sender<Vec<HitRect>>,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickResultState {
    pub request_id: u64,
    pub target: ViewportId,
    pub hash: Option<ObjectHash>,
    pub object: Option<GameObjectId>,
}

#[derive(Clone)]
pub struct WorldChannels {
    pub render_tx: Sender<RenderMsg>,
    pub game_event_tx: Sender<GameAppEvent>,
    pub pick_result_rx: Receiver<PickResult>,
    pub hit_rect_rx: Receiver<Vec<HitRect>>,
    viewports: HashMap<ViewportId, RenderTargets>,
    next_target_id: u64,
}

impl WorldChannels {
    pub fn new(
        render_tx: Sender<RenderMsg>,
        game_event_tx: Sender<GameAppEvent>,
        pick_result_rx: Receiver<PickResult>,
        hit_rect_rx: Receiver<Vec<HitRect>>,
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
            hit_rect_rx,
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
    /// Components that are awaiting storage removal at the end of a component phase
    pending_component_removals: HashSet<TypedComponentId>,
    /// Tracks nested component phase execution depth.
    component_phase_depth: usize,
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
    /// When true, component `on_gui` callbacks are suppressed (e.g. editor mode)
    pub suppress_component_gui: bool,
    /// When true, automatic `on_click` picking is suppressed.
    /// Explicit pick requests still store their result for request-id lookup.
    pub suppress_pick_callbacks: bool,
    tracked_pick_requests: HashSet<u64>,
    callback_pick_requests: HashSet<u64>,
    pending_pick_results: HashMap<u64, PickResultState>,
    /// Current viewport sub-rect for coordinate adjustment during picking.
    /// Set by the editor when using set_viewport_rect.
    pub pick_viewport_rect: Option<[f32; 4]>,
    pub(crate) channels: WorldChannels,
    thread_binding: Option<WorldBinding>,
    pub strobe: StrobeFrame,
    pub strobe_input: StrobeInputState,
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
            pending_component_removals: HashSet::new(),
            component_phase_depth: 0,
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
            suppress_component_gui: false,
            suppress_pick_callbacks: false,
            tracked_pick_requests: HashSet::new(),
            callback_pick_requests: HashSet::new(),
            pending_pick_results: HashMap::new(),
            pick_viewport_rect: None,
            channels,
            thread_binding: None,
            strobe: StrobeFrame::default(),
            strobe_input: StrobeInputState::default(),
        })
    }

    /// Creates a new world through World::empty and binds it to the current thread.
    pub fn new(
        assets: Arc<AssetStore>,
        render_tx: Sender<RenderMsg>,
        game_event_tx: Sender<GameAppEvent>,
        pick_result_rx: Receiver<PickResult>,
        hit_rect_rx: Receiver<Vec<HitRect>>,
    ) -> Box<World> {
        let channels = WorldChannels::new(render_tx, game_event_tx, pick_result_rx, hit_rect_rx);
        World::new_with_channels(assets, channels)
    }

    pub fn new_with_channels(assets: Arc<AssetStore>, channels: WorldChannels) -> Box<World> {
        let mut world = World::empty(channels, assets);
        world.bind_thread();
        world
    }

    /// View [`World::new`]. This function will just set up data structures around the world
    /// needed for initialization. Mostly useful for tests.
    pub fn fresh() -> FreshWorldChannels {
        let (tx1, rx1) = unbounded();
        let (tx2, rx2) = unbounded();
        let (pick_tx, pick_rx) = unbounded();
        let (hit_rect_tx, hit_rect_rx) = unbounded();
        let (store, assets_rx) = AssetStore::new();
        let world = World::new(store, tx1, tx2, pick_rx, hit_rect_rx);
        (world, rx1, rx2, assets_rx, pick_tx, hit_rect_tx)
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
    #[profiling::function]
    pub fn bind_thread(&mut self) {
        self.thread_binding = Some(WorldBinding::bind(self));
    }

    /// Replace the channels used to communicate with the render and windowing threads.
    ///
    /// This lets a world be moved between render targets without reconstructing it.
    #[profiling::function]
    pub fn rewire_channels(&mut self, channels: WorldChannels) {
        self.channels = channels;
        self.input
            .set_game_event_tx(self.channels.game_event_tx.clone());
    }

    /// Retrieves a reference to a game object by its ID
    #[profiling::function]
    pub fn get_object(&self, obj: GameObjectId) -> Option<&GameObject> {
        self.objects
            .get(obj)
            .and_then(|o| o.is_alive().then_some(o.as_ref()))
    }

    /// Retrieves a mutable reference to a game object by its ID
    #[profiling::function]
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
    #[profiling::function]
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
    #[profiling::function]
    pub(crate) fn release_object(&mut self, obj: GameObjectId) {
        let Some(count) = self.object_ref_counts.get_mut(&obj) else {
            return;
        };

        *count = count.saturating_sub(1);
        if *count != 0 {
            return;
        }

        self.object_ref_counts.remove(&obj);
        if self.pending_deletions.contains(&obj) {
            if self.is_in_component_phase() {
                return;
            }
            self.finish_scheduled_object_teardown(obj);
            self.finalize_object_removal(obj);
        }
    }

    #[profiling::function]
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

    pub(crate) fn is_in_component_phase(&self) -> bool {
        self.component_phase_depth != 0
    }

    fn begin_component_phase(&mut self) {
        self.component_phase_depth += 1;
    }

    fn end_component_phase(&mut self) {
        if self.component_phase_depth == 0 {
            return;
        }

        self.component_phase_depth -= 1;
        if self.component_phase_depth != 0 {
            return;
        }

        self.flush_scheduled_component_removals();
        self.flush_scheduled_object_removals();
    }

    pub(crate) fn schedule_component_removal(&mut self, ctid: TypedComponentId) {
        if self.is_in_component_phase() {
            self.pending_component_removals.insert(ctid);
            return;
        }

        self.components.remove(ctid);
    }

    fn flush_scheduled_component_removals(&mut self) {
        if self.pending_component_removals.is_empty() {
            return;
        }

        let pending = take(&mut self.pending_component_removals);
        for ctid in pending {
            self.components.remove(ctid);
        }
    }

    fn finish_scheduled_object_teardown(&mut self, obj: GameObjectId) {
        let components = if let Some(object) = self.objects.get_mut(obj) {
            if object.components.is_empty() {
                return;
            }
            take(&mut object.components)
        } else {
            return;
        };

        let world = self as *mut World;
        for mut comp in components {
            unsafe {
                comp.delete(&mut *world);
            }
            self.components.remove(&comp);
        }
    }

    fn flush_scheduled_object_removals(&mut self) {
        if self.is_in_component_phase() || self.pending_deletions.is_empty() {
            return;
        }

        let pending: Vec<_> = self.pending_deletions.iter().copied().collect();
        for obj in pending {
            if !self.objects.contains_key(obj) {
                self.pending_deletions.remove(&obj);
                continue;
            }

            self.finish_scheduled_object_teardown(obj);

            if self.object_ref_counts.get(&obj).copied().unwrap_or(0) == 0 {
                self.finalize_object_removal(obj);
            }
        }
    }

    #[profiling::function]
    pub(crate) fn schedule_object_removal(&mut self, obj: GameObjectId) {
        if !self.objects.contains_key(obj) {
            return;
        }

        if !self.pending_deletions.insert(obj) {
            return;
        }

        if self.is_in_component_phase() {
            return;
        }

        self.finish_scheduled_object_teardown(obj);
        if self.object_ref_counts.get(&obj).copied().unwrap_or(0) == 0 {
            self.finalize_object_removal(obj);
        }
    }

    #[profiling::function]
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

    #[profiling::function]
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
    #[profiling::function]
    pub fn new_object<S: Into<String>>(&mut self, name: S) -> GameObjectId {
        let obj = GameObject {
            id: GameObjectId::null(),
            name: name.into(),
            alive: Cell::new(true),
            enabled: Cell::new(true),
            self_disabled: Cell::new(false),
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
        let next_camera = camera.clone().downgrade();
        if self.main_active_camera != next_camera {
            camera.force_render_sync();
        }
        camera.set_render_target(ViewportId::PRIMARY);
        self.main_active_camera = next_camera;
        self.channels
            .set_active_camera(ViewportId::PRIMARY, self.main_active_camera);
    }

    pub fn set_active_camera_for_target(
        &mut self,
        target: ViewportId,
        mut camera: CRef<CameraComponent>,
    ) {
        let next_camera = camera.clone().downgrade();
        if self.channels.active_camera_for(target) != next_camera {
            camera.force_render_sync();
        }
        camera.set_render_target(target);
        self.channels.set_active_camera(target, next_camera);
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
        self.create_window_with_size(EngineArgs::default_window_size())
    }

    pub fn create_window_with_size(&mut self, size: UVec2) -> ViewportId {
        let size = PhysicalSize::new(size.x, size.y);
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

    #[profiling::function]
    fn process_pick_results(&mut self) {
        while let Ok(result) = self.channels.pick_result_rx.try_recv() {
            let should_notify_callbacks = self.callback_pick_requests.remove(&result.id);
            let picked = result.hash.and_then(|obj_hash| {
                self.objects
                    .iter()
                    .find(|(_, o)| o.object_hash() == obj_hash)
                    .map(|(picked_id, obj)| {
                        (
                            picked_id,
                            obj.is_alive(),
                            obj.is_notified_for(EventType::CLICK),
                            obj.components.clone(),
                        )
                    })
            });
            let picked_object = picked.as_ref().map(|(picked_id, _, _, _)| *picked_id);

            if self.tracked_pick_requests.remove(&result.id) {
                self.pending_pick_results.insert(
                    result.id,
                    PickResultState {
                        request_id: result.id,
                        target: result.target,
                        hash: result.hash,
                        object: picked_object,
                    },
                );
            }

            let Some((_, is_alive, is_notified_for_click, components)) = picked else {
                continue;
            };

            if !should_notify_callbacks {
                continue;
            }

            // Skip on_click callbacks when suppressed (editor scene view)
            if self.suppress_pick_callbacks {
                continue;
            }

            // For normal game picking, only notify objects that registered for clicks
            if !is_alive || !is_notified_for_click {
                continue;
            }

            let world = self as *mut World;

            for mut comp in components {
                unsafe { comp.on_click(&mut *world) }
            }
        }
    }

    pub fn take_pick_result(&mut self, request_id: u64) -> Option<PickResultState> {
        self.pending_pick_results.remove(&request_id)
    }

    pub fn cancel_pick_result(&mut self, request_id: u64) {
        self.tracked_pick_requests.remove(&request_id);
        self.callback_pick_requests.remove(&request_id);
        self.pending_pick_results.remove(&request_id);
    }

    #[profiling::function]
    fn maybe_request_pick(&mut self) {
        if self.suppress_pick_callbacks || self.click_listeners.is_empty() {
            return;
        }

        if !self.input.is_button_down(MouseButton::Left) {
            return;
        }

        let target = self.input.active_target();
        let Some(position) = self.cursor_pick_position(target) else {
            return;
        };
        let _ = self.send_pick_request(target, position, false, true);
    }

    fn cursor_pick_position(&self, target: ViewportId) -> Option<(u32, u32)> {
        if self.input.is_cursor_locked() {
            return None;
        }

        let size = self.viewport_size(target)?;
        if size.width == 0 || size.height == 0 {
            return None;
        }

        let pos = self.input.mouse_position();
        let (x, y) = if let Some([vp_x, vp_y, vp_w, vp_h]) = self.pick_viewport_rect {
            // Transform screen coords to picking texture coords:
            // Screen-relative to viewport -> absolute picking texture coords.
            // The scene is rendered into a sub-rect of the full-size picking texture.
            let rel_x = (pos.x - vp_x) / vp_w; // 0..1 within viewport
            let rel_y = (pos.y - vp_y) / vp_h;
            if !(0.0..=1.0).contains(&rel_x) || !(0.0..=1.0).contains(&rel_y) {
                return None; // Click outside viewport area
            }
            let tex_x = vp_x + rel_x * vp_w;
            let tex_y = vp_y + rel_y * vp_h;
            (tex_x.floor() as u32, tex_y.floor() as u32)
        } else {
            (pos.x.max(0.0).floor() as u32, pos.y.max(0.0).floor() as u32)
        };
        let clamped_x = x.min(size.width.saturating_sub(1));
        let clamped_y = y.min(size.height.saturating_sub(1));

        Some((clamped_x, clamped_y))
    }

    pub fn request_pick_at_cursor(&mut self) -> Option<u64> {
        let target = self.input.active_target();
        let position = self.cursor_pick_position(target)?;
        self.request_pick(target, position)
    }

    pub fn request_pick_at(&mut self, target: ViewportId, x: f32, y: f32) -> Option<u64> {
        let size = self.viewport_size(target)?;
        if size.width == 0 || size.height == 0 {
            return None;
        }

        let x = x.max(0.0).floor() as u32;
        let y = y.max(0.0).floor() as u32;
        let position = (
            x.min(size.width.saturating_sub(1)),
            y.min(size.height.saturating_sub(1)),
        );
        self.request_pick(target, position)
    }

    pub fn request_pick(&mut self, target: ViewportId, position: (u32, u32)) -> Option<u64> {
        self.send_pick_request(target, position, true, false)
    }

    fn send_pick_request(
        &mut self,
        target: ViewportId,
        position: (u32, u32),
        track_result: bool,
        notify_callbacks: bool,
    ) -> Option<u64> {
        let request = PickRequest {
            id: self.next_pick_request_id,
            target,
            position,
        };
        let request_id = request.id;

        self.channels
            .render_tx
            .send(RenderMsg::PickRequest(request))
            .ok()?;
        if track_result {
            self.tracked_pick_requests.insert(request_id);
        }
        if notify_callbacks {
            self.callback_pick_requests.insert(request_id);
        }
        self.next_pick_request_id = self.next_pick_request_id.wrapping_add(1);
        Some(request_id)
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
        self.begin_component_phase();

        let scheduled = self
            .objects
            .values()
            .filter(|obj| obj.is_alive())
            .flat_map(|obj| obj.components.iter().map(|component| component.typed_id()))
            .collect::<Vec<_>>();

        let world = self as *mut World;
        for tid in scheduled {
            if self.pending_component_removals.contains(&tid) {
                continue;
            }

            let Some(component) = self.components.get_dyn(tid) else {
                continue;
            };
            if !component.ctx.is_enabled() {
                continue;
            }

            unsafe {
                func(component.get_mut(), &mut *world);
            }
        }

        self.end_component_phase();
    }

    /// Runs possible physics update if the timestep time has elapsed yet
    #[profiling::function]
    pub fn fixed_update(&mut self) {
        while self.physics.is_due() {
            {
                profiling::scope!("Component fixed_update");
                self.execute_component_func(Component::fixed_update);
            }

            {
                profiling::scope!("Physics Step");
                self.physics.step();
            }

            {
                profiling::scope!("Component post_fixed_update");
                self.execute_component_func(Component::post_fixed_update);
            }
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
    #[profiling::function]
    pub fn update(&mut self) {
        self.process_pick_results();
        self.maybe_request_pick();
        {
            profiling::scope!("Component update");
            self.execute_component_func(Component::update);
        }
        {
            profiling::scope!("Component late_update");
            self.execute_component_func(Component::late_update);
        }
    }

    /// Performs late update operations after the main update
    ///
    /// If you're using the App runtime, this will be handled for you. Only call this function
    /// if you are trying to use a detached world context.
    #[profiling::function]
    pub fn post_update(&mut self) {
        let world = self as *mut World;
        {
            profiling::scope!("Component post_update");
            self.execute_component_func(Component::post_update);
        }

        self.update_strobe_input();
        self.execute_component_on_gui(world);
        self.sync_fresh_components();
        self.sync_removed_components();

        let mut command_batch = Vec::with_capacity(self.components.len());

        self.sync_updated_transforms(&mut command_batch);
        self.sync_component_proxies(world, &mut command_batch);
        self.sync_viewport_cameras(&mut command_batch);

        {
            profiling::scope!("Submit Render command batch");
            self.channels
                .render_tx
                .send(RenderMsg::CommandBatch(command_batch))
                .unwrap();
        }

        {
            profiling::scope!("Update strobe");
            let _ = self
                .channels
                .render_tx
                .send(RenderMsg::UpdateStrobe(take(&mut self.strobe)));
        }
    }

    #[profiling::function]
    fn sync_viewport_cameras(&mut self, command_batch: &mut Vec<RenderMsg>) {
        for target_id in self.channels.viewports.keys().copied() {
            if let Some(mut camera) = self
                .active_camera_for_target(target_id)
                .and_then(|c| c.upgrade(self))
            {
                Self::push_camera_updates(target_id, command_batch, &mut camera);
            }
        }
    }

    #[profiling::function]
    fn sync_component_proxies(&mut self, world: *mut World, command_batch: &mut Vec<RenderMsg>) {
        for (ctid, comp) in self.components.iter_mut() {
            let ctx = CPUDrawCtx::new(ctid, command_batch);
            unsafe {
                comp.update_proxy(&*world, ctx);
            }
        }
    }

    fn update_strobe_input(&mut self) {
        profiling::scope!("Strobe input");
        // Receive hit rects from render thread
        while let Ok(rects) = self.channels.hit_rect_rx.try_recv() {
            self.strobe_input.update_hit_rects(rects);
        }
        let pos = self.input.mouse_position();
        let mouse_pos = glamx::Vec2::new(pos.x, pos.y);
        let mouse_down = self.input.is_button_pressed(MouseButton::Left);
        let just_pressed = self.input.is_button_down(MouseButton::Left);
        let just_released = self.input.is_button_released(MouseButton::Left);
        self.strobe_input
            .begin_frame(mouse_pos, mouse_down, just_pressed, just_released);
    }

    fn execute_component_on_gui(&mut self, world: *mut World) {
        if self.suppress_component_gui {
            return;
        }
        profiling::scope!("Component on_gui");
        for mut comp in self.components.iter_refs() {
            let ctx = UiContext::new(comp.ctx.parent.hash, comp.ctx.tid);
            unsafe {
                comp.on_gui(&mut *world, ctx);
            }
        }
    }

    #[profiling::function]
    fn sync_updated_transforms(&mut self, command_batch: &mut Vec<RenderMsg>) {
        for (_, obj) in self.objects.iter() {
            if !obj.is_alive() || !obj.transform.is_dirty() {
                continue;
            }

            let world_affine = obj.transform.affine();
            let render_affine = obj.transform.render_affine();
            for comp in obj.components.iter() {
                command_batch.push(RenderMsg::UpdateTransform(
                    comp.typed_id(),
                    render_affine.unwrap_or(world_affine),
                    render_affine.map(|_| world_affine),
                ));
            }
        }
    }

    #[profiling::function]
    fn push_camera_updates(
        target_id: ViewportId,
        batch: &mut Vec<RenderMsg>,
        active_camera: &mut CRef<CameraComponent>,
    ) {
        if let Some(transform_update) = active_camera.maybe_transform_update(target_id) {
            batch.push(transform_update);
        }

        if let Some(update) = active_camera.maybe_projection_update(target_id) {
            batch.push(update);
        }
    }

    /// Internally sync removed components to the Render Thread for proxy deletion
    #[profiling::function]
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
    #[profiling::function]
    fn sync_fresh_components(&mut self) {
        if self.components.fresh.is_empty() {
            return;
        }

        let fresh = take(&mut self.components.fresh);
        for cid in fresh {
            let Some(mut comp) = self.components.get_dyn(cid) else {
                continue;
            };

            let world_affine = comp.parent().transform.affine();
            let render_affine = comp.parent().transform.render_affine();
            if let Some(proxy) = comp.create_render_proxy(self) {
                self.channels
                    .render_tx
                    .send(RenderMsg::RegisterProxy(
                        cid,
                        comp.parent().object_hash(),
                        proxy,
                        render_affine.unwrap_or(world_affine),
                        render_affine.map(|_| world_affine),
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
    #[profiling::function]
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
    #[profiling::function]
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
    #[profiling::function]
    pub fn find_all_components_of_type<C: Component + 'static>(&self) -> Vec<CRef<C>> {
        let mut collection = Vec::new();

        for child in &self.children {
            Self::find_components_of_children(&mut collection, *child);
        }

        collection
    }

    /// Helper method to recursively collect components of a specific type from a game object and its children
    #[profiling::function]
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
    #[profiling::function]
    pub fn find_objects_with_property(&self, key: &str) -> Vec<GameObjectId> {
        self.objects
            .iter()
            .filter_map(|(id, o)| (o.is_alive() && o.has_property(key)).then_some(id))
            .collect()
    }

    /// Find all objects that contain a property with the given key and value
    #[profiling::function]
    pub fn find_objects_with_property_value(&self, key: &str, value: &Value) -> Vec<GameObjectId> {
        self.objects
            .iter()
            .filter_map(|(id, o)| (o.is_alive() && o.property(key)? == value).then_some(id))
            .collect()
    }

    #[profiling::function]
    pub fn capture_offscreen_textures(&self, target: ViewportId, path: impl Into<PathBuf>) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::CaptureOffscreenTextures(target, path.into()))
            .is_ok()
    }

    #[profiling::function]
    pub fn capture_picking_texture(&self, target: ViewportId, path: impl Into<PathBuf>) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::CapturePickingTexture(target, path.into()))
            .is_ok()
    }

    pub fn set_gbuffer_debug_targets(
        &self,
        target: ViewportId,
        targets: Option<GBufferDebugTargets>,
    ) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::SetGBufferDebug(target, targets))
            .is_ok()
    }

    /// Set a sub-rect for 3D rendering within the viewport.
    /// When set, the 3D scene renders only in the given [x, y, width, height] region.
    /// UI (Strobe) rendering is not affected. Pass None to render full-viewport.
    pub fn set_viewport_rect(&self, target: ViewportId, rect: Option<[f32; 4]>) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::SetViewportRect(target, rect))
            .is_ok()
    }

    /// Enable or disable post-processing for a specific viewport.
    pub fn set_disable_post_processing(&self, target: ViewportId, disable: bool) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::SetDisablePostProcessing(target, disable))
            .is_ok()
    }

    pub fn set_viewport_skybox(&self, target: ViewportId, cubemap: Option<HCubemap>) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::SetSkybox(target, cubemap))
            .is_ok()
    }

    pub fn set_skybox(&self, cubemap: Option<HCubemap>) -> bool {
        self.set_viewport_skybox(ViewportId::PRIMARY, cubemap)
    }

    pub fn set_viewport_skybox_mode(&self, target: ViewportId, mode: SkyboxMode) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::SetSkyboxMode(target, mode))
            .is_ok()
    }

    pub fn set_skybox_mode(&self, mode: SkyboxMode) -> bool {
        self.set_viewport_skybox_mode(ViewportId::PRIMARY, mode)
    }

    pub fn set_viewport_sky_atmosphere(
        &self,
        target: ViewportId,
        settings: SkyAtmosphereSettings,
    ) -> bool {
        self.channels
            .render_tx
            .send(RenderMsg::SetSkyAtmosphere(target, settings))
            .is_ok()
    }

    pub fn set_sky_atmosphere(&self, settings: SkyAtmosphereSettings) -> bool {
        self.set_viewport_sky_atmosphere(ViewportId::PRIMARY, settings)
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
    #[profiling::function]
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
    #[profiling::function]
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
        self.pending_component_removals.clear();
        self.component_phase_depth = 0;
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

impl AsRef<Store<ComputeShader>> for World {
    fn as_ref(&self) -> &Store<ComputeShader> {
        &self.assets.compute_shaders
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

impl AsRef<Store<Cubemap>> for World {
    fn as_ref(&self) -> &Store<Cubemap> {
        &self.assets.cubemaps
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

impl AsRef<Store<RenderCubemap>> for World {
    fn as_ref(&self) -> &Store<RenderCubemap> {
        &self.assets.render_cubemaps
    }
}

impl AsRef<Store<Material>> for World {
    fn as_ref(&self) -> &Store<Material> {
        &self.assets.materials
    }
}

impl AsRef<Store<MaterialInstance>> for World {
    fn as_ref(&self) -> &Store<MaterialInstance> {
        &self.assets.material_instances
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
        info!("{}-> {}", "  ".repeat(i as usize), &child.name);
        info!(
            "{}- Components: {}",
            "  ".repeat(i as usize + 1),
            child.components.len()
        );

        for comp in &child.components {
            let info = type_info(comp.ctx.tid.type_id())
                .map(|i| i.name)
                .unwrap_or("Unreflected Component");
            info!("{}- {}", "  ".repeat(i as usize + 3), info);
        }

        print_objects_rec(&child.children, i + 1);
    }
}
