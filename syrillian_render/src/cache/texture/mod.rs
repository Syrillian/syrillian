use syrillian_asset::store::StoreType;
use wgpu::{
    AddressMode, Extent3d, FilterMode, MipmapFilterMode, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDimension,
};

mod cached;
mod render_textures;
mod textures;

pub use cached::GpuTexture;

pub trait TextureAsset: StoreType {
    fn desc(&self) -> TextureDescriptor<'_> {
        let layers = self.layer_count().max(1);

        // usage = usage | TextureUsages::TEXTURE_BINDING
        //     | TextureUsages::RENDER_ATTACHMENT
        //     | TextureUsages::COPY_SRC
        //     | TextureUsages::COPY_DST;

        TextureDescriptor {
            label: None,
            size: Extent3d {
                width: self.width(),
                height: self.height(),
                depth_or_array_layers: layers,
            },
            mip_level_count: self.mip_level_count(),
            sample_count: self.sample_count(),
            dimension: self.dimensions(),
            format: self.format(),
            usage: self.flags(),
            view_formats: self.view_formats(),
        }
    }

    fn view_desc(&self) -> wgpu::TextureViewDescriptor<'_> {
        wgpu::TextureViewDescriptor {
            label: Some("Texture View"),
            format: Some(self.format()),
            dimension: Some(self.view_dimension()),
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
            usage: None,
        }
    }

    fn sampler_desc(&self) -> wgpu::SamplerDescriptor<'_> {
        wgpu::SamplerDescriptor {
            address_mode_u: self.repeat_mode(),
            address_mode_v: self.repeat_mode(),
            address_mode_w: self.repeat_mode(),
            mag_filter: self.filter_mode(),
            min_filter: self.filter_mode(),
            mipmap_filter: self.mip_filter_mode(),
            ..Default::default()
        }
    }

    fn layer_count(&self) -> u32;
    fn flags(&self) -> TextureUsages;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn format(&self) -> TextureFormat;
    fn view_formats(&self) -> &[TextureFormat];
    fn mip_level_count(&self) -> u32;
    fn sample_count(&self) -> u32;
    fn dimensions(&self) -> TextureDimension;
    fn view_dimension(&self) -> TextureViewDimension;
    fn repeat_mode(&self) -> AddressMode;
    fn filter_mode(&self) -> FilterMode;
    fn mip_filter_mode(&self) -> MipmapFilterMode;
    fn data(&self) -> Option<&[u8]>;
    fn has_transparency(&self) -> bool;
}
