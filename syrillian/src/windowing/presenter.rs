use crate::ViewportId;
use crate::rendering::RenderedFrame;
use crate::rendering::State;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, warn};
use wgpu::{
    CommandEncoderDescriptor, Extent3d, Origin3d, Surface, SurfaceConfiguration, SurfaceError,
    TexelCopyTextureInfo, TextureAspect,
};
use winit::dpi::PhysicalSize;
use winit::window::{Window, WindowId};

struct PresentViewport {
    window: Window,
    surface: Surface<'static>,
    config: SurfaceConfiguration,
}

pub struct Presenter {
    state: Arc<State>,
    viewports: HashMap<ViewportId, PresentViewport>,
    window_map: HashMap<WindowId, ViewportId>,
}

impl Presenter {
    pub fn new(
        state: Arc<State>,
        main_window: Window,
        surface: Surface<'static>,
        config: SurfaceConfiguration,
    ) -> Self {
        surface.configure(&state.device, &config);

        let mut window_map = HashMap::new();
        window_map.insert(main_window.id(), ViewportId::PRIMARY);

        let mut viewports = HashMap::new();
        viewports.insert(
            ViewportId::PRIMARY,
            PresentViewport {
                window: main_window,
                surface,
                config,
            },
        );

        Presenter {
            state,
            viewports,
            window_map,
        }
    }

    pub fn find_render_target_id(&self, window_id: &WindowId) -> Option<ViewportId> {
        self.window_map.get(window_id).copied()
    }

    pub fn window(&self, viewport: ViewportId) -> Option<&Window> {
        self.viewports.get(&viewport).map(|vp| &vp.window)
    }

    pub fn window_mut(&mut self, viewport: ViewportId) -> Option<&mut Window> {
        self.viewports.get_mut(&viewport).map(|vp| &mut vp.window)
    }

    pub fn add_window(
        &mut self,
        target_id: ViewportId,
        window: Window,
    ) -> Option<SurfaceConfiguration> {
        if self.viewports.contains_key(&target_id) {
            warn!(
                "Viewport #{:?} already exists; ignoring duplicate add",
                target_id
            );
            return None;
        }

        let surface = match self.state.create_surface(&window) {
            Ok(surface) => surface,
            Err(e) => {
                error!("Failed to create surface: {e}");
                return None;
            }
        };

        let config = match self.state.surface_config(&surface, window.inner_size()) {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to create surface config: {e}");
                return None;
            }
        };

        surface.configure(&self.state.device, &config);
        self.window_map.insert(window.id(), target_id);
        self.viewports.insert(
            target_id,
            PresentViewport {
                window,
                surface,
                config: config.clone(),
            },
        );

        Some(config)
    }

    pub fn resize(
        &mut self,
        target_id: ViewportId,
        new_size: PhysicalSize<u32>,
    ) -> Option<SurfaceConfiguration> {
        let viewport = self.viewports.get_mut(&target_id)?;
        let config = match self.state.surface_config(&viewport.surface, new_size) {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to update surface config: {e}");
                return None;
            }
        };
        viewport.config = config.clone();
        viewport
            .surface
            .configure(&self.state.device, &viewport.config);
        Some(config)
    }

    pub fn blit(&mut self, target_id: ViewportId, frame: &RenderedFrame) -> bool {
        let Some(viewport) = self.viewports.get_mut(&target_id) else {
            warn!("Invalid Viewport {target_id:?} referenced");
            return false;
        };

        if frame.format != viewport.config.format {
            warn!(
                "Render format mismatch for {:?}: frame={:?}, surface={:?}",
                target_id, frame.format, viewport.config.format
            );
            return false;
        }

        let mut output = match viewport.surface.get_current_texture() {
            Ok(output) => output,
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                viewport
                    .surface
                    .configure(&self.state.device, &viewport.config);
                return true;
            }
            Err(SurfaceError::OutOfMemory) => {
                error!("The application ran out of GPU memory!");
                return false;
            }
            Err(SurfaceError::Timeout) => return true,
            Err(e @ SurfaceError::Other) => {
                error!("Surface acquisition failed: {e}");
                return false;
            }
        };

        if output.suboptimal {
            warn!("Surface output is suboptimal; reconfiguring surface");
            viewport
                .surface
                .configure(&self.state.device, &viewport.config);
            output = match viewport.surface.get_current_texture() {
                Ok(output) => output,
                Err(_) => return false,
            };
        }

        let copy_width = viewport.config.width.min(frame.size.width);
        let copy_height = viewport.config.height.min(frame.size.height);
        if copy_width == 0 || copy_height == 0 {
            return true;
        }

        let mut encoder = self
            .state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Present Blit Encoder"),
            });

        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &frame.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            Extent3d {
                width: copy_width,
                height: copy_height,
                depth_or_array_layers: 1,
            },
        );

        self.state.queue.submit(Some(encoder.finish()));
        viewport.window.pre_present_notify();
        output.present();

        true
    }
}
