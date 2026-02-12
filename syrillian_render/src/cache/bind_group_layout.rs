use crate::cache::{AssetCache, CacheType};
use syrillian_asset::BGL;
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, Device, Queue};

impl CacheType for BGL {
    type Hot = BindGroupLayout;

    #[profiling::function]
    fn upload(self, device: &Device, _queue: &Queue, _cache: &AssetCache) -> Self::Hot {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some(&self.label),
            entries: self.entries.as_slice(),
        })
    }
}
