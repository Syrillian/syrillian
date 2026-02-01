use super::StrobeFrame;
use crate::core::ModelUniform;
use crate::core::bone::BoneData;
use crate::math::{Mat4, Vec2};
use crate::rendering::cache::AssetCache;
use crate::rendering::proxies::mesh_proxy::{MeshUniformIndex, RuntimeMeshData};
use crate::rendering::proxies::text_proxy::TextRenderData;
use crate::rendering::uniform::ShaderUniform;
use crate::rendering::{FrameCtx, GPUDrawCtx, RenderPassType, State};
use crate::strobe::ui_element::Rect;
use crate::strobe::{CacheId, ContextWithId, LayoutElement, StrobeRoot};
use delegate::delegate;
use std::collections::HashMap;
use std::mem;
use std::sync::RwLock;
use syrillian::ViewportId;
use web_time::Instant;
use wgpu::{BindGroup, BufferDescriptor, BufferUsages, RenderPass};
use winit::dpi::PhysicalSize;

#[derive(Default)]
pub struct StrobeRenderer {
    strobe_roots: HashMap<ViewportId, Vec<StrobeRoot>>,
    image_cache: HashMap<(CacheId, u64), RuntimeMeshData>,
    text_cache: HashMap<(CacheId, u64), TextRenderData>,
}

pub struct UiDrawContext<'a, 'b, 'c, 'd, 'e> {
    image_cache: &'a mut HashMap<(CacheId, u64), RuntimeMeshData>,
    text_cache: &'a mut HashMap<(CacheId, u64), TextRenderData>,
    gpu_ctx: &'b GPUDrawCtx<'e>,
    cache: &'c AssetCache,
    cache_id: CacheId,
    pub render_id: u32,
    viewport_size: PhysicalSize<u32>,
    start_time: Instant,
    state: &'d State,
}

impl ContextWithId for UiDrawContext<'_, '_, '_, '_, '_> {
    fn set_id(&mut self, id: u32) {
        self.render_id = id;
    }
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
        let key = (self.cache_id, self.render_id as u64);
        self.text_cache.entry(key).or_insert_with(|| {
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

    pub(crate) fn ui_image_data(&mut self, model_mat: &Mat4) -> &mut RuntimeMeshData {
        let key = (self.cache_id, self.render_id as u64);
        self.image_cache.entry(key).or_insert_with(|| {
            let model_bgl = self.cache.bgl_model();
            let mesh_data = ModelUniform {
                model_mat: *model_mat,
            };

            let uniform = ShaderUniform::<MeshUniformIndex>::builder((*model_bgl).clone())
                .with_buffer_data(&mesh_data)
                .with_buffer_data(&BoneData::DUMMY)
                .build(&self.state.device);

            RuntimeMeshData { mesh_data, uniform }
        })
    }
}

impl StrobeRenderer {
    pub fn update_frame(&mut self, frame: StrobeFrame) {
        self.strobe_roots.clear();

        for root in frame.strobe_roots {
            let target = root.target;
            self.strobe_roots.entry(target).or_default().push(root);
        }
    }

    pub fn has_draws(&self, target: ViewportId) -> bool {
        self.strobe_roots
            .get(&target)
            .is_some_and(|r| !r.is_empty())
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
        let roots_map = mem::take(&mut self.strobe_roots);

        let roots = roots_map.get(&target);

        if roots.is_none() {
            self.strobe_roots = roots_map;
            return;
        }

        let mut current_context = UiDrawContext {
            image_cache: &mut self.image_cache,
            text_cache: &mut self.text_cache,
            gpu_ctx: ctx,
            cache,
            cache_id: 0,
            render_id: 0,
            viewport_size,
            start_time,
            state,
        };

        if let Some(roots) = roots {
            let full_rect = Rect::new(
                Vec2::ZERO,
                Vec2::new(viewport_size.width as f32, viewport_size.height as f32),
            );

            for root in roots {
                current_context.cache_id = root.cache_id;
                current_context.render_id = root.root.id;
                root.root.render_layout(&mut current_context, full_rect);
            }
        }

        self.strobe_roots = roots_map;
    }
}
