use crate::assets::{H, HandleName, StoreType, TextureAsset};
use crate::rendering::TextureFormat;
use std::fmt::Debug;
use wgpu::{
    AddressMode, FilterMode, MipmapFilterMode, TextureDimension, TextureUsages,
    TextureViewDimension,
};

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
    fn name() -> &'static str {
        "Texture 2D Array"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}

impl TextureAsset for Texture2DArray {
    fn layer_count(&self) -> u32 {
        self.array_layers
    }

    fn flags(&self) -> TextureUsages {
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC
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
