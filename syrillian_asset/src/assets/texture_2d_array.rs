use crate::store::{H, HandleName, StoreType};
use std::fmt::Debug;
use wgpu::{AddressMode, FilterMode, MipmapFilterMode, TextureFormat};

#[derive(Debug, Clone)]
pub struct Texture2DArray {
    pub width: u32,
    pub height: u32,
    pub array_layers: u32,
    pub format: TextureFormat,
    pub repeat_mode: AddressMode,
    pub filter_mode: FilterMode,
    pub mip_filter_mode: MipmapFilterMode,
    pub has_transparency: bool,
}

impl StoreType for Texture2DArray {
    const NAME: &str = "Texture 2D Array";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
