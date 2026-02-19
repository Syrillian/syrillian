use snafu::Snafu;
use std::io;

#[derive(Debug, Snafu)]
#[snafu(context(suffix(Err)), visibility(pub))]
pub enum StreamingAssetError {
    #[snafu(transparent)]
    Io {
        source: io::Error,
    },
    HeaderRead {
        source: io::Error,
    },
    #[snafu(display("The header value of the streaming asset file was corrupted"))]
    HeaderMagicMismatch,
    #[snafu(display("Failed to read the index of the streaming asset: {source}"))]
    IndexRead {
        source: io::Error,
    },
    #[snafu(display("Failed to read a stored path from the streaming asset: {source}"))]
    PathRead {
        source: io::Error,
    },
    #[snafu(display(
        "Failed to read the index of the scene asset because it was corrupted (Tried to read {tried} bytes but found {actual})"
    ))]
    IndexCorrupted {
        tried: usize,
        actual: usize,
    },
    #[snafu(display("The streaming asset format version {version} is not supported"))]
    UnsupportedVersion {
        version: u32,
    },
    #[snafu(display("A stored path in the streaming asset was invalid UTF-8"))]
    InvalidPathData,
    #[snafu(display("A relative path was too long to store ({len} bytes): {path}"))]
    PathTooLong {
        path: String,
        len: usize,
    },
    #[snafu(display("An entry in the index was invalid as the asset type couldn't be determined"))]
    InvalidIndexEntry,
    #[snafu(display("Failed to parse asset source '{path}': {reason}"))]
    AssetParse {
        path: String,
        reason: String,
    },
    #[snafu(display("Failed to read the blob index of the streaming asset: {source}"))]
    BlobIndexRead {
        source: io::Error,
    },
    #[snafu(display("An entry in the blob index was invalid"))]
    InvalidBlobIndexEntry,
    #[snafu(display("Failed to read the payload of an asset entry: {source}"))]
    PayloadRead {
        source: io::Error,
    },
    #[snafu(display("Failed to read the bytes of a blob entry: {source}"))]
    BlobRead {
        source: io::Error,
    },
}

pub type Result<T, E = StreamingAssetError> = std::result::Result<T, E>;
