use crate::store::streaming::asset_store::{StreamingAssetFile, StreamingAssetPayload};
use crate::store::streaming::error::Result;
use crate::store::streaming::packaged_scene::BuiltPayload;

pub trait StreamableAsset {
    fn encode(&self) -> BuiltPayload;
    fn decode(payload: &StreamingAssetPayload, package: &mut StreamingAssetFile) -> Result<Self>
    where
        Self: Sized;
}
