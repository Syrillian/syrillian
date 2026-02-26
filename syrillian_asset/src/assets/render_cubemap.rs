use crate::store::{H, HandleName, StoreType, UpdateAssetMessage};
use crossbeam_channel::Sender;
use wgpu::TextureFormat;

#[derive(Debug, Clone)]
pub struct RenderCubemap {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

impl StoreType for RenderCubemap {
    const NAME: &str = "Render Cubemap";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(
        &self,
        key: crate::store::AssetKey,
        assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        assets_tx
            .send((key, UpdateAssetMessage::UpdateRenderCubemap(self.clone())))
            .is_ok()
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
