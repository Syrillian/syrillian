use crate::cache::TextureAsset;
use crate::rendering::TextureFormat;
use std::slice;
use syrillian_asset::{RenderCubemap, RenderTexture2D, RenderTexture2DArray};
use wgpu::{
    AddressMode, FilterMode, MipmapFilterMode, TextureDimension, TextureUsages,
    TextureViewDimension,
};

impl TextureAsset for RenderTexture2D {
    fn layer_count(&self) -> u32 {
        1
    }

    fn flags(&self) -> TextureUsages {
        TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST
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
        slice::from_ref(&self.format)
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
        TextureViewDimension::D2
    }

    fn repeat_mode(&self) -> AddressMode {
        AddressMode::Repeat
    }

    fn filter_mode(&self) -> FilterMode {
        FilterMode::Nearest
    }

    fn mip_filter_mode(&self) -> MipmapFilterMode {
        MipmapFilterMode::Nearest
    }

    fn data(&self) -> Option<&[u8]> {
        None
    }

    fn has_transparency(&self) -> bool {
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

impl TextureAsset for RenderCubemap {
    fn layer_count(&self) -> u32 {
        6
    }

    fn flags(&self) -> TextureUsages {
        TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST
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
        TextureViewDimension::Cube
    }

    fn repeat_mode(&self) -> AddressMode {
        AddressMode::Repeat
    }

    fn filter_mode(&self) -> FilterMode {
        FilterMode::Nearest
    }

    fn mip_filter_mode(&self) -> MipmapFilterMode {
        MipmapFilterMode::Nearest
    }

    fn data(&self) -> Option<&[u8]> {
        None
    }

    fn has_transparency(&self) -> bool {
        false
    }
}
