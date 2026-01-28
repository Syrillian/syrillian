use crate::assets::{H, HandleName, StoreType, TextureAsset};
use crate::rendering::TextureFormat;
use std::fmt::Debug;
use wgpu::{
    AddressMode, FilterMode, MipmapFilterMode, TextureDimension, TextureUsages,
    TextureViewDimension,
};

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
    fn name() -> &'static str {
        "Render Texture 2D Array"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}

impl TextureAsset for RenderTexture2DArray {
    fn layer_count(&self) -> u32 {
        self.array_layers
    }

    fn flags(&self) -> TextureUsages {
        TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::COPY_SRC
            | TextureUsages::RENDER_ATTACHMENT
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn format(&self) -> TextureFormat {
        self.format
    }

    fn view_formats(&self) -> &[TextureFormat] {
        std::slice::from_ref(&self.format)
    }

    fn mip_level_count(&self) -> u32 {
        1
    }

    fn sample_count(&self) -> u32 {
        1
    }

    fn dimensions(&self) -> TextureDimension {
        TextureDimension::D2
    }

    fn view_dimension(&self) -> TextureViewDimension {
        TextureViewDimension::D2Array
    }

    fn repeat_mode(&self) -> AddressMode {
        self.repeat_mode
    }

    fn filter_mode(&self) -> FilterMode {
        self.filter_mode
    }

    fn mip_filter_mode(&self) -> MipmapFilterMode {
        self.mip_filter_mode
    }

    fn data(&self) -> Option<&[u8]> {
        None
    }

    fn has_transparency(&self) -> bool {
        self.has_transparency
    }
}
