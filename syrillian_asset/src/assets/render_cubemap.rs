use crate::store::{H, HandleName, StoreType};
use wgpu::TextureFormat;

#[derive(Debug, Clone)]
pub struct RenderCubemap {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

impl StoreType for RenderCubemap {
    fn name() -> &'static str {
        "Render Cubemap"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
