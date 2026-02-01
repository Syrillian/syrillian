use crate::assets::AssetStore;
use crate::rendering::{RenderMsg, UiContext};
use crate::world::{World, WorldChannels};
use crate::{AppState, ViewportId};
use crossbeam_channel::{Receiver, SendError, Sender, TryRecvError, bounded, unbounded};
use std::sync::Arc;
use tracing::{debug, error, info, instrument};
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};

use crate::components::{Component, TypedComponentId};
use crate::core::ObjectHash;
#[cfg(not(target_arch = "wasm32"))]
use std::marker::PhantomData;
#[cfg(not(target_arch = "wasm32"))]
use std::thread::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderEventTarget {
    pub id: ViewportId,
}

#[derive(Debug, Clone)]
pub enum RenderAppEvent {
    Init(RenderEventTarget),
    Input(RenderEventTarget, WindowEvent),
    DeviceEvent(DeviceId, DeviceEvent),
    StartFrame(RenderEventTarget),
    Resize(RenderEventTarget, PhysicalSize<u32>),
}

#[derive(Debug, Clone)]
pub enum GameAppEvent {
    UpdateWindowTitle(ViewportId, String),
    SetCursorMode(ViewportId, bool, bool),
    AddWindow(ViewportId, PhysicalSize<u32>),
    Shutdown,
}

impl GameAppEvent {
    pub fn cursor_mode(target: ViewportId, locked: bool, visible: bool) -> GameAppEvent {
        Self::SetCursorMode(target, locked, visible)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GameThread<S: AppState> {
    _thread: JoinHandle<()>,
    render_event_tx: Sender<RenderAppEvent>,
    pub game_event_rx: Receiver<GameAppEvent>,
    _state: PhantomData<S>,
}

#[cfg(target_arch = "wasm32")]
pub struct GameThread<S: AppState> {
    thread: GameThreadInner<S>,
    render_event_tx: Sender<RenderAppEvent>,
}

struct GameThreadInner<S: AppState> {
    world: Box<World>,
    state: S,
    render_event_rx: Receiver<RenderAppEvent>,
    active_target: ViewportId,
    initialized: bool,
}

impl<S: AppState> GameThreadInner<S> {
    #[cfg(not(target_arch = "wasm32"))]
    fn spawn(
        asset_store: Arc<AssetStore>,
        channels: WorldChannels,
        render_event_rx: Receiver<RenderAppEvent>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            profiling::register_thread!("game");

            let state = S::default();
            Self::spawn_local(state, asset_store, channels, render_event_rx).run();

            debug!("Game thread exited");
        })
    }

    fn spawn_local(
        state: S,
        asset_store: Arc<AssetStore>,
        channels: WorldChannels,
        render_event_rx: Receiver<RenderAppEvent>,
    ) -> GameThreadInner<S> {
        let world = World::new_with_channels(asset_store, channels);

        GameThreadInner {
            world,
            state,
            render_event_rx,
            active_target: ViewportId::PRIMARY,
            initialized: false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<S: AppState> GameThread<S> {
    pub fn new(
        asset_store: Arc<AssetStore>,
        channels: WorldChannels,
        game_event_rx: Receiver<GameAppEvent>,
    ) -> Self {
        let (render_event_tx, render_event_rx) = unbounded();

        let thread = GameThreadInner::<S>::spawn(asset_store, channels, render_event_rx);

        GameThread {
            _thread: thread,
            render_event_tx,
            game_event_rx,
            _state: PhantomData,
        }
    }

    pub fn init(&self) -> bool {
        self.render_event_tx
            .send(RenderAppEvent::Init(RenderEventTarget {
                id: ViewportId::PRIMARY,
            }))
            .is_ok()
    }

    pub fn input(
        &self,
        target: ViewportId,
        event: WindowEvent,
    ) -> Result<(), Box<SendError<RenderAppEvent>>> {
        self.render_event_tx
            .send(RenderAppEvent::Input(
                RenderEventTarget { id: target },
                event,
            ))
            .map_err(Box::new)
    }

    pub fn device_event(
        &self,
        device_id: DeviceId,
        event: DeviceEvent,
    ) -> Result<(), Box<SendError<RenderAppEvent>>> {
        self.render_event_tx
            .send(RenderAppEvent::DeviceEvent(device_id, event))
            .map_err(Box::new)
    }

    pub fn resize(
        &self,
        target: ViewportId,
        size: PhysicalSize<u32>,
    ) -> Result<(), Box<SendError<RenderAppEvent>>> {
        self.render_event_tx
            .send(RenderAppEvent::Resize(
                RenderEventTarget { id: target },
                size,
            ))
            .map_err(Box::new)
    }

    // TODO: Think about if render frame and world should be linked
    #[instrument(skip_all)]
    pub fn next_frame(&self, target: ViewportId) -> Result<(), Box<SendError<RenderAppEvent>>> {
        self.render_event_tx
            .send(RenderAppEvent::StartFrame(RenderEventTarget { id: target }))
            .map_err(Box::new)
    }
}

#[cfg(target_arch = "wasm32")]
impl<S: AppState> GameThread<S> {
    pub fn new(state: S, asset_store: Arc<AssetStore>, channels: WorldChannels) -> Self {
        let (render_event_tx, render_event_rx) = unbounded();

        let thread = GameThreadInner::spawn_local(state, asset_store, channels, render_event_rx);

        GameThread {
            thread,
            render_event_tx,
        }
    }
    pub fn init(&mut self) -> bool {
        self.thread.init()
    }

    pub fn input(
        &self,
        target: RenderTarget,
        event: WindowEvent,
    ) -> Result<(), SendError<RenderAppEvent>> {
        self.render_event_tx.send(RenderAppEvent::Input(
            RenderEventTarget { id: target },
            event,
        ))
    }

    pub fn device_event(
        &self,
        target: RenderTarget,
        device_id: DeviceId,
        event: DeviceEvent,
    ) -> Result<(), SendError<RenderAppEvent>> {
        self.render_event_tx
            .send(RenderAppEvent::DeviceEvent(device_id, event))
    }

    pub fn resize(
        &self,
        target: RenderTarget,
        size: PhysicalSize<u32>,
    ) -> Result<(), SendError<RenderAppEvent>> {
        self.render_event_tx.send(RenderAppEvent::Resize(
            RenderEventTarget { id: target },
            size,
        ))
    }

    // TODO: Think about if render frame and world should be linked
    pub fn next_frame(&self, target: RenderTarget) -> Result<(), SendError<RenderAppEvent>> {
        self.render_event_tx
            .send(RenderAppEvent::StartFrame(RenderEventTarget { id: target }))
    }
}
impl<S: AppState> GameThreadInner<S> {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn run(mut self) {
        loop {
            if !self.pump_events() {
                break;
            }
            if self.initialized {
                let target = self.active_target;
                if !self.update(target) {
                    break;
                }
            }
        }
    }

    #[profiling::function]
    pub fn pump_events(&mut self) -> bool {
        let mut keep_running = true;
        loop {
            let event = match self.render_event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    info!("Window Event Loop exited. Exiting event loop.");
                    return false;
                }
            };

            keep_running = match event {
                RenderAppEvent::Init(target) => {
                    if target.id == ViewportId::PRIMARY {
                        self.init()
                    } else {
                        true
                    }
                }
                RenderAppEvent::Input(target, event) => self.input(target.id, event),
                RenderAppEvent::Resize(target, size) => self.resize(target.id, size),
                RenderAppEvent::StartFrame(target) => {
                    self.active_target = target.id;
                    true
                }
                RenderAppEvent::DeviceEvent(id, event) => self.device_event(id, &event),
            };

            if !keep_running {
                break;
            }
        }

        if !keep_running {
            info!("Game signaled exit. Exiting event loop.");
        }

        keep_running
    }

    #[profiling::function]
    pub fn init(&mut self) -> bool {
        if let Err(e) = self.state.init(&mut self.world) {
            error!("World init function hook returned: {e}");
            return false;
        }

        self.initialized = true;
        true
    }

    #[profiling::function]
    pub fn input(&mut self, target: ViewportId, event: WindowEvent) -> bool {
        self.active_target = target;
        self.world.input.set_active_target(target);
        self.world.input.process_event(target, &event);

        true
    }

    #[profiling::function]
    pub fn device_event(&mut self, _device: DeviceId, event: &DeviceEvent) -> bool {
        self.world.input.process_device_input_event(event);

        true
    }

    #[profiling::function]
    pub fn resize(&mut self, target: ViewportId, size: PhysicalSize<u32>) -> bool {
        self.world.set_viewport_size(target, size);

        true
    }

    // TODO: Think about if renderer delta time should be linked to world tick time
    #[profiling::function]
    pub fn update(&mut self, target: ViewportId) -> bool {
        let world = self.world.as_mut();
        if world.is_shutting_down() {
            world.teardown();
            return self.signal_frame_end(target);
        }

        profiling::scope!("update");
        if let Err(e) = self.state.update(world) {
            error!("Error happened when calling update function hook: {e}");
        }

        world.fixed_update();
        world.update();

        if let Err(e) = self.state.late_update(world) {
            error!("Error happened when calling late update function hook: {e}");
        }

        if let Err(e) = self.state.on_gui(
            world,
            &UiContext::new(ObjectHash::MAX, TypedComponentId::null::<dyn Component>()),
        ) {
            error!("Error happened when calling late update function hook: {e}");
        }

        world.post_update();

        if let Err(e) = self.state.post_update(world) {
            error!("Error happened when calling post update function hook: {e}");
        }

        world.next_frame();

        self.signal_frame_end(target)
    }

    #[profiling::function]
    fn signal_frame_end(&mut self, target: ViewportId) -> bool {
        let (frame_done_tx, frame_done_rx) = bounded(0);
        if self
            .world
            .channels
            .render_tx
            .send(RenderMsg::FrameEnd(target, frame_done_tx))
            .is_err()
        {
            return false;
        }

        frame_done_rx.recv().is_ok()
    }
}
