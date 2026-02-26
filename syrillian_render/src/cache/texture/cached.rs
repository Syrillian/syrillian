use crate::cache::{AssetCache, CacheType, TextureAsset};
use std::sync::Arc;
use wgpu::{Device, Extent3d, Queue, Sampler, TextureFormat, TextureView};

#[derive(Debug)]
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub(super) full_view: TextureView,
    pub(super) views: Vec<TextureView>,
    pub(super) sampler: Sampler,
    pub has_transparency: bool,
}

impl GpuTexture {
    pub fn size(&self) -> Extent3d {
        self.texture.size()
    }

    pub fn format(&self) -> TextureFormat {
        self.texture.format()
    }

    pub fn array_layers(&self) -> u32 {
        self.texture.depth_or_array_layers()
    }

    /// The entire view of the texture including all layers
    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    /// The entire view of the texture including all layers
    pub fn view(&self) -> &TextureView {
        &self.full_view
    }

    pub fn layer_view(&self, layer: u32) -> Option<&TextureView> {
        self.views.get(layer as usize)
    }
}

impl<T: TextureAsset> CacheType for T {
    type Hot = Arc<GpuTexture>;
    type UpdateMessage = Self;

    fn upload(this: Self, device: &Device, queue: &Queue, _cache: &AssetCache) -> Self::Hot {
        this.upload(device, queue)
    }
}
