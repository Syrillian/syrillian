use crate::rendering::viewport::RenderViewport;
use crate::rendering::{AssetCache, RenderPassType, State};
use crate::strobe::{StrobeRenderer, UiGPUContext};
use parking_lot::RwLock;
use wgpu::{
    CommandEncoder, LoadOp, Operations, RenderPass, RenderPassColorAttachment,
    RenderPassDescriptor, StoreOp, TextureView,
};

pub struct UiRenderPass {}

impl UiRenderPass {
    pub fn new() -> Self {
        Self {}
    }

    pub fn begin_pass<'a>(
        &self,
        output: &'a TextureView,
        encoder: &'a mut CommandEncoder,
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("UI Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: output,
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

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        strobe: &mut StrobeRenderer,
        output: &TextureView,
        viewport: &RenderViewport,
        cache: &AssetCache,
        state: &State,
    ) {
        let has_ui_draws_queued = strobe.has_draws(viewport.id);
        if has_ui_draws_queued {
            let pass = self.begin_pass(output, encoder);

            let draw_ctx = UiGPUContext {
                pass: RwLock::new(pass),
                pass_type: RenderPassType::Color2D,
                render_bind_group: viewport.render_data.uniform.bind_group(),
            };

            strobe.render(&draw_ctx, cache, state, viewport);
        }
    }
}
