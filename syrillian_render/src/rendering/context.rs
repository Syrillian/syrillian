use crate::lighting::proxy::LightProxy;
use crate::proxies::SceneProxy;
use crate::rendering::message::RenderMsg;
use parking_lot::RwLock;
use syrillian_utils::TypedComponentId;
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
