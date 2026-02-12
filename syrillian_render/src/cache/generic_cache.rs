use crate::cache::AssetCache;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use syrillian_asset::store::{AssetKey, H, Store, StoreType, StoreTypeFallback};
use tracing::{trace, warn};
use wgpu::{Device, Queue};

type Slot<T> = <T as CacheType>::Hot;

pub struct Cache<T: CacheType> {
    data: DashMap<AssetKey, Slot<T>>,
    cache_misses: AtomicUsize,

    store: Arc<Store<T>>,
    device: Device,
    queue: Queue,
}

pub trait CacheType: Sized + StoreType {
    type Hot: Clone;
    fn upload(self, device: &Device, queue: &Queue, cache: &AssetCache) -> Self::Hot;
}

impl<T: CacheType + StoreTypeFallback> Cache<T> {
    pub fn get(&self, h: H<T>, cache: &AssetCache) -> T::Hot {
        self.data
            .entry(h.into())
            .or_insert_with(|| self.refresh_item(h, &self.device, &self.queue, cache))
            .clone()
    }

    #[profiling::function]
    fn refresh_item(&self, h: H<T>, device: &Device, queue: &Queue, cache: &AssetCache) -> T::Hot {
        let cold = {
            profiling::scope!("Store::get");
            self.store.get(h).clone()
        };

        let misses = self.cache_misses.load(Ordering::Acquire) + 1;
        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        trace!("Refreshing {} Cache Handle {}", T::NAME, h.ident_fmt());

        if misses.is_multiple_of(1000) {
            warn!(
                "[{} Cache] Invalid Handle: {}, Misses: {}",
                T::NAME,
                T::ident_fmt(h),
                misses
            );
        }

        cold.upload(device, queue, cache)
    }
}

impl<T: CacheType> Cache<T> {
    pub fn new(store: Arc<Store<T>>, device: Device, queue: Queue) -> Self {
        Cache {
            data: DashMap::new(),
            cache_misses: AtomicUsize::new(0),
            store,
            device,
            queue,
        }
    }

    pub fn store(&self) -> &Store<T> {
        &self.store
    }

    pub fn try_get(&self, h: H<T>, cache: &AssetCache) -> Option<T::Hot> {
        self.data
            .entry(h.into())
            .or_try_insert_with(|| self.try_refresh_item(h, &self.device, &self.queue, cache))
            .ok()
            .map(|h| h.clone())
    }

    pub fn refresh_dirty(&self) -> usize {
        let dirty = self.store.pop_dirty();

        for asset in &dirty {
            self.data.remove(asset);
        }

        dirty.len()
    }

    fn try_refresh_item(
        &self,
        h: H<T>,
        device: &Device,
        queue: &Queue,
        cache: &AssetCache,
    ) -> Result<T::Hot, ()> {
        let cold = self.store.try_get(h).ok_or(())?.clone();

        let misses = self.cache_misses.load(Ordering::Acquire) + 1;
        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        if misses.is_multiple_of(1000) {
            warn!(
                "[{} Cache] Invalid Handle: {}, Misses: {}",
                T::NAME,
                T::ident_fmt(h),
                misses
            );
        }

        Ok(cold.upload(device, queue, cache))
    }
}
