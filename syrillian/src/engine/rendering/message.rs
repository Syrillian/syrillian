use crate::ViewportId;
use crate::assets::HTexture2D;
use crate::components::TypedComponentId;
use crate::core::ObjectHash;
use crate::math::Affine3A;
use crate::rendering::lights::LightProxy;
use crate::rendering::picking::PickRequest;
use crate::rendering::proxies::SceneProxy;
use crate::rendering::render_data::CameraUniform;
use crate::rendering::strobe::StrobeFrame;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;

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
    UpdateStrobe(StrobeFrame),
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
            RenderMsg::UpdateStrobe(_) => "Update Strobe Draw List",
        };

        write!(f, "{name}")
    }
}
