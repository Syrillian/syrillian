use crate::store::{H, HandleName, StoreType};
use std::fmt::Debug;
use wgpu::TextureFormat;

#[derive(Debug, Clone)]
pub struct RenderTexture2D {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

impl StoreType for RenderTexture2D {
    fn name() -> &'static str {
        "Render Texture 2D"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
