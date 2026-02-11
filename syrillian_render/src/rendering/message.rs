use crate::ObjectHash;
use crate::lighting::proxy::LightProxy;
use crate::proxies::SceneProxy;
use crate::rendering::picking::PickRequest;
use crate::rendering::render_data::CameraUniform;
use crate::rendering::viewport::ViewportId;
use crate::strobe::StrobeFrame;
use crossbeam_channel::Sender;
use glamx::Affine3A;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use syrillian_asset::HTexture2D;
use syrillian_utils::TypedComponentId;

#[derive(Debug, Clone, Copy)]
pub struct GBufferDebugTargets {
    pub normal: HTexture2D,
    pub material: HTexture2D,
}

pub type ProxyUpdateCommand = Box<dyn FnOnce(&mut dyn SceneProxy) + Send>;
pub type LightProxyCommand = Box<dyn FnOnce(&mut LightProxy) + Send>;
pub type CameraUpdateCommand = Box<dyn FnOnce(&mut CameraUniform) + Send>;

pub enum RenderMsg {
    RegisterProxy(TypedComponentId, ObjectHash, Box<dyn SceneProxy>, Affine3A),
    RegisterLightProxy(TypedComponentId, Box<LightProxy>),
    RemoveProxy(TypedComponentId),
    UpdateTransform(TypedComponentId, Affine3A),
    ProxyUpdate(TypedComponentId, ProxyUpdateCommand),
    LightProxyUpdate(TypedComponentId, LightProxyCommand),
    UpdateActiveCamera(ViewportId, CameraUpdateCommand),
    ProxyState(TypedComponentId, bool), // enabled
    PickRequest(PickRequest),
    CommandBatch(Vec<RenderMsg>),
    CaptureOffscreenTextures(ViewportId, PathBuf),
    CapturePickingTexture(ViewportId, PathBuf),
    CaptureTexture(HTexture2D, PathBuf),
    SetGBufferDebug(ViewportId, Option<GBufferDebugTargets>),
    UpdateStrobe(StrobeFrame),
    FrameEnd(ViewportId, Sender<()>),
}

impl Debug for RenderMsg {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            RenderMsg::RegisterProxy(..) => "Register Proxy",
            RenderMsg::RegisterLightProxy(..) => "Register Light Proxy",
            RenderMsg::RemoveProxy(_) => "Remove Proxy",
            RenderMsg::UpdateTransform(..) => "Update Transform",
            RenderMsg::ProxyUpdate(..) => "Proxy Update",
            RenderMsg::LightProxyUpdate(..) => "Light Proxy Update",
            RenderMsg::UpdateActiveCamera(..) => "Update Active Camera",
            RenderMsg::ProxyState(_, enable) => &format!("Proxy Enabled: {enable}"),
            RenderMsg::PickRequest(..) => "Pick Request",
            RenderMsg::CommandBatch(inner) => &format!("Command Batch {inner:?}"),
            RenderMsg::CaptureOffscreenTextures(_, _) => "Capture Offscreen Texture",
            RenderMsg::CapturePickingTexture(_, _) => "Capture Picking Texture",
            RenderMsg::CaptureTexture(_, _) => "Capture Texture",
            RenderMsg::SetGBufferDebug(_, _) => "Set GBuffer Debug",
            RenderMsg::UpdateStrobe(_) => "Update Strobe Draw List",
            RenderMsg::FrameEnd(_, _) => "Frame End",
        };

        write!(f, "{name}")
    }
}
