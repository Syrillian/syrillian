use crate::AppState;
use crate::game_thread::GameAppEvent;
use crate::windowing::game_thread::GameThread;
use crate::windowing::presenter::Presenter;
use crate::windowing::render_thread::RenderThread;
use crate::world::WorldChannels;
use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::error::Error;
use std::marker::PhantomData;
use std::sync::Arc;
use syrillian_asset::AssetStore;
use syrillian_render::rendering::renderer::RenderedFrame;
use syrillian_render::rendering::state::State;
use syrillian_render::rendering::viewport::ViewportId;
use syrillian_utils::EngineArgs;
use tracing::{error, info, instrument, trace, warn};
use winit::application::ApplicationHandler;
use winit::dpi::Size;
use winit::error::EventLoopError;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowExtWebSys};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowAttributes, WindowId};

pub struct App<S: AppState> {
    main_window_attributes: WindowAttributes,
    presenter: Option<Presenter>,
    render_thread: Option<RenderThread>,
    game_thread: Option<GameThread<S>>,
    pending_frames: HashMap<ViewportId, RenderedFrame>,
}

pub struct AppSettings<S: AppState> {
    pub main_window: WindowAttributes,
    pub(crate) _state_type: PhantomData<S>,
}

impl<S: AppState> AppSettings<S> {
    pub fn run(self) -> Result<(), Box<dyn Error>> {
        let (event_loop, app) = self.init_state()?;
        app.run(event_loop)
    }

    fn init_state(self) -> Result<(EventLoop<()>, App<S>), Box<dyn Error>> {
        let event_loop = match EventLoop::new() {
            Err(EventLoopError::NotSupported(_)) => {
                return Err("No graphics backend found that could be used.".into());
            }
            e => e?,
        };
        event_loop.set_control_flow(ControlFlow::Poll);

        let app = App {
            main_window_attributes: self.main_window,
            presenter: None,
            render_thread: None,
            game_thread: None,
            pending_frames: HashMap::new(),
        };

        Ok((event_loop, app))
    }
}

impl<S: AppState> App<S> {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn run(mut self, event_loop: EventLoop<()>) -> Result<(), Box<dyn Error>> {
        // let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
        // let _puffin_server = puffin_http::Server::new(&server_addr)?;
        // warn!("Serving profile data on {server_addr}. Run `puffin_viewer` to view it.");
        // profiling::puffin::set_scopes_on(true);

        profiling::register_thread!("window");

        event_loop.run_app(&mut self)?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn run(self, event_loop: EventLoop<()>) -> Result<(), Box<dyn Error>> {
        event_loop.spawn_app(self);
        Ok(())
    }

    #[instrument(skip_all)]
    fn init(&mut self, event_loop: &ActiveEventLoop) {
        info!("Initializing render state");

        let asset_store = AssetStore::new();

        let (render_state_tx, render_state_rx) = unbounded();
        let (game_event_tx, game_event_rx) = unbounded();
        let (pick_result_tx, pick_result_rx) = unbounded();

        let main_window = event_loop
            .create_window(self.main_window_attributes.clone())
            .unwrap();

        if EngineArgs::get().fullscreen {
            Self::start_in_fullscreen(&main_window);
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(canvas) = main_window.canvas() {
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_elements_by_tag_name("body")
                .get_with_index(0)
                .unwrap()
                .append_child(&canvas)
                .unwrap();
        }

        trace!("Created render surface");

        let (state, surface, config) = match State::new(&main_window) {
            Ok(state) => state,
            Err(err) => {
                error!("Couldn't create render state: {err}");
                event_loop.exit();
                return;
            }
        };
        let state = Arc::new(state);

        let presenter = Presenter::new(state.clone(), main_window, surface, config.clone());

        let render_thread = match RenderThread::new(
            state.clone(),
            asset_store.clone(),
            render_state_rx,
            pick_result_tx,
            config.clone(),
        ) {
            Ok(thread) => thread,
            Err(err) => {
                error!("Couldn't create render thread: {err}");
                event_loop.exit();
                return;
            }
        };

        trace!("Created Render Thread");

        let channels = WorldChannels::new(render_state_tx, game_event_tx, pick_result_rx);
        let game_thread = GameThread::new(asset_store.clone(), channels, game_event_rx);

        if !game_thread.init() {
            error!("Couldn't initialize Game Thread");
            event_loop.exit();
            return;
        }

        if let Some(window) = presenter.window(ViewportId::PRIMARY) {
            window.request_redraw();
        }

        self.presenter = Some(presenter);
        self.render_thread = Some(render_thread);
        self.game_thread = Some(game_thread);
    }

    #[instrument(skip_all)]
    fn handle_events(
        presenter: &mut Presenter,
        render_thread: &RenderThread,
        game_thread: &GameThread<S>,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        for event in game_thread.game_event_rx.try_iter() {
            match event {
                GameAppEvent::UpdateWindowTitle(event_target, title) => {
                    if let Some(window) = presenter.window_mut(event_target) {
                        window.set_title(&title);
                    }
                }
                GameAppEvent::SetCursorMode(event_target, locked, visible) => {
                    if let Some(window) = presenter.window_mut(event_target) {
                        if locked {
                            trace!("RT: Locked cursor");
                            window
                                .set_cursor_grab(CursorGrabMode::Locked)
                                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                                .expect("Couldn't grab cursor");
                        } else {
                            trace!("RT: Unlocked cursor");
                            window
                                .set_cursor_grab(CursorGrabMode::None)
                                .expect("Couldn't ungrab cursor");
                        }
                        window.set_cursor_visible(visible);
                        if visible {
                            trace!("RT: Shown cursor");
                        } else {
                            trace!("RT: Hid cursor");
                        }
                    }
                }
                GameAppEvent::AddWindow(event_target, size) => {
                    let window = match event_loop.create_window(
                        WindowAttributes::default()
                            .with_inner_size(Size::Physical(size))
                            .with_title(format!("Syrillian Window {}", event_target.get())),
                    ) {
                        Ok(w) => w,
                        Err(e) => {
                            error!("Failed to create window: {e}");
                            return false;
                        }
                    };

                    let Some(config) = presenter.add_window(event_target, window) else {
                        error!("Failed to create window surface");
                        return false;
                    };
                    if let Err(e) = render_thread.add_viewport(event_target, config) {
                        error!("Failed to create renderer viewport: {e}");
                        return false;
                    }

                    if let Some(window) = presenter.window(event_target) {
                        window.request_redraw();
                    }
                }
                GameAppEvent::Shutdown => return false,
            }
        }
        true
    }

    #[instrument(skip_all)]
    fn handle_all_game_events(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(presenter) = self.presenter.as_mut() else {
            return true;
        };

        let Some(game_thread) = self.game_thread.as_ref() else {
            return true;
        };

        let Some(render_thread) = self.render_thread.as_ref() else {
            return true;
        };

        Self::handle_events(presenter, render_thread, game_thread, event_loop)
    }

    fn start_in_fullscreen(main_window: &Window) {
        if cfg!(target_os = "macos") {
            main_window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            let Some(monitor) = main_window
                .current_monitor()
                .or_else(|| main_window.primary_monitor())
            else {
                warn!("No monitor found to be fullscreen on");
                return;
            };

            let available_video_modes: Vec<_> = monitor.video_modes().collect();
            trace!("All available video modes: {available_video_modes:?}");

            let Some(next) = available_video_modes.into_iter().next() else {
                let name = monitor
                    .name()
                    .unwrap_or_else(|| "Generic Monitor".to_string());
                warn!("No video mode handle found for switching to fullscreen on monitor {name:?}");
                return;
            };

            main_window.set_fullscreen(Some(Fullscreen::Exclusive(next)));
        }
    }
}

impl<S: AppState> ApplicationHandler for App<S> {
    #[instrument(skip_all)]
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        match cause {
            StartCause::Poll => {
                if !self.handle_all_game_events(event_loop) {
                    event_loop.exit();
                }
            }
            StartCause::Init => self.init(event_loop),
            StartCause::ResumeTimeReached { .. } => (),
            StartCause::WaitCancelled { .. } => (),
        }
    }

    #[instrument(skip_all)]
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // TODO: Reinit cache?
    }

    #[profiling::function]
    #[instrument(skip_all)]
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if event_loop.exiting() {
            return;
        }

        if !self.handle_all_game_events(event_loop) {
            event_loop.exit();
            return;
        }

        let Some(game_thread) = self.game_thread.as_ref() else {
            return;
        };
        let Some(presenter) = self.presenter.as_mut() else {
            return;
        };
        let Some(render_thread) = self.render_thread.as_mut() else {
            return;
        };

        let target_id = presenter
            .find_render_target_id(&window_id)
            .expect("runtime missing for window");
        let drives_update = target_id.is_primary();

        match event {
            WindowEvent::RedrawRequested => {
                profiling::scope!("redraw requested");
                if drives_update {
                    if let Some(batch) = render_thread.poll_batch() {
                        for frame in batch.frames {
                            self.pending_frames.insert(frame.target, frame);
                        }

                        if let Some(frame) = self.pending_frames.remove(&target_id)
                            && !presenter.blit(target_id, &frame)
                        {
                            event_loop.exit(); // TODO: Maybe just remove the window
                            let _ = batch.present_done_tx.send(());
                            return;
                        }

                        let _ = batch.present_done_tx.send(());
                    }
                } else if let Some(frame) = self.pending_frames.remove(&target_id) {
                    let _ = presenter.blit(target_id, &frame);
                } else {
                    return;
                }

                if let Some(window) = presenter.window(target_id) {
                    window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(config) = presenter.resize(target_id, size) {
                    if render_thread.resize_viewport(target_id, config).is_err() {
                        event_loop.exit();
                    }
                } else {
                    event_loop.exit();
                }
                self.pending_frames.remove(&target_id);
                if game_thread.resize(target_id, size).is_err() {
                    event_loop.exit();
                }
            }
            _ => {
                if game_thread.input(target_id, event).is_err() {
                    event_loop.exit();
                }
            }
        }

        // debug_assert!(event_start.elapsed().as_secs_f32() < 2.0);
    }

    #[instrument(skip_all)]
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if !self.handle_all_game_events(event_loop) {
            event_loop.exit();
            return;
        }

        let Some(game_thread) = self.game_thread.as_ref() else {
            return;
        };

        if game_thread.device_event(device_id, event.clone()).is_err() {
            event_loop.exit();
        }
    }
}
