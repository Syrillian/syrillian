use crate::cache::AssetCache;
use crate::lighting::proxy::{LightProxy, LightType};
use crate::passes::pipeline::RenderPipeline;
use crate::rendering::FrameCtx;
use crate::rendering::picking::PickingSurface;
use crate::rendering::render_data::{RenderUniformData, SkyAtmosphereSettings, SkyboxMode};
use glamx::UVec2;
use std::time::{Duration, Instant};
use syrillian_asset::{HCubemap, HRenderTexture2D};
use tracing::instrument;
use wgpu::{Device, Queue, SurfaceConfiguration, TextureViewDescriptor};
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ViewportId(pub u64);

impl ViewportId {
    pub const PRIMARY: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn is_primary(self) -> bool {
        self.get() == Self::PRIMARY.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RenderTarget {
    Viewport(ViewportId),
    Texture(HRenderTexture2D),
}

impl RenderTarget {
    pub const PRIMARY_WINDOW: Self = Self::Viewport(ViewportId::PRIMARY);
}

pub struct RenderViewport {
    pub id: ViewportId,
    pub config: SurfaceConfiguration,
    pub render_pipeline: RenderPipeline,
    pub picking_surface: PickingSurface,
    pub render_data: RenderUniformData,
    pub start_time: Instant,
    pub delta_time: Duration,
    pub last_frame_time: Instant,
    pub frame_count: usize,
    desired_skybox: Option<HCubemap>,
    resolved_skybox: Option<HCubemap>,
    sky_mode: SkyboxMode,
}

impl RenderViewport {
    pub fn new(
        id: ViewportId,
        mut config: SurfaceConfiguration,
        device: &Device,
        cache: &AssetCache,
    ) -> Self {
        Self::clamp_config(&mut config);

        let render_bgl = cache.bgl_render();

        let picking_surface = PickingSurface::new(device, &config);
        let fallback_skybox = cache.cubemap_fallback();
        let render_data = RenderUniformData::empty(
            device,
            &render_bgl,
            fallback_skybox.view().clone(),
            fallback_skybox.sampler().clone(),
        );
        let post_pipeline = RenderPipeline::new(device, cache, &config);

        RenderViewport {
            id,
            config,
            render_pipeline: post_pipeline,
            picking_surface,
            render_data,
            start_time: Instant::now(),
            delta_time: Duration::default(),
            last_frame_time: Instant::now(),
            frame_count: 0,
            desired_skybox: None,
            resolved_skybox: None,
            sky_mode: SkyboxMode::Cubemap,
        }
    }

    fn clamp_config(config: &mut SurfaceConfiguration) {
        config.width = config.width.max(1);
        config.height = config.height.max(1);
    }

    #[instrument(skip_all)]
    #[profiling::function]
    pub fn resize(
        &mut self,
        mut config: SurfaceConfiguration,
        device: &Device,
        cache: &AssetCache,
    ) {
        Self::clamp_config(&mut config);
        self.config = config;

        self.render_pipeline.recreate(device, cache, &self.config);
        self.picking_surface.recreate(device, &self.config);
    }

    #[instrument(skip_all)]
    #[profiling::function]
    pub fn begin_render(&mut self) -> FrameCtx {
        self.frame_count += 1;

        let depth_view = self
            .render_pipeline
            .depth_texture
            .create_view(&TextureViewDescriptor::default());

        FrameCtx { depth_view }
    }

    pub fn update_render_data(&mut self, queue: &Queue) {
        self.update_system_data(queue);
        self.render_data.upload_sky_data(queue);
    }

    pub fn update_view_camera_data(&mut self, queue: &Queue) {
        self.render_data.upload_camera_data(queue);
    }

    pub fn update_system_data(&mut self, queue: &Queue) {
        let window_size = UVec2::new(self.config.width.max(1), self.config.height.max(1));

        let system_data = &mut self.render_data.system_data;
        system_data.screen_size = window_size;
        system_data.time = self.start_time.elapsed().as_secs_f32();
        system_data.delta_time = self.delta_time.as_secs_f32();

        self.render_data.upload_system_data(queue);
    }

    /// Updates the delta time based on the elapsed time since the last frame
    pub fn tick_delta_time(&mut self) {
        self.delta_time = self.last_frame_time.elapsed();
        self.last_frame_time = Instant::now();
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.config.width,
            height: self.config.height,
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn set_skybox(&mut self, cubemap: Option<HCubemap>) {
        self.desired_skybox = cubemap;
    }

    pub fn set_sky_mode(&mut self, mode: SkyboxMode) {
        self.sky_mode = mode;
        self.render_data.set_sky_mode(mode);
    }

    pub fn sky_mode(&self) -> SkyboxMode {
        self.sky_mode
    }

    pub fn set_sky_atmosphere(&mut self, settings: SkyAtmosphereSettings) {
        self.render_data.set_sky_settings(settings);
    }

    pub fn sync_sun_light(&mut self, light: Option<&LightProxy>) {
        let Some(light) = light else {
            return;
        };

        if LightType::try_from(light.type_id).ok() != Some(LightType::Sun) {
            return;
        }

        self.render_data.sync_sun_light(light);
    }

    pub fn refresh_skybox_binding(&mut self, device: &Device, cache: &AssetCache) {
        let resolved = self.desired_skybox.and_then(|handle| {
            if cache.cubemap(handle).is_some() {
                Some(handle)
            } else {
                None
            }
        });

        if self.resolved_skybox == resolved {
            return;
        }

        let skybox = resolved
            .and_then(|handle| cache.cubemap(handle))
            .unwrap_or_else(|| cache.cubemap_fallback());
        let render_bgl = cache.bgl_render();
        self.render_data.rebuild_bind_group(
            device,
            &render_bgl,
            skybox.view().clone(),
            skybox.sampler().clone(),
        );
        self.resolved_skybox = resolved;
    }
}
