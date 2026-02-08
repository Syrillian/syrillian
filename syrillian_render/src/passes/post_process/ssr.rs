use crate::cache::AssetCache;
use crate::passes::post_process::PostProcessData;
use crate::rendering::offscreen_surface::OffscreenSurface;
use crate::rendering::render_data::RenderUniformData;
use wgpu::{
    BindGroupLayout, Color, CommandEncoder, Device, LoadOp, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, StoreOp, SurfaceConfiguration, TextureView,
};

pub struct ScreenSpaceReflectionRenderPass {
    pub output: OffscreenSurface,
    pub data: PostProcessData,
}

impl ScreenSpaceReflectionRenderPass {
    pub fn new(
        device: &Device,
        config: &SurfaceConfiguration,
        post_process_bgl: BindGroupLayout,
        color_input: &OffscreenSurface,
        depth_view: TextureView,
        g_normal_view: TextureView,
        g_material_view: TextureView,
    ) -> Self {
        let ssr_surface_output = OffscreenSurface::new(device, config);
        let data = PostProcessData::new(
            device,
            post_process_bgl,
            color_input.view().clone(),
            depth_view,
            g_normal_view,
            g_material_view,
        );

        Self {
            output: ssr_surface_output,
            data,
        }
    }

    pub fn render(
        &self,
        camera_render_data: &RenderUniformData,
        encoder: &mut CommandEncoder,
        cache: &AssetCache,
    ) {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("SSR Post Process Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: self.output.view(),
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

        let ssr_shader = cache.shader_post_process_ssr();
        let groups = ssr_shader.bind_groups();
        pass.set_pipeline(ssr_shader.solid_pipeline());
        pass.set_bind_group(groups.render, camera_render_data.uniform.bind_group(), &[]);
        if let Some(idx) = groups.post_process {
            pass.set_bind_group(idx, self.data.uniform.bind_group(), &[]);
        }
        pass.draw(0..6, 0..1);
    }
}
