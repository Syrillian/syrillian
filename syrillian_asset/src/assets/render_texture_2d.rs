use crate::store::{H, HandleName, StoreType, UpdateAssetMessage};
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

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(
        &self,
        key: crate::store::AssetKey,
        assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        assets_tx
            .send((key, UpdateAssetMessage::UpdateRenderTexture2D(self.clone())))
            .is_ok()
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
