use crate::cache::{AssetCache, CacheType};
use syrillian_asset::BGL;
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, Device, Queue};

impl CacheType for BGL {
    type Hot = BindGroupLayout;
    type UpdateMessage = Self;

    #[profiling::function]
    fn upload(this: Self, device: &Device, _queue: &Queue, _cache: &AssetCache) -> Self::Hot {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some(&this.label),
            entries: this.entries.as_slice(),
        })
    }
}
