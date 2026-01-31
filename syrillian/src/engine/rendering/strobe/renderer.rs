use super::{StrobeFrame, UiDraw};
use crate::core::ModelUniform;
use crate::core::bone::BoneData;
use crate::rendering::cache::AssetCache;
use crate::rendering::proxies::mesh_proxy::{MeshUniformIndex, RuntimeMeshData};
use crate::rendering::proxies::text_proxy::TextRenderData;
use crate::rendering::uniform::ShaderUniform;
use crate::rendering::{FrameCtx, GPUDrawCtx, RenderPassType, State};
use crate::strobe::CacheId;
use delegate::delegate;
use glamx::Affine3A;
use std::collections::HashMap;
use std::mem;
use std::sync::RwLock;
use syrillian::ViewportId;
use web_time::Instant;
use wgpu::{BindGroup, BufferDescriptor, BufferUsages, RenderPass};
use winit::dpi::PhysicalSize;

#[derive(Default)]
pub struct StrobeRenderer {
    draws: HashMap<ViewportId, Vec<UiDraw>>,
    image_cache: HashMap<CacheId, RuntimeMeshData>,
    text_cache: HashMap<CacheId, TextRenderData>,
}

pub struct UiDrawContext<'a, 'b, 'c, 'd, 'e> {
    image_cache: &'a mut HashMap<CacheId, RuntimeMeshData>,
    text_cache: &'a mut HashMap<CacheId, TextRenderData>,
    gpu_ctx: &'b GPUDrawCtx<'e>,
    cache: &'c AssetCache,
    cache_id: CacheId,
    viewport_size: PhysicalSize<u32>,
    start_time: Instant,
    state: &'d State,
}

impl<'a, 'b, 'c, 'd, 'e> UiDrawContext<'a, 'b, 'c, 'd, 'e> {
    delegate! {
        to self {
            #[field]
            pub fn gpu_ctx(&self) -> &GPUDrawCtx<'e>;

            #[field]
            pub fn cache(&self) -> &AssetCache;

            #[field]
            pub fn cache_id(&self) -> CacheId;

            #[field]
            pub fn viewport_size(&self) -> PhysicalSize<u32>;

            #[field]
            pub fn start_time(&self) -> Instant;

            #[field]
            pub fn state(&self) -> &State;
        }

        to self.gpu_ctx {
            #[field(&)]
            pub fn pass(&self) -> &RwLock<RenderPass<'e>>;

            #[field]
            pub fn pass_type(&self) -> RenderPassType;

            #[field]
            pub fn frame(&self) -> &'b FrameCtx;

            #[field]
            pub fn render_bind_group(&self) -> &'b BindGroup;

            #[field]
            pub fn light_bind_group(&self) -> &'b BindGroup;

            #[allow(unused)]
            #[field]
            fn shadow_bind_group(&self) -> &'b BindGroup;
        }
    }

    pub(crate) fn ui_text_data(&mut self) -> &mut TextRenderData {
        self.text_cache.entry(self.cache_id).or_insert_with(|| {
            let model_bgl = self.cache.bgl_model();
            let model = ModelUniform::empty();
            let uniform = ShaderUniform::<MeshUniformIndex>::builder((*model_bgl).clone())
                .with_buffer_data(&model)
                .with_buffer_data(&BoneData::DUMMY)
                .build(&self.state.device);

            let glyph_vbo = self.state.device.create_buffer(&BufferDescriptor {
                label: Some("Strobe Text Glyphs"),
                size: 4,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            TextRenderData { uniform, glyph_vbo }
        })
    }

    pub(crate) fn ui_image_data(&mut self, model_mat: &Affine3A) -> &mut RuntimeMeshData {
        self.image_cache.entry(self.cache_id).or_insert_with(|| {
            let model_bgl = self.cache.bgl_model();
            let model = ModelUniform::empty();
            let uniform = ShaderUniform::<MeshUniformIndex>::builder((*model_bgl).clone())
                .with_buffer_data(&model)
                .with_buffer_data(&BoneData::DUMMY)
                .build(&self.state.device);

            let mesh_data = ModelUniform {
                model_mat: (*model_mat).into(),
            };

            RuntimeMeshData { mesh_data, uniform }
        })
    }
}

impl StrobeRenderer {
    pub fn update_frame(&mut self, frame: StrobeFrame) {
        self.draws.clear();
        for draw in frame.draws {
            let target = draw.draw_target();
            self.draws.entry(target).or_default().push(draw);
        }
    }

    pub fn has_draws(&self, target: ViewportId) -> bool {
        self.draws.get(&target).is_some_and(|d| !d.is_empty())
    }

    pub fn render(
        &mut self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        state: &State,
        target: ViewportId,
        start_time: Instant,
        viewport_size: PhysicalSize<u32>,
    ) {
        let draw_map = mem::take(&mut self.draws);
        let Some(draws) = draw_map.get(&target) else {
            self.draws = draw_map;
            return;
        };

        let mut current_context = UiDrawContext {
            image_cache: &mut self.image_cache,
            text_cache: &mut self.text_cache,
            gpu_ctx: ctx,
            cache,
            cache_id: 0,
            viewport_size,
            start_time,
            state,
        };

        for draw in draws.iter() {
            current_context.cache_id = draw.cache_id();
            draw.render(&mut current_context);
        }

        self.draws = draw_map;
    }
}
