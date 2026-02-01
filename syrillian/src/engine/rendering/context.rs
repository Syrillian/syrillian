use crate::components::TypedComponentId;
use crate::core::ObjectHash;
use crate::rendering::lights::LightProxy;
use crate::rendering::message::RenderMsg;
use crate::rendering::proxies::SceneProxy;
use crate::strobe::{CacheId, StrobeNode, StrobeRoot, UiBuilder};
use crate::{ViewportId, World};
use glamx::vec2;
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

    pub fn draw(
        &self,
        world: &mut World,
        target: ViewportId,
        ui: impl FnOnce(&mut UiBuilder),
    ) -> bool {
        let Some(size) = world.viewport_size(target) else {
            return false;
        };

        let mut root = StrobeNode::default();
        let mut builder = UiBuilder::new(&mut root, vec2(size.width as f32, size.height as f32));
        ui(&mut builder);

        if root.children.is_empty() && root.element.is_none() {
            return true;
        }

        world.strobe.strobe_roots.push(StrobeRoot {
            root,
            target,
            cache_id: self.current_id,
        });

        true
    }
}
