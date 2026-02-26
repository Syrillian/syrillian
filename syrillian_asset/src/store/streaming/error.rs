use crate::store::streaming::asset_store::AssetType;
use snafu::Snafu;
use std::io;
use std::sync::Arc;

fn some_and_arc(
    err: Box<dyn std::error::Error + Send + Sync>,
) -> Option<Arc<Box<dyn std::error::Error + Send + Sync>>> {
    Some(Arc::new(err))
}

#[derive(Debug, Clone, Snafu)]
#[snafu(context(suffix(Err)), visibility(pub))]
pub enum AssetStreamingError {
    #[snafu(transparent)]
    Io {
        #[snafu(source(from(io::Error, Arc::new)))]
        source: Arc<io::Error>,
    },
    HeaderRead {
        #[snafu(source(from(io::Error, Arc::new)))]
        source: Arc<io::Error>,
    },
    #[snafu(display("The header value of the decoding asset file was corrupted"))]
    HeaderMagicMismatch,
    #[snafu(display("Failed to read the index of the decoding asset: {source}"))]
    IndexRead {
        #[snafu(source(from(io::Error, Arc::new)))]
        source: Arc<io::Error>,
    },
    #[snafu(display("Failed to read a stored path from the decoding asset: {source}"))]
    PathRead {
        #[snafu(source(from(io::Error, Arc::new)))]
        source: Arc<io::Error>,
    },
    #[snafu(display(
        "Failed to read the index of the scene asset because it was corrupted (Tried to read {tried} bytes but found {actual})"
    ))]
    IndexCorrupted { tried: usize, actual: usize },
    #[snafu(display("The decoding asset format version {version} is not supported"))]
    UnsupportedVersion { version: u32 },
    #[snafu(display("A stored path in the decoding asset was invalid UTF-8"))]
    InvalidPathData,
    #[snafu(display("A relative path was too long to store ({len} bytes): {path}"))]
    PathTooLong { path: String, len: usize },
    #[snafu(display("An entry in the index was invalid as the asset type couldn't be determined"))]
    InvalidIndexEntry,
    #[snafu(display("Failed to parse asset source '{path}': {reason}"))]
    AssetParse { path: String, reason: String },
    #[snafu(display("Failed to read the blob index of the decoding asset: {source}"))]
    BlobIndexRead {
        #[snafu(source(from(io::Error, Arc::new)))]
        source: Arc<io::Error>,
    },
    #[snafu(display("An entry in the blob index was invalid"))]
    InvalidBlobIndexEntry,
    #[snafu(display("Failed to read the payload of an asset entry: {source}"))]
    PayloadRead {
        #[snafu(source(from(io::Error, Arc::new)))]
        source: Arc<io::Error>,
    },
    #[snafu(display("Failed to read the bytes of a blob entry: {source}"))]
    BlobRead {
        #[snafu(source(from(io::Error, Arc::new)))]
        source: Arc<io::Error>,
    },
    #[snafu(display("Blob was expected to exist, but wasn't found"))]
    BlobNotFound,

    #[snafu(display("Failed to load package {path:?}: {reason}"))]
    PackageLoad { path: String, reason: String },

    #[snafu(display("Failed to scan package directory {path:?}: {reason}"))]
    PackageScan { path: String, reason: String },

    #[snafu(display("No packaged asset entry found for path {path:?}"))]
    AssetNotFound { path: String },

    #[snafu(display("Asset mapping for {path:?} changed while loading"))]
    AssetStale { path: String },

    #[snafu(display("No packaged asset entry found for hash 0x{hash:016x}"))]
    HashNotFound { hash: u64 },

    #[snafu(display(
        "Packaged asset {path:?} has type '{actual}', but '{expected}' was requested"
    ))]
    TypeMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[snafu(display("The decoding worker is not running"))]
    WorkerNotRunning,

    #[snafu(display("Failed to enqueue {path:?} because the decoding worker is not available"))]
    WorkerChannelClosed { path: String },

    #[snafu(display("Failed to spawn decoding worker thread: {reason}"))]
    WorkerSpawn { reason: String },

    #[snafu(display("The package entry for {path:?} no longer exists"))]
    PackageIndexMissing { path: String },

    #[snafu(display("Failed to read packaged asset {path:?}: {reason}"))]
    PackageRead { path: String, reason: String },

    #[snafu(
        display("Packaged asset {path:?} has unsupported runtime type '{}'", asset_type.name())
    )]
    UnsupportedType { path: String, asset_type: AssetType },

    #[snafu(whatever, display("{message}"))]
    Decode {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error + Send + Sync>, some_and_arc)))]
        source: Option<Arc<Box<dyn std::error::Error + Send + Sync>>>,
    },

    #[snafu(display("Payload was not a JSON Object: {source}"))]
    InvalidJsonPayload {
        #[snafu(source(from(serde_json::Error, Arc::new)))]
        source: Arc<serde_json::Error>,
    },

    #[snafu(display("{label} blob element count {actual} did not match expected {expected}"))]
    BlobSize {
        label: String,
        expected: usize,
        actual: usize,
    },

    #[snafu(display("{label} blob byte length {actual} did not match expected {expected}"))]
    BlobByteLength {
        label: String,
        expected: usize,
        actual: usize,
    },

    #[snafu(display("The mount of {path} was overwritten while asset was loading"))]
    OverwrittenMount { path: String },
}

pub type Result<T, E = AssetStreamingError> = std::result::Result<T, E>;
