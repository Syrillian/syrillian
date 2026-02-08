use crossbeam_channel::{Receiver, Sender, bounded, select, unbounded};
use std::sync::Arc;
use syrillian_asset::AssetStore;
use syrillian_render::rendering::message::RenderMsg;
use syrillian_render::rendering::picking::PickResult;
use syrillian_render::rendering::renderer::{RenderedFrame, Renderer};
use syrillian_render::rendering::state::State;
use syrillian_render::rendering::viewport::ViewportId;
use tracing::warn;
use wgpu::SurfaceConfiguration;

pub enum RenderControlMsg {
    AddViewport(ViewportId, SurfaceConfiguration),
    ResizeViewport(ViewportId, SurfaceConfiguration),
    RemoveViewport(ViewportId),
}

pub struct RenderBatch {
    pub frames: Vec<RenderedFrame>,
    pub present_done_tx: Sender<()>,
}

struct RenderThreadInner {
    renderer: Renderer,
    render_rx: Receiver<RenderMsg>,
    control_rx: Receiver<RenderControlMsg>,
    frame_tx: Sender<RenderBatch>,
}

impl RenderThreadInner {
    fn new(
        state: Arc<State>,
        store: Arc<AssetStore>,
        render_rx: Receiver<RenderMsg>,
        control_rx: Receiver<RenderControlMsg>,
        frame_tx: Sender<RenderBatch>,
        pick_result_tx: Sender<PickResult>,
        primary_config: SurfaceConfiguration,
    ) -> Result<Self, syrillian_render::error::RenderError> {
        let renderer = Renderer::new(state, store, pick_result_tx, primary_config)?;
        Ok(Self {
            renderer,
            render_rx,
            control_rx,
            frame_tx,
        })
    }

    fn run(mut self) {
        profiling::register_thread!("render");

        loop {
            let render_rx = self.render_rx.clone();
            let control_rx = self.control_rx.clone();
            select! {
                recv(render_rx) -> msg => {
                    let Ok(msg) = msg else { break; };
                    if !self.handle_render_msg(msg) {
                        break;
                    }
                }
                recv(control_rx) -> msg => {
                    let Ok(msg) = msg else { break; };
                    self.handle_control(msg);
                }
            }
        }
    }

    fn handle_control(&mut self, msg: RenderControlMsg) {
        match msg {
            RenderControlMsg::AddViewport(target, config) => {
                if let Err(e) = self.renderer.add_viewport(target, config) {
                    warn!("Failed to add render viewport: {e}");
                }
            }
            RenderControlMsg::ResizeViewport(target, config) => {
                if !self.renderer.resize(target, config) {
                    warn!("Failed to resize render viewport {:?}", target);
                }
            }
            RenderControlMsg::RemoveViewport(target) => {
                self.renderer.remove_viewport(target);
            }
        }
    }

    #[profiling::function]
    fn handle_render_msg(&mut self, msg: RenderMsg) -> bool {
        match msg {
            RenderMsg::FrameEnd(_, world_done_tx) => {
                while let Ok(msg) = self.control_rx.try_recv() {
                    self.handle_control(msg);
                }

                self.renderer.update();
                let frames = self.renderer.render_all();
                let (present_done_tx, present_done_rx) = bounded(0);
                if self
                    .frame_tx
                    .send(RenderBatch {
                        frames,
                        present_done_tx,
                    })
                    .is_err()
                {
                    let _ = world_done_tx.send(());
                    return false;
                }

                #[cfg(not(target_arch = "wasm32"))]
                let _ = present_done_rx.recv();
                #[cfg(target_arch = "wasm32")]
                drop(present_done_rx);
                let _ = world_done_tx.send(());

                profiling::finish_frame!();
            }
            msg => {
                self.renderer.handle_message(msg);
            }
        }

        true
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct RenderThread {
    _thread: std::thread::JoinHandle<()>,
    control_tx: Sender<RenderControlMsg>,
    frame_rx: Receiver<RenderBatch>,
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderThread {
    pub fn new(
        state: Arc<State>,
        store: Arc<AssetStore>,
        render_rx: Receiver<RenderMsg>,
        pick_result_tx: Sender<PickResult>,
        primary_config: SurfaceConfiguration,
    ) -> Result<Self, syrillian_render::error::RenderError> {
        let (control_tx, control_rx) = unbounded();
        let (frame_tx, frame_rx) = bounded(1);
        let inner = RenderThreadInner::new(
            state,
            store,
            render_rx,
            control_rx,
            frame_tx,
            pick_result_tx,
            primary_config,
        )?;

        let thread = std::thread::spawn(move || {
            inner.run();
        });

        Ok(RenderThread {
            _thread: thread,
            control_tx,
            frame_rx,
        })
    }

    pub fn add_viewport(
        &self,
        target: ViewportId,
        config: SurfaceConfiguration,
    ) -> Result<(), crossbeam_channel::SendError<RenderControlMsg>> {
        self.control_tx
            .send(RenderControlMsg::AddViewport(target, config))
    }

    pub fn resize_viewport(
        &self,
        target: ViewportId,
        config: SurfaceConfiguration,
    ) -> Result<(), crossbeam_channel::SendError<RenderControlMsg>> {
        self.control_tx
            .send(RenderControlMsg::ResizeViewport(target, config))
    }

    pub fn remove_viewport(
        &self,
        target: ViewportId,
    ) -> Result<(), crossbeam_channel::SendError<RenderControlMsg>> {
        self.control_tx
            .send(RenderControlMsg::RemoveViewport(target))
    }

    pub fn poll_batch(&mut self) -> Option<RenderBatch> {
        self.frame_rx.try_recv().ok()
    }
}

#[cfg(target_arch = "wasm32")]
pub struct RenderThread {
    inner: RenderThreadInner,
    control_tx: Sender<RenderControlMsg>,
    frame_rx: Receiver<RenderBatch>,
}

#[cfg(target_arch = "wasm32")]
impl RenderThread {
    pub fn new(
        state: Arc<State>,
        store: Arc<AssetStore>,
        render_rx: Receiver<RenderMsg>,
        pick_result_tx: Sender<PickResult>,
        primary_config: SurfaceConfiguration,
    ) -> Result<Self, syrillian_render::error::RenderError> {
        let (control_tx, control_rx) = unbounded();
        let (frame_tx, frame_rx) = unbounded();
        let inner = RenderThreadInner::new(
            state,
            store,
            render_rx,
            control_rx,
            frame_tx,
            pick_result_tx,
            primary_config,
        )?;

        Ok(RenderThread {
            inner,
            control_tx,
            frame_rx,
        })
    }

    pub fn add_viewport(
        &self,
        target: ViewportId,
        config: SurfaceConfiguration,
    ) -> Result<(), crossbeam_channel::SendError<RenderControlMsg>> {
        self.control_tx
            .send(RenderControlMsg::AddViewport(target, config))
    }

    pub fn resize_viewport(
        &self,
        target: ViewportId,
        config: SurfaceConfiguration,
    ) -> Result<(), crossbeam_channel::SendError<RenderControlMsg>> {
        self.control_tx
            .send(RenderControlMsg::ResizeViewport(target, config))
    }

    pub fn remove_viewport(
        &self,
        target: ViewportId,
    ) -> Result<(), crossbeam_channel::SendError<RenderControlMsg>> {
        self.control_tx
            .send(RenderControlMsg::RemoveViewport(target))
    }

    pub fn poll_batch(&mut self) -> Option<RenderBatch> {
        use crossbeam_channel::TryRecvError;

        loop {
            match self.inner.control_rx.try_recv() {
                Ok(msg) => {
                    self.inner.handle_control(msg);
                    continue;
                }
                Err(TryRecvError::Disconnected) => return None,
                Err(TryRecvError::Empty) => {}
            }

            match self.inner.render_rx.try_recv() {
                Ok(msg) => {
                    let is_frame_end = matches!(msg, RenderMsg::FrameEnd(_, _));
                    if !self.inner.handle_render_msg(msg) {
                        return None;
                    }
                    if is_frame_end {
                        break;
                    }
                    continue;
                }
                Err(TryRecvError::Disconnected) => return None,
                Err(TryRecvError::Empty) => break,
            }
        }

        self.frame_rx.try_recv().ok()
    }
}
