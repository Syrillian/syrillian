use crate::cache::{AssetCache, CacheType, TextureAsset};
use std::sync::Arc;
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Device, Extent3d, Queue, Sampler, TextureFormat, TextureView};

#[derive(Debug)]
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: TextureView,
    pub sampler: Sampler,
    pub size: Extent3d,
    pub format: TextureFormat,
    pub has_transparency: bool,
}

impl<T: TextureAsset> CacheType for T {
    type Hot = Arc<GpuTexture>;

    fn upload(self, device: &Device, queue: &Queue, _cache: &AssetCache) -> Self::Hot {
        profiling::function_scope!("upload texture");
        let desc = self.desc();

        let texture = match self.data() {
            None => device.create_texture(&self.desc()),
            Some(data) => {
                device.create_texture_with_data(queue, &desc, TextureDataOrder::LayerMajor, data)
            }
        };

        let view = texture.create_view(&self.view_desc());
        let sampler = device.create_sampler(&self.sampler_desc());

        Arc::new(GpuTexture {
            texture,
            view,
            sampler,
            size: desc.size,
            format: desc.format,
            has_transparency: self.has_transparency(),
        })
    }
}
