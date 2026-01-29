use crate::components::TypedComponentId;
use crate::core::ObjectHash;
use crate::rendering::lights::LightProxy;
use crate::rendering::message::RenderMsg;
use crate::rendering::proxies::SceneProxy;
use crate::strobe::{CacheId, UiDraw, UiImageDraw, UiLineDraw, UiTextDraw};
use crate::{ViewportId, World};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::RwLock;
use wgpu::{BindGroup, RenderPass, TextureView};

pub struct FrameCtx {
    pub depth_view: TextureView,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RenderPassType {
    Color,
    Color2D,
    Shadow,
    Picking,
    PickingUi,
}

pub struct GPUDrawCtx<'a> {
    pub pass: RwLock<RenderPass<'a>>,
    pub pass_type: RenderPassType,
    pub frame: &'a FrameCtx,
    pub render_bind_group: &'a BindGroup,
    pub light_bind_group: &'a BindGroup,
    pub shadow_bind_group: &'a BindGroup,
    pub transparency_pass: bool,
}

pub struct CPUDrawCtx<'a> {
    current_cid: TypedComponentId,
    batch: &'a mut Vec<RenderMsg>,
}

impl<'a> CPUDrawCtx<'a> {
    pub fn new(cid: TypedComponentId, batch: &'a mut Vec<RenderMsg>) -> Self {
        Self {
            current_cid: cid,
            batch,
        }
    }

    pub fn send_proxy_update(&mut self, cmd: impl FnOnce(&mut dyn SceneProxy) + Send + 'static) {
        let msg = RenderMsg::ProxyUpdate(self.current_cid, Box::new(cmd));
        self.batch.push(msg);
    }

    pub fn send_light_proxy_update(&mut self, cmd: impl FnOnce(&mut LightProxy) + Send + 'static) {
        let msg = RenderMsg::LightProxyUpdate(self.current_cid, Box::new(cmd));
        self.batch.push(msg);
    }

    pub fn disable_proxy(&mut self) {
        let msg = RenderMsg::ProxyState(self.current_cid, false);
        self.batch.push(msg);
    }

    pub fn enable_proxy(&mut self) {
        let msg = RenderMsg::ProxyState(self.current_cid, true);
        self.batch.push(msg);
    }
}

pub struct UiContext {
    current_id: CacheId,
}

impl UiContext {
    pub(crate) fn new(object_hash: ObjectHash, component_id: TypedComponentId) -> UiContext {
        let mut hasher = DefaultHasher::default();
        object_hash.hash(&mut hasher);
        component_id.0.hash(&mut hasher);
        component_id.1.hash(&mut hasher);

        UiContext {
            current_id: hasher.finish(),
        }
    }

    pub fn text(&self, world: &mut World, target: ViewportId, text: UiTextDraw) {
        world
            .strobe
            .draws
            .push(UiDraw::text(self.current_id, target, Box::new(text)));
    }

    pub fn image(&self, world: &mut World, target: ViewportId, image: UiImageDraw) {
        world
            .strobe
            .draws
            .push(UiDraw::image(self.current_id, target, Box::new(image)));
    }

    pub fn line(&self, world: &mut World, target: ViewportId, line: UiLineDraw) {
        world
            .strobe
            .draws
            .push(UiDraw::line(self.current_id, target, Box::new(line)))
    }
}
