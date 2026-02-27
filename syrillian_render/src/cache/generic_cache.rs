use crate::cache::AssetCache;
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use syrillian_asset::store::{AssetKey, H, StoreType, StoreTypeFallback};
use tracing::warn;
use wgpu::{Device, Queue};

type Slot<T> = <T as CacheType>::Hot;

pub struct Cache<T: CacheType> {
    data: DashMap<AssetKey, Slot<T>>,
    cache_misses: AtomicUsize,

    device: Device,
    queue: Queue,
}

pub trait CacheType: Sized + StoreType {
    type Hot: Clone;
    type UpdateMessage;
    fn upload(
        msg: Self::UpdateMessage,
        device: &Device,
        queue: &Queue,
        cache: &AssetCache,
    ) -> Self::Hot;
}

impl<T: CacheType + StoreTypeFallback> Cache<T> {
    pub fn get(&self, h: H<T>) -> T::Hot {
        if let Some(item) = self.try_get(h) {
            return item;
        }

        self.data
            .get(&T::fallback().into())
            .expect("Fallback Asset was not pre-cached")
            .clone()
    }
}

impl<T: CacheType> Cache<T> {
    pub fn new(device: Device, queue: Queue) -> Self {
        Cache {
            data: DashMap::new(),
            cache_misses: AtomicUsize::new(0),
            device,
            queue,
        }
    }

    pub fn try_get(&self, h: H<T>) -> Option<T::Hot> {
        if let Some(item) = self.data.get(&h.into()) {
            return Some(item.clone());
        };

        let misses = self.cache_misses.fetch_add(1, Ordering::SeqCst);

        if misses.is_multiple_of(1000) {
            warn!(
                "[{} Cache] Invalid Handle: {}, Misses: {}",
                T::NAME,
                T::ident_fmt(h),
                misses
            );
        }

        None
    }

    pub fn inspect<F, R>(&self, h: H<T>, call: F) -> Option<R>
    where
        F: Fn(&T::Hot) -> R,
    {
        if let Some(item) = self.data.get(&h.into()) {
            return Some(call(item.value()));
        };

        let misses = self.cache_misses.fetch_add(1, Ordering::SeqCst);

        if misses.is_multiple_of(1000) {
            warn!(
                "[{} Cache] Invalid Handle: {}, Misses: {}",
                T::NAME,
                T::ident_fmt(h),
                misses
            );
        }

        None
    }

    #[profiling::function]
    pub fn refresh_item(&self, key: AssetKey, message: T::UpdateMessage, cache: &AssetCache) {
        self.data
            .insert(key, T::upload(message, &self.device, &self.queue, cache));
    }

    pub fn remove(&self, key: AssetKey) {
        self.data.remove(&key);
    }
}
