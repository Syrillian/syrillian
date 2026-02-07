use crate::rendering::offscreen_surface::OffscreenSurface;
use crate::rendering::post_process::ScreenSpaceReflectionRenderPass;
use crate::rendering::post_process::ui_pass::UiRenderPass;
use crate::rendering::post_process_pass::PostProcessData;
use crate::rendering::render_data::RenderUniformData;
use crate::rendering::viewport::RenderViewport;
use crate::rendering::{AssetCache, RenderedFrame, State, TextureFormat};
use crate::strobe::StrobeRenderer;
use syrillian_utils::EngineArgs;
use wgpu::{
    Color, CommandEncoder, Device, Extent3d, LoadOp, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, StoreOp, SurfaceConfiguration, Texture, TextureDescriptor,
    TextureDimension, TextureUsages, TextureViewDescriptor,
};

pub struct RenderPipeline {
    pub depth_texture: Texture,
    pub offscreen_surface: OffscreenSurface,
    pub ssr_pass: ScreenSpaceReflectionRenderPass,
    pub ui_pass: UiRenderPass,
    pub final_surfaces: [OffscreenSurface; 2],
    pub final_data: PostProcessData,
    pub g_normal: Texture,
    pub g_material: Texture,
}

impl RenderPipeline {
    pub fn new(device: &Device, cache: &AssetCache, config: &SurfaceConfiguration) -> Self {
        let pp_bgl = cache.bgl_post_process();

        let normal_texture = Self::create_g_buffer("GBuffer (Normals)", device, config);
        let material_texture = Self::create_material_texture(device, config);
        let depth_texture = Self::create_depth_texture(device, config);
        let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());
        let normal_view = normal_texture.create_view(&TextureViewDescriptor::default());
        let material_view = material_texture.create_view(&TextureViewDescriptor::default());

        let offscreen_surface = OffscreenSurface::new(device, config);
        let final_surfaces = [
            OffscreenSurface::new(device, config),
            OffscreenSurface::new(device, config),
        ];

        let ssr_pass = ScreenSpaceReflectionRenderPass::new(
            device,
            config,
            pp_bgl.clone(),
            &offscreen_surface,
            depth_view.clone(),
            normal_view.clone(),
            material_view.clone(),
        );

        let post_process_final = if EngineArgs::get().no_ssr {
            PostProcessData::new(
                device,
                pp_bgl.clone(),
                offscreen_surface.view().clone(),
                depth_view.clone(),
                normal_view.clone(),
                material_view.clone(),
            )
        } else {
            PostProcessData::new(
                device,
                pp_bgl.clone(),
                ssr_pass.output.view().clone(),
                depth_view.clone(),
                normal_view.clone(),
                material_view.clone(),
            )
        };

        let ui_pass = UiRenderPass::new();

        Self {
            depth_texture,
            offscreen_surface,
            ssr_pass,
            ui_pass,
            final_surfaces,
            final_data: post_process_final,
            g_normal: normal_texture,
            g_material: material_texture,
        }
    }

    pub fn recreate(&mut self, device: &Device, cache: &AssetCache, config: &SurfaceConfiguration) {
        *self = Self::new(device, cache, config);
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

    pub fn render_post_process(
        &self,
        camera_render_data: &RenderUniformData,
        encoder: &mut CommandEncoder,
        cache: &AssetCache,
    ) {
        if !EngineArgs::get().no_ssr {
            self.ssr_pass.render(camera_render_data, encoder, cache);
        }
    }

    pub fn render_ui_onto_final_frame(
        &self,
        encoder: &mut CommandEncoder,
        strobe: &mut StrobeRenderer,
        viewport: &RenderViewport,
        cache: &AssetCache,
        state: &State,
    ) {
        let final_color = &self.final_surfaces[viewport.frame_count() % 2];

        viewport.render_pipeline.ui_pass.render(
            encoder,
            strobe,
            final_color.view(),
            viewport,
            cache,
            state,
        );
    }

    pub fn finalize_frame(
        &self,
        encoder: &mut CommandEncoder,
        viewport: &RenderViewport,
        cache: &AssetCache,
    ) -> RenderedFrame {
        let final_color = &self.final_surfaces[viewport.frame_count() % 2];
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Final Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: final_color.view(),
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

        let post_shader = cache.shader_post_process();
        let groups = post_shader.bind_groups();
        pass.set_pipeline(post_shader.solid_pipeline());
        pass.set_bind_group(
            groups.render,
            viewport.render_data.uniform.bind_group(),
            &[],
        );
        if let Some(idx) = groups.post_process {
            pass.set_bind_group(idx, self.final_data.uniform.bind_group(), &[]);
        }
        pass.draw(0..6, 0..1);

        RenderedFrame {
            target: viewport.id,
            frame: final_color.texture().clone(),
            size: viewport.size(),
            format: viewport.config.format,
        }
    }
}
