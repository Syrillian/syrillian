//! High-level renderer driving all drawing operations.
//!
//! The [`Renderer`] owns the [`State`], manages frame buffers and traverses
//! the all Scene Proxies it gets from the world to draw the latest snapshots of all world objects each frame.
//! It also provides debug drawing and post-processing utilities.

use super::error::*;
use crate::ViewportId;
use crate::components::TypedComponentId;
use crate::core::BoundingSphere;
use crate::engine::assets::{AssetStore, HTexture2D};
use crate::engine::rendering::FrameCtx;
use crate::engine::rendering::cache::{AssetCache, GpuTexture};
use crate::engine::rendering::offscreen_surface::OffscreenSurface;
use crate::engine::rendering::post_process_pass::PostProcessData;
use crate::math::{Mat4, UVec2, Vec3, Vec4};
#[cfg(debug_assertions)]
use crate::rendering::DebugRenderer;
use crate::rendering::light_manager::LightManager;
use crate::rendering::lights::LightType;
use crate::rendering::message::RenderMsg;
use crate::rendering::picking::{PickRequest, PickResult, color_bytes_to_hash};
use crate::rendering::proxies::SceneProxyBinding;
use crate::rendering::render_data::RenderUniformData;
use crate::rendering::strobe::StrobeRenderer;
use crate::rendering::texture_export::{TextureExportError, save_texture_to_png};
use crate::rendering::{GPUDrawCtx, RenderPassType, State};
use crossbeam_channel::Sender;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::mem;
use std::sync::{Arc, RwLock};
use syrillian_utils::debug_panic;
use tracing::{instrument, trace, warn};
use web_time::{Duration, Instant};
use wgpu::*;
use winit::dpi::PhysicalSize;

pub const PICKING_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
const PICKING_ROW_PITCH: u32 = 256;

pub struct RenderedFrame {
    pub target: ViewportId,
    pub texture: Texture,
    pub size: PhysicalSize<u32>,
    pub format: TextureFormat,
}

struct PickingSurface {
    texture: Texture,
    view: TextureView,
}

impl PickingSurface {
    fn new(device: &Device, config: &SurfaceConfiguration) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Picking Texture"),
            size: Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: PICKING_TEXTURE_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());

        Self { texture, view }
    }

    fn recreate(&mut self, device: &Device, config: &SurfaceConfiguration) {
        *self = Self::new(device, config);
    }

    fn view(&self) -> &TextureView {
        &self.view
    }

    fn texture(&self) -> &Texture {
        &self.texture
    }
}

pub struct RenderViewport {
    config: SurfaceConfiguration,
    depth_texture: Texture,
    offscreen_surface: OffscreenSurface,
    ssr_surface: OffscreenSurface,
    final_surface: OffscreenSurface,
    picking_surface: PickingSurface,
    post_process_ssr: PostProcessData,
    post_process_final: PostProcessData,
    g_normal: Texture,
    g_material: Texture,
    render_data: RenderUniformData,
    start_time: Instant,
    delta_time: Duration,
    last_frame_time: Instant,
    frame_count: usize,
}

impl RenderViewport {
    fn new(mut config: SurfaceConfiguration, state: &State, cache: &AssetCache) -> Self {
        Self::clamp_config(&mut config);

        let render_bgl = cache.bgl_render();
        let pp_bgl = cache.bgl_post_process();

        let picking_surface = PickingSurface::new(&state.device, &config);
        let offscreen_surface = OffscreenSurface::new(&state.device, &config);
        let ssr_surface = OffscreenSurface::new(&state.device, &config);
        let final_surface = OffscreenSurface::new(&state.device, &config);
        let depth_texture = Self::create_depth_texture(&state.device, &config);
        let normal_texture = Self::create_g_buffer("GBuffer (Normals)", &state.device, &config);
        let material_texture = Self::create_material_texture(&state.device, &config);
        let post_process_ssr = PostProcessData::new(
            &state.device,
            (*pp_bgl).clone(),
            offscreen_surface.view().clone(),
            depth_texture.create_view(&TextureViewDescriptor::default()),
            normal_texture.create_view(&TextureViewDescriptor::default()),
            material_texture.create_view(&TextureViewDescriptor::default()),
        );
        let post_process_final = PostProcessData::new(
            &state.device,
            (*pp_bgl).clone(),
            ssr_surface.view().clone(),
            depth_texture.create_view(&TextureViewDescriptor::default()),
            normal_texture.create_view(&TextureViewDescriptor::default()),
            material_texture.create_view(&TextureViewDescriptor::default()),
        );

        let render_data = RenderUniformData::empty(&state.device, &render_bgl);

        RenderViewport {
            config,
            depth_texture,
            offscreen_surface,
            ssr_surface,
            final_surface,
            picking_surface,
            post_process_ssr,
            post_process_final,
            g_normal: normal_texture,
            g_material: material_texture,
            render_data,
            start_time: Instant::now(),
            delta_time: Duration::default(),
            last_frame_time: Instant::now(),
            frame_count: 0,
        }
    }

    fn clamp_config(config: &mut SurfaceConfiguration) {
        config.width = config.width.max(1);
        config.height = config.height.max(1);
    }

    #[instrument(skip_all)]
    fn resize(&mut self, mut config: SurfaceConfiguration, state: &State, cache: &AssetCache) {
        Self::clamp_config(&mut config);
        self.config = config;

        self.offscreen_surface.recreate(&state.device, &self.config);
        self.ssr_surface.recreate(&state.device, &self.config);
        self.final_surface.recreate(&state.device, &self.config);
        self.depth_texture = Self::create_depth_texture(&state.device, &self.config);
        self.g_normal = Self::create_g_buffer("GBuffer (Normals)", &state.device, &self.config);
        self.g_material = Self::create_material_texture(&state.device, &self.config);
        self.picking_surface.recreate(&state.device, &self.config);
        let pp_bgl = cache.bgl_post_process();
        self.post_process_ssr = PostProcessData::new(
            &state.device,
            (*pp_bgl).clone(),
            self.offscreen_surface.view().clone(),
            self.depth_texture
                .create_view(&TextureViewDescriptor::default()),
            self.g_normal.create_view(&TextureViewDescriptor::default()),
            self.g_material
                .create_view(&TextureViewDescriptor::default()),
        );
        self.post_process_final = PostProcessData::new(
            &state.device,
            (*pp_bgl).clone(),
            self.ssr_surface.view().clone(),
            self.depth_texture
                .create_view(&TextureViewDescriptor::default()),
            self.g_normal.create_view(&TextureViewDescriptor::default()),
            self.g_material
                .create_view(&TextureViewDescriptor::default()),
        );
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

    #[instrument(skip_all)]
    fn begin_render(&mut self) -> FrameCtx {
        self.frame_count += 1;

        let depth_view = self
            .depth_texture
            .create_view(&TextureViewDescriptor::default());

        FrameCtx { depth_view }
    }

    fn update_render_data(&mut self, queue: &Queue) {
        self.update_system_data(queue);
    }

    fn update_view_camera_data(&mut self, queue: &Queue) {
        self.render_data.upload_camera_data(queue);
    }

    fn update_system_data(&mut self, queue: &Queue) {
        let window_size = UVec2::new(self.config.width.max(1), self.config.height.max(1));

        let system_data = &mut self.render_data.system_data;
        system_data.screen_size = window_size;
        system_data.time = self.start_time.elapsed().as_secs_f32();
        system_data.delta_time = self.delta_time.as_secs_f32();

        self.render_data.upload_system_data(queue);
    }

    /// Updates the delta time based on the elapsed time since the last frame
    fn tick_delta_time(&mut self) {
        self.delta_time = self.last_frame_time.elapsed();
        self.last_frame_time = Instant::now();
    }

    fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.config.width,
            height: self.config.height,
        }
    }
}

#[allow(dead_code)]
pub struct Renderer {
    pub state: Arc<State>,
    pub cache: AssetCache,
    shadow_render_data: RenderUniformData,
    viewports: HashMap<ViewportId, RenderViewport>,
    proxies: HashMap<TypedComponentId, SceneProxyBinding>,
    sorted_proxies: Vec<(u32, TypedComponentId)>,
    strobe: RefCell<StrobeRenderer>,
    start_time: Instant,
    pick_result_tx: Sender<PickResult>,
    pending_pick_requests: Vec<PickRequest>,
    pub(super) lights: LightManager,
}

impl Renderer {
    pub fn new(
        state: Arc<State>,
        store: Arc<AssetStore>,
        pick_result_tx: Sender<PickResult>,
        primary_config: SurfaceConfiguration,
    ) -> Result<Self> {
        let cache = AssetCache::new(store, state.as_ref());

        let render_bgl = cache.bgl_render();
        let shadow_render_data = RenderUniformData::empty(&state.device, &render_bgl);
        let lights = LightManager::new(&cache, &state.device);
        let start_time = Instant::now();

        let mut viewports = HashMap::new();
        viewports.insert(
            ViewportId::PRIMARY,
            RenderViewport::new(primary_config, state.as_ref(), &cache),
        );

        Ok(Renderer {
            state,
            cache,
            shadow_render_data,
            viewports,
            start_time,
            proxies: HashMap::new(),
            sorted_proxies: Vec::new(),
            strobe: RefCell::new(StrobeRenderer::default()),
            pick_result_tx,
            pending_pick_requests: Vec::new(),
            lights,
        })
    }

    fn take_pick_request(&mut self, target: ViewportId) -> Option<PickRequest> {
        if let Some(idx) = self
            .pending_pick_requests
            .iter()
            .rposition(|r| r.target == target)
        {
            Some(self.pending_pick_requests.swap_remove(idx))
        } else {
            None
        }
    }

    /// Export the offscreen render target for a viewport as a PNG image.
    pub fn export_offscreen_pngs(
        &self,
        target: ViewportId,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), TextureExportError> {
        let viewport = self
            .viewports
            .get(&target)
            .ok_or(TextureExportError::Unavailable {
                reason: "render target not found",
            })?;

        let path = path.as_ref();

        save_texture_to_png(
            &self.state.device,
            &self.state.queue,
            &viewport.g_normal,
            path.join("g_normal.png"),
        )?;

        save_texture_to_png(
            &self.state.device,
            &self.state.queue,
            &viewport.g_material,
            path.join("g_material.png"),
        )?;

        save_texture_to_png(
            &self.state.device,
            &self.state.queue,
            &viewport.depth_texture,
            path.join("offscreen_depth.png"),
        )?;

        save_texture_to_png(
            &self.state.device,
            &self.state.queue,
            viewport.offscreen_surface.texture(),
            path.join("offscreen_surface.png"),
        )
    }

    /// Export the picking buffer for a viewport as a PNG image.
    pub fn export_picking_png(
        &self,
        target: ViewportId,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), TextureExportError> {
        let viewport = self
            .viewports
            .get(&target)
            .ok_or(TextureExportError::Unavailable {
                reason: "render target not found",
            })?;

        save_texture_to_png(
            &self.state.device,
            &self.state.queue,
            viewport.picking_surface.texture(),
            path,
        )
    }

    /// Export any GPU texture to a PNG image.
    pub fn export_texture_png(
        &self,
        texture: &GpuTexture,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), TextureExportError> {
        save_texture_to_png(
            &self.state.device,
            &self.state.queue,
            &texture.texture,
            path,
        )
    }

    /// Export a cached texture handle to a PNG image.
    pub fn export_cached_texture_png(
        &self,
        texture: HTexture2D,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), TextureExportError> {
        let gpu_tex = self.cache.texture(texture);
        self.export_texture_png(&gpu_tex, path)
    }

    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    pub fn resize(&mut self, target_id: ViewportId, config: SurfaceConfiguration) -> bool {
        let Some(viewport) = self.viewports.get_mut(&target_id) else {
            warn!("Invalid Viewport {target_id:?} referenced");
            return false;
        };

        viewport.resize(config, &self.state, &self.cache);

        true
    }

    pub fn redraw(&mut self, target_id: ViewportId) -> bool {
        let Some(mut viewport) = self.viewports.remove(&target_id) else {
            warn!("Invalid Viewport {target_id:?} referenced");
            return false;
        };

        viewport.tick_delta_time();
        let rendered = self.render_frame_inner(target_id, &mut viewport);

        self.viewports.insert(target_id, viewport);

        rendered
    }

    #[instrument(skip_all)]
    pub fn update(&mut self) {
        let mut proxies = mem::take(&mut self.proxies);
        for proxy in proxies.values_mut() {
            proxy.update(self);
        }
        self.proxies = proxies;

        for vp in self.viewports.values_mut() {
            vp.update_render_data(&self.state.queue);
        }

        self.resort_proxies();

        self.lights
            .update(&self.cache, &self.state.queue, &self.state.device);
    }

    #[instrument(skip_all)]
    fn resort_proxies(&mut self) {
        let frustum = self
            .viewports
            .get(&ViewportId::PRIMARY)
            .map(|vp| Frustum::from_matrix(&vp.render_data.camera_data.proj_view_mat));

        self.sorted_proxies =
            sorted_enabled_proxy_ids(&self.proxies, self.cache.store(), frustum.as_ref());
    }

    #[instrument(skip_all)]
    fn render_frame_inner(&mut self, target_id: ViewportId, viewport: &mut RenderViewport) -> bool {
        let mut ctx = viewport.begin_render();

        let frustum = Frustum::from_matrix(&viewport.render_data.camera_data.proj_view_mat);
        self.sorted_proxies =
            sorted_enabled_proxy_ids(&self.proxies, self.cache.store(), Some(&frustum));

        if let Some(request) = self.take_pick_request(target_id) {
            self.picking_pass(viewport, &mut ctx, request);
        }

        self.render(target_id, viewport, &mut ctx);
        self.render_final_pass(viewport, viewport.final_surface.view());

        if self.cache.last_refresh().elapsed().as_secs_f32() > 5.0 {
            trace!("Refreshing cache...");
            let refreshed_count = self.cache.refresh_all();
            if cfg!(debug_assertions) && refreshed_count != 0 {
                trace!("Refreshed cache elements {}", refreshed_count);
            }
        }

        true
    }

    #[instrument(skip_all)]
    pub fn render_frame(&mut self, target_id: ViewportId) -> Option<RenderedFrame> {
        let mut viewport = self.viewports.remove(&target_id)?;
        viewport.tick_delta_time();

        let rendered = self.render_frame_inner(target_id, &mut viewport);
        let frame = if rendered {
            Some(RenderedFrame {
                target: target_id,
                texture: viewport.final_surface.texture().clone(),
                size: viewport.size(),
                format: viewport.config.format,
            })
        } else {
            None
        };

        self.viewports.insert(target_id, viewport);

        frame
    }

    pub fn render_all(&mut self) -> Vec<RenderedFrame> {
        let targets: Vec<_> = self.viewports.keys().copied().collect();
        let mut frames = Vec::with_capacity(targets.len());

        for target in targets {
            if let Some(frame) = self.render_frame(target) {
                frames.push(frame);
            }
        }

        frames
    }

    #[instrument(skip_all)]
    fn picking_pass(
        &mut self,
        viewport: &RenderViewport,
        ctx: &mut FrameCtx,
        request: PickRequest,
    ) {
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Picking Encoder"),
            });

        {
            let pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Picking Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: viewport.picking_surface.view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &ctx.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..RenderPassDescriptor::default()
            });

            self.render_scene(
                ctx,
                pass,
                RenderPassType::Picking,
                &self.sorted_proxies,
                &viewport.render_data,
            );
        }

        let render_ui = self.strobe.borrow().has_draws(request.target);
        if render_ui {
            let pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Picking UI Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: viewport.picking_surface.view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..RenderPassDescriptor::default()
            });

            let draw_ctx = GPUDrawCtx {
                frame: ctx,
                pass: RwLock::new(pass),
                pass_type: RenderPassType::PickingUi,
                render_bind_group: viewport.render_data.uniform.bind_group(),
                light_bind_group: self.lights.uniform().bind_group(),
                shadow_bind_group: self.lights.placeholder_shadow_uniform().bind_group(),
                transparency_pass: false,
            };
            let mut strobe = self.strobe.borrow_mut();

            strobe.render(
                &draw_ctx,
                &self.cache,
                &self.state,
                request.target,
                self.start_time,
                viewport.size(),
            );
        }

        let read_buffer = self.state.device.create_buffer(&BufferDescriptor {
            label: Some("Picking Readback Buffer"),
            size: PICKING_ROW_PITCH as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: viewport.picking_surface.texture(),
                mip_level: 0,
                origin: Origin3d {
                    x: request.position.0,
                    y: request.position.1,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &read_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(PICKING_ROW_PITCH),
                    rows_per_image: Some(1),
                },
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        self.state.queue.submit(Some(encoder.finish()));

        if let Some(result) = self.resolve_pick_buffer(read_buffer, request) {
            let _ = self.pick_result_tx.send(result);
        }
    }

    #[instrument(skip_all)]
    fn resolve_pick_buffer(&self, buffer: Buffer, request: PickRequest) -> Option<PickResult> {
        let slice = buffer.slice(0..4);
        let (tx, rx) = crossbeam_channel::bounded(1);
        slice.map_async(MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        let _ = self.state.device.poll(PollType::wait_indefinitely());

        if rx.recv().ok().and_then(Result::ok).is_none() {
            buffer.unmap();
            return None;
        }

        let data = slice.get_mapped_range();
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&data[..4]);
        drop(data);
        buffer.unmap();

        let hash = color_bytes_to_hash(bytes);

        Some(PickResult {
            id: request.id,
            target: request.target,
            hash,
        })
    }

    #[instrument(skip_all)]
    fn render(&mut self, target_id: ViewportId, viewport: &RenderViewport, ctx: &mut FrameCtx) {
        self.shadow_pass(ctx);
        self.main_pass(target_id, viewport, ctx);
    }

    #[instrument(skip_all)]
    fn shadow_pass(&mut self, ctx: &mut FrameCtx) {
        self.lights
            .update(&self.cache, &self.state.queue, &self.state.device);

        let shadow_layers = self
            .lights
            .shadow_array(&self.cache.store().render_texture_arrays)
            .unwrap()
            .array_layers;
        let light_count = self.lights.update_shadow_map_ids(shadow_layers);

        let assignments: Vec<_> = self
            .lights
            .shadow_assignments()
            .iter()
            .copied()
            .take(light_count as usize)
            .collect();

        // Shadow map ids and assignments may change when capacity is constrained, so upload the
        // updated proxy data again before the main pass consumes it.
        self.lights
            .update(&self.cache, &self.state.queue, &self.state.device);

        for assignment in assignments {
            let Some(light) = self.lights.light(assignment.light_index).copied() else {
                debug_panic!("Invalid light index");
                continue;
            };

            let Ok(light_type) = LightType::try_from(light.type_id) else {
                debug_panic!("Invalid Light Type Id was stored");
                continue;
            };

            match light_type {
                LightType::Spot => {
                    if assignment.face == 0 {
                        self.shadow_render_data
                            .update_shadow_camera_for_spot(&light, &self.state.queue);
                        self.prepare_shadow_map(ctx, assignment.layer);
                    }
                }
                LightType::Point => {
                    self.shadow_render_data.update_shadow_camera_for_point(
                        &light,
                        assignment.face,
                        &self.state.queue,
                    );
                    self.prepare_shadow_map(ctx, assignment.layer);
                }
                LightType::Sun => {}
            }
        }
    }

    #[instrument(skip_all)]
    fn prepare_shadow_map(&mut self, ctx: &mut FrameCtx, layer: u32) {
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Shadow Pass Encoder"),
            });

        let Some(layer_view) = self.lights.shadow_layer(&self.cache, layer) else {
            debug_panic!("Shadow layer view {layer} was not found");
            return;
        };

        let pass = self.prepare_shadow_pass(&mut encoder, &layer_view);

        self.render_scene(
            ctx,
            pass,
            RenderPassType::Shadow,
            &self.sorted_proxies,
            &self.shadow_render_data,
        );

        self.state.queue.submit(Some(encoder.finish()));
    }

    #[instrument(skip_all)]
    fn main_pass(&mut self, target_id: ViewportId, viewport: &RenderViewport, ctx: &mut FrameCtx) {
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Main Encoder"),
            });

        {
            let pass = self.prepare_main_render_pass(&mut encoder, viewport, ctx);

            self.render_scene(
                ctx,
                pass,
                RenderPassType::Color,
                &self.sorted_proxies,
                &viewport.render_data,
            );
        }

        let has_ui_draws_queued = self.strobe.borrow().has_draws(target_id);
        if has_ui_draws_queued {
            let pass = self.prepare_ui_render_pass(&mut encoder, viewport, ctx);

            let draw_ctx = GPUDrawCtx {
                frame: ctx,
                pass: RwLock::new(pass),
                pass_type: RenderPassType::Color2D,
                render_bind_group: viewport.render_data.uniform.bind_group(),
                light_bind_group: self.lights.uniform().bind_group(),
                shadow_bind_group: self.lights.placeholder_shadow_uniform().bind_group(),
                transparency_pass: false,
            };

            self.strobe.borrow_mut().render(
                &draw_ctx,
                &self.cache,
                &self.state,
                target_id,
                self.start_time,
                viewport.size(),
            );
        }

        self.state.queue.submit(Some(encoder.finish()));
    }

    #[instrument(skip_all)]
    fn render_scene(
        &self,
        frame_ctx: &FrameCtx,
        pass: RenderPass,
        pass_type: RenderPassType,
        proxies: &[(u32, TypedComponentId)],
        render_uniform: &RenderUniformData,
    ) {
        let shadow_bind_group = match pass_type {
            RenderPassType::Color | RenderPassType::Color2D => self.lights.shadow_uniform(),
            RenderPassType::Shadow | RenderPassType::Picking | RenderPassType::PickingUi => {
                self.lights.placeholder_shadow_uniform()
            }
        }
        .bind_group();

        let mut draw_ctx = GPUDrawCtx {
            frame: frame_ctx,
            pass: RwLock::new(pass),
            pass_type,
            render_bind_group: render_uniform.uniform.bind_group(),
            light_bind_group: self.lights.uniform().bind_group(),
            shadow_bind_group,
            transparency_pass: false,
        };

        self.render_proxies(&mut draw_ctx, proxies);

        #[cfg(debug_assertions)]
        if DebugRenderer::light() && pass_type == RenderPassType::Color {
            self.lights.render_debug_lights(self, &draw_ctx);
        }
    }

    #[instrument(skip_all)]
    fn render_proxies(&self, ctx: &mut GPUDrawCtx, proxies: &[(u32, TypedComponentId)]) {
        ctx.transparency_pass = false;

        for proxy in proxies.iter().map(|(_, ctid)| self.proxies.get(ctid)) {
            let Some(proxy) = proxy else {
                debug_panic!("Sorted proxy not in proxy list");
                continue;
            };
            proxy.render_by_pass(self, ctx);
        }

        match ctx.pass_type {
            RenderPassType::Color | RenderPassType::Shadow => (),
            RenderPassType::Picking => return,
            RenderPassType::Color2D | RenderPassType::PickingUi => {
                debug_panic!("Shouldn't render scene in 2D passes");
                return;
            }
        }

        ctx.transparency_pass = true;

        for proxy in proxies.iter().map(|(_, ctid)| self.proxies.get(ctid)) {
            let Some(proxy) = proxy else {
                debug_panic!("Sorted proxy not in proxy list");
                continue;
            };
            proxy.render_by_pass(self, ctx);
        }
    }

    #[instrument(skip_all)]
    fn render_final_pass(&mut self, viewport: &RenderViewport, color_view: &TextureView) {
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Final Pass Copy Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("SSR Post Process Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: viewport.ssr_surface.view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..RenderPassDescriptor::default()
            });

            let ssr_shader = self.cache.shader_post_process_ssr();
            let groups = ssr_shader.bind_groups();
            pass.set_pipeline(ssr_shader.solid_pipeline());
            pass.set_bind_group(
                groups.render,
                viewport.render_data.uniform.bind_group(),
                &[],
            );
            if let Some(idx) = groups.post_process {
                pass.set_bind_group(idx, viewport.post_process_ssr.uniform.bind_group(), &[]);
            }
            pass.draw(0..6, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Post Process Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..RenderPassDescriptor::default()
            });

            let post_shader = self.cache.shader_post_process();
            let groups = post_shader.bind_groups();
            pass.set_pipeline(post_shader.solid_pipeline());
            pass.set_bind_group(
                groups.render,
                viewport.render_data.uniform.bind_group(),
                &[],
            );
            if let Some(idx) = groups.post_process {
                pass.set_bind_group(idx, viewport.post_process_final.uniform.bind_group(), &[]);
            }
            pass.draw(0..6, 0..1);
        }

        self.state.queue.submit(Some(encoder.finish()));
    }

    #[instrument(skip_all)]
    pub fn handle_message(&mut self, msg: RenderMsg) {
        match msg {
            RenderMsg::RegisterProxy(cid, object_hash, mut proxy, local_to_world) => {
                trace!("Registered Proxy for #{:?}", cid.0);
                let data = proxy.setup_render(self, &local_to_world);
                let binding = SceneProxyBinding::new(cid, object_hash, local_to_world, data, proxy);
                self.proxies.insert(cid, binding);
            }
            RenderMsg::RegisterLightProxy(cid, proxy) => {
                trace!("Registered Light Proxy for #{:?}", cid.0);
                self.lights.add_proxy(cid, *proxy);
            }
            RenderMsg::RemoveProxy(cid) => {
                self.proxies.remove(&cid);
                self.lights.remove_proxy(cid);
            }
            RenderMsg::UpdateTransform(cid, ltw) => {
                if let Some(cid) = self.proxies.get_mut(&cid) {
                    cid.update_transform(ltw);
                }
            }
            RenderMsg::ProxyUpdate(cid, command) => {
                if let Some(binding) = self.proxies.get_mut(&cid) {
                    command(binding.proxy.as_mut());
                }
            }
            RenderMsg::LightProxyUpdate(cid, command) => {
                self.lights.execute_light_command(cid, command);
            }
            RenderMsg::UpdateActiveCamera(render_target_id, camera_data) => {
                if let Some(vp) = self.viewports.get_mut(&render_target_id) {
                    camera_data(&mut vp.render_data.camera_data);
                    vp.update_view_camera_data(&self.state.queue);
                }
            }
            RenderMsg::ProxyState(cid, enabled) => {
                if let Some(binding) = self.proxies.get_mut(&cid) {
                    binding.enabled = enabled;
                }
            }
            RenderMsg::PickRequest(request) => {
                if self.viewports.contains_key(&request.target) {
                    self.pending_pick_requests.push(request);
                } else {
                    debug_panic!("Picking Request contained invalid viewport target");
                }
            }
            RenderMsg::CommandBatch(batch) => {
                for message in batch {
                    self.handle_message(message);
                }
            }
            RenderMsg::CaptureOffscreenTextures(target, path) => {
                if let Err(e) = self.export_offscreen_pngs(target, &path) {
                    warn!("Couldn't capture offscreen texture: {e}");
                }
            }
            RenderMsg::CapturePickingTexture(target, path) => {
                if let Err(e) = self.export_picking_png(target, &path) {
                    warn!("Couldn't capture picking texture: {e}");
                }
            }
            RenderMsg::CaptureTexture(texture, path) => {
                if let Err(e) = self.export_cached_texture_png(texture, &path) {
                    warn!("Couldn't capture picking texture: {e}");
                }
            }
            RenderMsg::UpdateStrobe(frame) => {
                self.strobe.borrow_mut().update_frame(frame);
            }
            RenderMsg::FrameEnd(_, _) => {}
        }
    }

    pub fn add_viewport(
        &mut self,
        target_id: ViewportId,
        config: SurfaceConfiguration,
    ) -> Result<()> {
        if self.viewports.contains_key(&target_id) {
            warn!(
                "Viewport #{:?} already exists; ignoring duplicate add",
                target_id
            );
            return Ok(());
        }

        let viewport = RenderViewport::new(config, &self.state, &self.cache);
        self.viewports.insert(target_id, viewport);

        Ok(())
    }

    pub fn remove_viewport(&mut self, target_id: ViewportId) {
        self.viewports.remove(&target_id);
    }

    #[instrument(skip_all)]
    fn prepare_shadow_pass<'a>(
        &self,
        encoder: &'a mut CommandEncoder,
        shadow_map: &TextureView,
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: shadow_map,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..RenderPassDescriptor::default()
        })
    }

    #[instrument(skip_all)]
    fn prepare_main_render_pass<'a>(
        &self,
        encoder: &'a mut CommandEncoder,
        viewport: &RenderViewport,
        ctx: &mut FrameCtx,
    ) -> RenderPass<'a> {
        let g_normal_view = viewport
            .g_normal
            .create_view(&TextureViewDescriptor::default());
        let g_material_view = viewport
            .g_material
            .create_view(&TextureViewDescriptor::default());
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Offscreen Render Pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: viewport.offscreen_surface.view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &g_normal_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &g_material_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &ctx.depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..RenderPassDescriptor::default()
        })
    }

    #[instrument(skip_all)]
    fn prepare_ui_render_pass<'a>(
        &self,
        encoder: &'a mut CommandEncoder,
        viewport: &RenderViewport,
        _ctx: &mut FrameCtx,
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("UI Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: viewport.offscreen_surface.view(),
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..RenderPassDescriptor::default()
        })
    }
}

#[instrument(skip_all)]
fn sorted_enabled_proxy_ids(
    proxies: &HashMap<TypedComponentId, SceneProxyBinding>,
    store: &AssetStore,
    frustum: Option<&Frustum>,
) -> Vec<(u32, TypedComponentId)> {
    proxies
        .iter()
        .filter(|(_, binding)| binding.enabled)
        .filter_map(|(tid, binding)| {
            let priority = binding.proxy.priority(store);
            let mut distance = 0.0;
            if let Some(f) = frustum
                && let Some(bounds) = binding.bounds()
            {
                if !f.intersects_sphere(&bounds) {
                    return None;
                }
                distance = f.side(FrustumSide::Near).distance_to(&bounds);
            };

            Some((tid, priority, distance))
        })
        .sorted_by_key(|(_, priority, distance)| (*priority, -(*distance * 100000.0) as i64))
        .map(|(tid, priority, _)| (priority, *tid))
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct FrustumPlane {
    normal: Vec3,
    d: f32,
}

impl FrustumPlane {
    fn distance_to(&self, sphere: &BoundingSphere) -> f32 {
        self.normal.dot(sphere.center) + self.d
    }
}

#[derive(Debug, Clone, Copy)]
struct Frustum {
    planes: [FrustumPlane; 6],
}

#[allow(unused)]
enum FrustumSide {
    Left,
    Right,
    Bottom,
    Top,
    Near,
    Far,
}

impl Frustum {
    #[instrument(skip_all)]
    fn from_matrix(m: &Mat4) -> Self {
        let row0 = m.row(0);
        let row1 = m.row(1);
        let row2 = m.row(2);
        let row3 = m.row(3);

        let plane_from = |v: Vec4| {
            let normal = Vec3::new(v.x, v.y, v.z);
            let len = normal.length();
            if len > 0.0 {
                FrustumPlane {
                    normal: normal / len,
                    d: v.w / len,
                }
            } else {
                FrustumPlane { normal, d: v.w }
            }
        };

        let planes = [
            plane_from(row3 + row0), // left
            plane_from(row3 - row0), // right
            plane_from(row3 + row1), // bottom
            plane_from(row3 - row1), // top
            plane_from(row3 + row2), // near
            plane_from(row3 - row2), // far
        ];

        Frustum { planes }
    }

    pub fn side(&self, side: FrustumSide) -> &FrustumPlane {
        match side {
            FrustumSide::Left => &self.planes[0],
            FrustumSide::Right => &self.planes[1],
            FrustumSide::Bottom => &self.planes[2],
            FrustumSide::Top => &self.planes[3],
            FrustumSide::Near => &self.planes[4],
            FrustumSide::Far => &self.planes[5],
        }
    }

    #[instrument(skip_all)]
    fn intersects_sphere(&self, sphere: &BoundingSphere) -> bool {
        self.planes
            .iter()
            .all(|p| p.distance_to(sphere) >= -sphere.radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ComponentId;
    use crate::math::Affine3A;
    use crate::rendering::proxies::SceneProxy;
    use slotmap::Key;
    use std::any::{Any, TypeId};
    use std::collections::HashMap;

    #[derive(Debug)]
    struct TestProxy {
        priority: u32,
    }

    impl SceneProxy for TestProxy {
        fn setup_render(&mut self, _: &Renderer, _: &Affine3A) -> Box<dyn Any + Send> {
            Box::new(())
        }

        fn update_render(&mut self, _: &Renderer, _: &mut (dyn Any + Send), _: &Affine3A) {}

        fn render(&self, _renderer: &Renderer, _ctx: &GPUDrawCtx, _binding: &SceneProxyBinding) {}

        fn priority(&self, _: &AssetStore) -> u32 {
            self.priority
        }
    }

    #[test]
    fn resort_proxies_orders_by_priority() {
        struct MarkerLow;
        struct MarkerMid;
        struct MarkerHigh;

        let store = AssetStore::new();
        let mut proxies = HashMap::new();

        let id_high = insert_proxy::<MarkerHigh>(&mut proxies, 900, true);
        let id_low = insert_proxy::<MarkerLow>(&mut proxies, 10, true);
        let id_mid = insert_proxy::<MarkerMid>(&mut proxies, 50, true);

        let sorted = sorted_enabled_proxy_ids(&proxies, &store, None);
        assert_eq!(sorted, vec![(10, id_low), (50, id_mid), (900, id_high)]);
    }

    #[test]
    fn resort_proxies_ignores_disabled_bindings() {
        struct MarkerEnabled;
        struct MarkerDisabled;

        let store = AssetStore::new();
        let mut proxies = HashMap::new();

        let id_enabled = insert_proxy::<MarkerEnabled>(&mut proxies, 5, true);
        let id_disabled = insert_proxy::<MarkerDisabled>(&mut proxies, 1, false);

        let sorted = sorted_enabled_proxy_ids(&proxies, &store, None);
        assert_eq!(sorted, vec![(5, id_enabled)]);
        assert!(!sorted.contains(&(1, id_disabled)));
    }

    fn insert_proxy<T: 'static>(
        proxies: &mut HashMap<TypedComponentId, SceneProxyBinding>,
        priority: u32,
        enabled: bool,
    ) -> TypedComponentId {
        let tid = TypedComponentId(TypeId::of::<T>(), ComponentId::null());
        let mut binding = SceneProxyBinding::new(
            tid,
            1,
            Affine3A::IDENTITY,
            Box::new(()),
            Box::new(TestProxy { priority }),
        );
        binding.enabled = enabled;
        proxies.insert(tid, binding);
        tid
    }
}
