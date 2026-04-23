use crate::cache::AssetCache;
use crate::passes::post_process::{
    BloomRenderPass, BloomSettings, FinalRenderPass, FxaaRenderPass, PostProcessPass,
    PostProcessPassContext, PostProcessRoute, PostProcessSharedViews,
    ScreenSpaceAmbientOcclusionRenderPass, ScreenSpaceReflectionRenderPass,
};
use crate::passes::ui_pass::UiRenderPass;
use crate::rendering::offscreen_surface::OffscreenSurface;
use crate::rendering::render_data::RenderUniformData;
use crate::rendering::renderer::RenderedFrame;
use crate::rendering::state::State;
use crate::rendering::viewport::{RenderViewport, ViewportId};
use crate::strobe::StrobeRenderer;
use syrillian_utils::{AntiAliasingMode, EngineArgs};
use wgpu::{
    CommandEncoder, Device, Extent3d, Queue, SurfaceConfiguration, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};
use winit::dpi::PhysicalSize;

const COLOR_ID_BASE: u32 = 0;
const COLOR_ID_POST_A: u32 = 1;
const COLOR_ID_POST_B: u32 = 2;
const COLOR_ID_FINAL_A: u32 = 3;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct PostProcessRouting {
    run_ssr: bool,
    run_ssao: bool,
    run_bloom: bool,
    run_fxaa: bool,
}

impl PostProcessRouting {
    fn current() -> Self {
        Self {
            run_ssr: !EngineArgs::get().no_ssr,
            run_ssao: !EngineArgs::get().no_ssao,
            run_bloom: !EngineArgs::get().no_bloom,
            run_fxaa: matches!(EngineArgs::aa_mode(), AntiAliasingMode::Fxaa),
        }
    }
}

struct ActivePostProcessRoutes {
    ssr: PostProcessRoute,
    ssao: PostProcessRoute,
    bloom: PostProcessRoute,
    fxaa: PostProcessRoute,
    final_pass: PostProcessRoute,
}

pub struct FinalFrameContext<'a> {
    pub render_data: &'a RenderUniformData,
    pub target: ViewportId,
    pub size: PhysicalSize<u32>,
    pub format: TextureFormat,
    pub frame_count: usize,
    pub disable_post_processing: bool,
}

pub struct RenderPipeline {
    device: Device,
    pub depth_texture: Texture,
    pub offscreen_surface: OffscreenSurface,
    pub final_surfaces: [OffscreenSurface; 2],
    post_process_surfaces: [OffscreenSurface; 2],
    pub g_normal: Texture,
    pub g_material: Texture,
    pub g_velocity: Texture,
    shared_views: PostProcessSharedViews,

    pub ssr_pass: ScreenSpaceReflectionRenderPass,
    pub ssao_pass: ScreenSpaceAmbientOcclusionRenderPass,
    pub fxaa_pass: FxaaRenderPass,
    pub bloom_pass: BloomRenderPass,
    pub final_pass: FinalRenderPass,

    route_key: PostProcessRouting,
    bloom_settings: BloomSettings,
    bloom_settings_dirty: bool,
}

impl RenderPipeline {
    pub fn new(device: &Device, cache: &AssetCache, config: &SurfaceConfiguration) -> Self {
        let pp_bgl = cache.bgl_post_process();

        let normal_texture = Self::create_g_buffer("GBuffer (Normals)", device, config);
        let material_texture = Self::create_material_texture(device, config);
        let velocity_texture = Self::create_velocity_texture(device, config);
        let depth_texture = Self::create_depth_texture(device, config);
        let shared_views = PostProcessSharedViews {
            depth: depth_texture.create_view(&TextureViewDescriptor::default()),
            g_normal: normal_texture.create_view(&TextureViewDescriptor::default()),
            g_material: material_texture.create_view(&TextureViewDescriptor::default()),
            g_velocity: velocity_texture.create_view(&TextureViewDescriptor::default()),
        };

        let offscreen_surface = OffscreenSurface::new_with(
            device,
            config,
            TextureFormat::Rgba8Unorm,
            TextureUsages::empty(),
        );

        let post_process_surfaces = [
            OffscreenSurface::new_with(
                device,
                config,
                TextureFormat::Rgba8Unorm,
                TextureUsages::STORAGE_BINDING,
            ),
            OffscreenSurface::new_with(
                device,
                config,
                TextureFormat::Rgba8Unorm,
                TextureUsages::STORAGE_BINDING,
            ),
        ];

        let final_surfaces = [
            OffscreenSurface::new(device, config),
            OffscreenSurface::new(device, config),
        ];

        let bloom_settings = BloomSettings::from_engine_args();
        let routing = PostProcessRouting::current();

        let routes = Self::compose_routes(
            routing,
            offscreen_surface.view().clone(),
            post_process_surfaces[0].view().clone(),
            post_process_surfaces[1].view().clone(),
            final_surfaces[0].view().clone(),
        );

        let ssr_pass = ScreenSpaceReflectionRenderPass::new(
            device,
            cache.bgl_post_process_compute(),
            &shared_views,
            &routes.ssr,
        );

        let size = offscreen_surface.texture().size();
        let ssao_pass = ScreenSpaceAmbientOcclusionRenderPass::new(
            device,
            size.width,
            size.height,
            cache.bgl_ssao_compute(),
            cache.bgl_ssao_apply_compute(),
            &shared_views,
            &routes.ssao,
        );

        let bloom_pass = BloomRenderPass::new(
            device,
            size.width,
            size.height,
            cache.bgl_bloom_compute(),
            &routes.bloom,
            &bloom_settings,
        );

        let fxaa_pass = FxaaRenderPass::new(device, &pp_bgl, &shared_views, &routes.fxaa);

        let final_pass = FinalRenderPass::new(device, &pp_bgl, &shared_views, &routes.final_pass);

        Self {
            device: device.clone(),
            depth_texture,
            offscreen_surface,
            final_surfaces,
            post_process_surfaces,
            g_normal: normal_texture,
            g_material: material_texture,
            g_velocity: velocity_texture,
            shared_views,
            ssr_pass,
            ssao_pass,
            fxaa_pass,
            bloom_pass,
            final_pass,
            route_key: routing,
            bloom_settings,
            bloom_settings_dirty: false,
        }
    }

    pub fn recreate(&mut self, device: &Device, cache: &AssetCache, config: &SurfaceConfiguration) {
        let bloom_settings = self.bloom_settings;
        *self = Self::new(device, cache, config);
        self.set_bloom_settings(bloom_settings);
    }

    #[inline]
    fn current_surface_index(frame_count: usize) -> usize {
        frame_count % 2
    }

    fn compose_routes(
        key: PostProcessRouting,
        base_view: TextureView,
        post_a_view: TextureView,
        post_b_view: TextureView,
        final_a_view: TextureView,
    ) -> ActivePostProcessRoutes {
        let default_route = PostProcessRoute {
            input_id: COLOR_ID_BASE,
            output_id: COLOR_ID_POST_A,
            input_color: base_view.clone(),
            output_color: post_a_view.clone(),
        };

        let mut ssr = default_route.clone();
        let mut ssao = default_route.clone();
        let mut bloom = default_route.clone();
        let mut fxaa = default_route.clone();

        let mut current_id = COLOR_ID_BASE;
        let mut current_view = base_view;
        let mut write_to_a = true;

        let mut next_output = || {
            if write_to_a {
                write_to_a = false;
                (COLOR_ID_POST_A, post_a_view.clone())
            } else {
                write_to_a = true;
                (COLOR_ID_POST_B, post_b_view.clone())
            }
        };

        if key.run_ssr {
            let (output_id, output_view) = next_output();
            ssr = PostProcessRoute {
                input_id: current_id,
                output_id,
                input_color: current_view.clone(),
                output_color: output_view.clone(),
            };
            current_id = output_id;
            current_view = output_view;
        }

        if key.run_ssao {
            let (output_id, output_view) = next_output();
            ssao = PostProcessRoute {
                input_id: current_id,
                output_id,
                input_color: current_view.clone(),
                output_color: output_view.clone(),
            };
            current_id = output_id;
            current_view = output_view;
        }

        if key.run_bloom {
            let (output_id, output_view) = next_output();
            bloom = PostProcessRoute {
                input_id: current_id,
                output_id,
                input_color: current_view.clone(),
                output_color: output_view.clone(),
            };
            current_id = output_id;
            current_view = output_view;
        }

        if key.run_fxaa {
            let (output_id, output_view) = next_output();
            fxaa = PostProcessRoute {
                input_id: current_id,
                output_id,
                input_color: current_view.clone(),
                output_color: output_view.clone(),
            };
            current_id = output_id;
            current_view = output_view;
        }

        let final_pass = PostProcessRoute {
            input_id: current_id,
            output_id: COLOR_ID_FINAL_A,
            input_color: current_view,
            output_color: final_a_view,
        };

        ActivePostProcessRoutes {
            ssr,
            ssao,
            bloom,
            fxaa,
            final_pass,
        }
    }

    fn create_depth_texture(device: &Device, config: &SurfaceConfiguration) -> Texture {
        device.create_texture(&TextureDescriptor {
            label: Some("Depth Texture"),
            size: Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_g_buffer(
        which: &'static str,
        device: &Device,
        config: &SurfaceConfiguration,
    ) -> Texture {
        device.create_texture(&TextureDescriptor {
            label: Some(which),
            size: Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rg16Float,
            usage: TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_material_texture(device: &Device, config: &SurfaceConfiguration) -> Texture {
        device.create_texture(&TextureDescriptor {
            label: Some("Material Property Texture"),
            size: Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8Unorm,
            usage: TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_velocity_texture(device: &Device, config: &SurfaceConfiguration) -> Texture {
        device.create_texture(&TextureDescriptor {
            label: Some("GBuffer (Velocity)"),
            size: Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rg16Float,
            usage: TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn rebuild_post_process_passes(&mut self, cache: &AssetCache, key: PostProcessRouting) {
        let routes = Self::compose_routes(
            key,
            self.offscreen_surface.view().clone(),
            self.post_process_surfaces[0].view().clone(),
            self.post_process_surfaces[1].view().clone(),
            self.final_surfaces[0].view().clone(),
        );

        self.ssr_pass = ScreenSpaceReflectionRenderPass::new(
            &self.device,
            cache.bgl_post_process_compute(),
            &self.shared_views,
            &routes.ssr,
        );

        let size = self.offscreen_surface.texture().size();
        self.ssao_pass = ScreenSpaceAmbientOcclusionRenderPass::new(
            &self.device,
            size.width,
            size.height,
            cache.bgl_ssao_compute(),
            cache.bgl_ssao_apply_compute(),
            &self.shared_views,
            &routes.ssao,
        );

        self.bloom_pass = BloomRenderPass::new(
            &self.device,
            size.width,
            size.height,
            cache.bgl_bloom_compute(),
            &routes.bloom,
            &self.bloom_settings,
        );

        self.fxaa_pass = FxaaRenderPass::new(
            &self.device,
            &cache.bgl_post_process(),
            &self.shared_views,
            &routes.fxaa,
        );

        self.final_pass = FinalRenderPass::new(
            &self.device,
            &cache.bgl_post_process(),
            &self.shared_views,
            &routes.final_pass,
        );

        self.route_key = key;
        self.bloom_settings_dirty = false;
    }

    fn ensure_route_configuration(&mut self, cache: &AssetCache) {
        let desired = PostProcessRouting::current();
        if desired != self.route_key {
            self.rebuild_post_process_passes(cache, desired);
        }
    }

    pub fn prepare_frame(
        &mut self,
        render_data: &mut RenderUniformData,
        queue: &Queue,
        _frame_count: usize,
    ) {
        let base_view_proj =
            render_data.camera_data.projection_mat * render_data.camera_data.view_mat;
        let view_proj = base_view_proj;

        render_data.camera_data.proj_view_mat = view_proj;
        render_data.camera_data.inv_proj_view_mat = view_proj.inverse();

        render_data.upload_camera_data(queue);

        if self.bloom_settings_dirty {
            let desired = PostProcessRouting::current();
            if desired == self.route_key {
                self.bloom_pass.update_settings(queue, &self.bloom_settings);
                self.bloom_settings_dirty = false;
            }
        }
    }

    pub fn set_bloom_settings(&mut self, settings: BloomSettings) {
        self.bloom_settings = settings.sanitized();
        self.bloom_settings_dirty = true;
    }

    pub fn bloom_settings(&self) -> &BloomSettings {
        &self.bloom_settings
    }

    pub fn render_ui_onto_final_frame(
        &self,
        encoder: &mut CommandEncoder,
        strobe: &mut StrobeRenderer,
        viewport: &RenderViewport,
        cache: &AssetCache,
        state: &State,
    ) {
        let final_color = &self.final_surfaces[Self::current_surface_index(viewport.frame_count())];

        UiRenderPass::render(encoder, strobe, final_color.view(), viewport, cache, state);
    }

    fn run_post_process_chain(
        &mut self,
        camera_render_data: &RenderUniformData,
        encoder: &mut CommandEncoder,
        cache: &AssetCache,
        final_output: TextureView,
    ) {
        let mut ctx = PostProcessPassContext {
            camera_render_data,
            encoder,
            cache,
        };

        let mut ping_index = 0usize;

        if self.route_key.run_ssr {
            let output_color = self.post_process_surfaces[ping_index].view();
            self.ssr_pass.execute(&mut ctx, output_color);
            ping_index = 1 - ping_index;
        }

        if self.route_key.run_ssao {
            let output_color = self.post_process_surfaces[ping_index].view();
            self.ssao_pass.execute(&mut ctx, output_color);
            ping_index = 1 - ping_index;
        }

        if self.route_key.run_bloom {
            let output_color = self.post_process_surfaces[ping_index].view();
            self.bloom_pass.execute(&mut ctx, output_color);
            ping_index = 1 - ping_index;
        }

        if self.route_key.run_fxaa {
            let output_color = self.post_process_surfaces[ping_index].view();
            self.fxaa_pass.execute(&mut ctx, output_color);
        }

        self.final_pass.execute(&mut ctx, &final_output);
    }

    pub fn finalize_frame(
        &mut self,
        encoder: &mut CommandEncoder,
        cache: &AssetCache,
        context: FinalFrameContext<'_>,
    ) -> RenderedFrame {
        // Determine effective post-processing routing
        let effective_key = if context.disable_post_processing {
            PostProcessRouting {
                run_ssr: false,
                run_ssao: false,
                run_bloom: false,
                run_fxaa: false,
            }
        } else {
            PostProcessRouting::current()
        };

        if effective_key != self.route_key {
            self.rebuild_post_process_passes(cache, effective_key);
        }

        let current_idx = Self::current_surface_index(context.frame_count);
        let final_color = &self.final_surfaces[current_idx];
        let final_frame_texture = final_color.texture().clone();

        self.run_post_process_chain(
            context.render_data,
            encoder,
            cache,
            final_color.view().clone(),
        );

        RenderedFrame {
            target: context.target,
            frame: final_frame_texture,
            size: context.size,
            format: context.format,
        }
    }
}
