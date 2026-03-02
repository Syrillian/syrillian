use crate::store::streaming::asset_store::AssetType;
use crate::store::{AssetKey, AssetRefreshMessage, H, HandleName, StoreType, UpdateAssetMessage};
use crossbeam_channel::Sender;
use std::fmt::Debug;
use wgpu::{AddressMode, FilterMode, MipmapFilterMode, TextureFormat};

#[derive(Debug, Clone)]
pub struct RenderTexture2DArray {
    pub width: u32,
    pub height: u32,
    pub array_layers: u32,
    pub format: TextureFormat,
    pub repeat_mode: AddressMode,
    pub filter_mode: FilterMode,
    pub mip_filter_mode: MipmapFilterMode,
    pub has_transparency: bool,
}

impl RenderTexture2DArray {
    pub fn new_shadow_map(capacity: u32, width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            array_layers: capacity,
            format: TextureFormat::Depth32Float,
            repeat_mode: AddressMode::Repeat,
            filter_mode: FilterMode::Linear,
            mip_filter_mode: MipmapFilterMode::Linear,
            has_transparency: false,
        }
    }
}

impl StoreType for RenderTexture2DArray {
    const NAME: &str = "Render Texture 2D Array";
    const TYPE: AssetType = AssetType::RenderTexture2DArray;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(&self, key: AssetKey, assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        assets_tx
            .send(AssetRefreshMessage::updated(
                key,
                UpdateAssetMessage::UpdateRenderTexture2DArray(self.clone()),
            ))
            .is_ok()
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
