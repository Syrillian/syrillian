use super::{H, StoreType};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct AssetKey(u32);

impl<T: StoreType> From<H<T>> for AssetKey {
    fn from(handle: H<T>) -> Self {
        AssetKey(handle.id())
    }
}

impl<T: StoreType> From<AssetKey> for H<T> {
    fn from(key: AssetKey) -> Self {
        H::new(key.0)
    }
}
