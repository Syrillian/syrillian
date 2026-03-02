use crate::store::streaming::asset_store::AssetType;
use crate::store::{AssetKey, AssetRefreshMessage, H, HandleName, StoreType, UpdateAssetMessage};
use crossbeam_channel::Sender;
use std::fmt::Debug;
use wgpu::TextureFormat;

#[derive(Debug, Clone)]
pub struct RenderTexture2D {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

impl StoreType for RenderTexture2D {
    const NAME: &str = "Render Texture 2D";
    const TYPE: AssetType = AssetType::RenderTexture2D;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(&self, key: AssetKey, assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        assets_tx
            .send(AssetRefreshMessage::updated(
                key,
                UpdateAssetMessage::UpdateRenderTexture2D(self.clone()),
            ))
            .is_ok()
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
