use crate::store::streaming::error::*;
use snafu::{OptionExt, ResultExt, ensure};
use std::collections::HashMap;
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;
use zerocopy::native_endian::*;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, TryFromBytes, Unaligned};

pub const STREAMING_ASSET_VERSION: u32 = 1;
pub const MAGIC_SIGNATURE_BYTES: [u8; 4] = *b"SYRN";
pub const MAGIC_SIGNATURE: i32 = i32::from_ne_bytes(MAGIC_SIGNATURE_BYTES);

#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Hash,
    TryFromBytes,
    IntoBytes,
    KnownLayout,
    Immutable,
    Unaligned,
)]
#[repr(u8)]
pub enum AssetType {
    Mesh = 0,
    SkinnedMesh = 1,
    Shader = 2,
    ComputeShader = 3,
    Texture2D = 4,
    Texture2DArray = 5,
    Cubemap = 6,
    RenderTexture2D = 7,
    RenderTexture2DArray = 8,
    RenderCubemap = 9,
    Material = 10,
    MaterialInstance = 11,
    BGL = 12,
    Font = 13,
    Sound = 14,
    AnimationClip = 15,
    Prefab = 16,
}

impl AssetType {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    pub const fn name(self) -> &'static str {
        match self {
            AssetType::Mesh => "Mesh",
            AssetType::Shader => "Shader",
            AssetType::ComputeShader => "ComputeShader",
            AssetType::Texture2D => "Texture2D",
            AssetType::Texture2DArray => "Texture2DArray",
            AssetType::Cubemap => "Cubemap",
            AssetType::RenderTexture2D => "RenderTexture2D",
            AssetType::RenderTexture2DArray => "RenderTexture2DArray",
            AssetType::RenderCubemap => "RenderCubemap",
            AssetType::Material => "Material",
            AssetType::MaterialInstance => "MaterialInstance",
            AssetType::BGL => "BGL",
            AssetType::Font => "Font",
            AssetType::Sound => "Sound",
            AssetType::AnimationClip => "AnimationClip",
            AssetType::Prefab => "Prefab",
            AssetType::SkinnedMesh => "SkinnedMesh",
        }
    }

    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(AssetType::Mesh),
            1 => Some(AssetType::SkinnedMesh),
            2 => Some(AssetType::Shader),
            3 => Some(AssetType::ComputeShader),
            4 => Some(AssetType::Texture2D),
            5 => Some(AssetType::Texture2DArray),
            6 => Some(AssetType::Cubemap),
            7 => Some(AssetType::RenderTexture2D),
            8 => Some(AssetType::RenderTexture2DArray),
            9 => Some(AssetType::RenderCubemap),
            10 => Some(AssetType::Material),
            11 => Some(AssetType::MaterialInstance),
            12 => Some(AssetType::BGL),
            13 => Some(AssetType::Font),
            14 => Some(AssetType::Sound),
            15 => Some(AssetType::AnimationClip),
            16 => Some(AssetType::Prefab),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum StreamingAssetBlobKind {
    TextureData = 0,
    MeshVertices = 1,
    MeshIndices = 2,
    BonesInverseBind = 3,
    BonesBindGlobal = 4,
    BonesBindLocal = 5,
    AnimationTranslationTimes = 6,
    AnimationTranslationValues = 7,
    AnimationRotationTimes = 8,
    AnimationRotationValues = 9,
    AnimationScaleTimes = 10,
    AnimationScaleValues = 11,
}

impl StreamingAssetBlobKind {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::TextureData),
            1 => Some(Self::MeshVertices),
            2 => Some(Self::MeshIndices),
            3 => Some(Self::BonesInverseBind),
            4 => Some(Self::BonesBindGlobal),
            5 => Some(Self::BonesBindLocal),
            6 => Some(Self::AnimationTranslationTimes),
            7 => Some(Self::AnimationTranslationValues),
            8 => Some(Self::AnimationRotationTimes),
            9 => Some(Self::AnimationRotationValues),
            10 => Some(Self::AnimationScaleTimes),
            11 => Some(Self::AnimationScaleValues),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::TextureData => "TextureData",
            Self::MeshVertices => "MeshVertices",
            Self::MeshIndices => "MeshIndices",
            Self::BonesInverseBind => "BonesInverseBind",
            Self::BonesBindGlobal => "BonesBindGlobal",
            Self::BonesBindLocal => "BonesBindLocal",
            Self::AnimationTranslationTimes => "AnimationTranslationTimes",
            Self::AnimationTranslationValues => "AnimationTranslationValues",
            Self::AnimationRotationTimes => "AnimationRotationTimes",
            Self::AnimationRotationValues => "AnimationRotationValues",
            Self::AnimationScaleTimes => "AnimationScaleTimes",
            Self::AnimationScaleValues => "AnimationScaleValues",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StreamingAssetBlobInfo {
    pub kind: StreamingAssetBlobKind,
    pub offset: u64,
    pub size: u64,
    pub element_count: usize,
}

#[derive(Clone, TryFromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
pub struct StreamingAssetIndexEntryRaw {
    pub asset_type: AssetType,
    pub path_len: U32,
    pub path_offset: U64,
    pub offset: U64,
    pub size: U64,
    pub hash: U64,
}

#[derive(Clone, TryFromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
pub struct StreamingAssetBlobIndexEntryRaw {
    pub owner_hash: U64,
    pub kind: u8,
    pub reserved: [u8; 7],
    pub offset: U64,
    pub size: U64,
    pub element_count: U64,
}

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
pub struct StreamingAssetHeader {
    pub magic: I32,
    pub version: U32,
    pub asset_count: U64,
    pub blob_count: U64,
    pub blob_index_offset: U64,
    pub blob_data_offset: U64,
}

#[derive(Clone)]
pub struct StreamingAssetIndexEntry {
    pub asset_type: AssetType,
    pub offset: u64,
    pub size: u64,
    pub hash: u64,
    pub relative_path: Option<String>,
}

pub struct StreamingAssetIndex {
    data: HashMap<u64, StreamingAssetIndexEntry>,
}

pub struct StreamingAssetBlobIndex {
    data: HashMap<u64, Vec<StreamingAssetBlobInfo>>,
}

pub struct StreamingAssetFile {
    pub handle: File,
    pub header: StreamingAssetHeader,
    pub index: StreamingAssetIndex,
    pub blobs: StreamingAssetBlobIndex,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StreamingAssetEntryInfo {
    pub asset_type: AssetType,
    pub offset: u64,
    pub size: u64,
    pub hash: u64,
    pub relative_path: Option<String>,
    pub blob_count: usize,
    pub blob_size: u64,
}

pub struct StreamingAssetPayload {
    pub data: serde_json::Value,
    pub blob_infos: StreamingAssetBlobInfos,
}

pub struct StreamingAssetBlobInfos {
    pub infos: Vec<StreamingAssetBlobInfo>,
}

impl StreamingAssetBlobInfos {
    pub fn find(
        &self,
        kind: StreamingAssetBlobKind,
    ) -> Result<&StreamingAssetBlobInfo, AssetStreamingError> {
        self.infos
            .iter()
            .find(|blob| blob.kind == kind)
            .context(BlobNotFoundErr)
    }
}

impl StreamingAssetFile {
    pub fn load<P: AsRef<Path>>(sto_path: P) -> Result<Self> {
        let path = sto_path.as_ref();
        let mut handle = File::open(path)?;
        let header = StreamingAssetHeader::parse_from_io(&mut handle)?;
        let index = StreamingAssetIndex::parse_from_io(&mut handle, &header)?;
        let blobs = StreamingAssetBlobIndex::parse_from_io(&mut handle, &header)?;
        Ok(Self {
            handle,
            header,
            index,
            blobs,
        })
    }

    pub fn version(&self) -> u32 {
        self.header.version.get()
    }

    pub fn asset_count(&self) -> u64 {
        self.header.asset_count.get()
    }

    pub fn blob_count(&self) -> u64 {
        self.header.blob_count.get()
    }

    pub fn entries(&self) -> Vec<StreamingAssetEntryInfo> {
        let mut entries = self
            .index
            .data
            .values()
            .map(|entry| StreamingAssetEntryInfo {
                asset_type: entry.asset_type,
                offset: entry.offset,
                size: entry.size,
                hash: entry.hash,
                relative_path: entry.relative_path.clone(),
                blob_count: self.blobs.data.get(&entry.hash).map_or(0, Vec::len),
                blob_size: self
                    .blobs
                    .data
                    .get(&entry.hash)
                    .map_or(0, |items| items.iter().map(|item| item.size).sum()),
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.offset);
        entries
    }

    pub fn path_for_hash(&self, hash: u64) -> Option<&str> {
        self.index
            .data
            .get(&hash)
            .and_then(|entry| entry.relative_path.as_deref())
    }

    pub fn blobs_for_hash(&self, hash: u64) -> &[StreamingAssetBlobInfo] {
        self.blobs.data.get(&hash).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn entry_by_path(&self, relative_path: &str) -> Option<StreamingAssetEntryInfo> {
        self.entries().into_iter().find(|entry| {
            entry
                .relative_path
                .as_deref()
                .is_some_and(|path| path == relative_path)
        })
    }

    pub fn entry_by_hash(&self, hash: u64) -> Option<StreamingAssetEntryInfo> {
        self.entries().into_iter().find(|entry| entry.hash == hash)
    }

    pub fn read_payload(
        &mut self,
        entry: &StreamingAssetEntryInfo,
        path: &str,
    ) -> Result<StreamingAssetPayload, AssetStreamingError> {
        let payload =
            self.read_payload_bytes(entry)
                .map_err(|source| AssetStreamingError::PackageRead {
                    path: path.to_string(),
                    reason: source.to_string(),
                })?;

        let payload = serde_json::from_slice(&payload).context(InvalidJsonPayloadErr)?;

        let blob_infos = self.blobs_for_hash(entry.hash).to_vec();
        let blob_infos = StreamingAssetBlobInfos { infos: blob_infos };

        Ok(StreamingAssetPayload {
            data: payload,
            blob_infos,
        })
    }

    pub fn read_payload_bytes(&mut self, entry: &StreamingAssetEntryInfo) -> Result<Vec<u8>> {
        self.handle
            .seek(SeekFrom::Start(entry.offset))
            .context(PayloadReadErr)?;

        let mut bytes = vec![0_u8; entry.size as usize];
        self.handle.read_exact(&mut bytes).context(PayloadReadErr)?;
        Ok(bytes)
    }

    pub fn read_blob_bytes(&mut self, blob: &StreamingAssetBlobInfo) -> Result<Vec<u8>> {
        self.handle
            .seek(SeekFrom::Start(blob.offset))
            .context(BlobReadErr)?;
        let mut bytes = vec![0_u8; blob.size as usize];
        self.handle.read_exact(&mut bytes).context(BlobReadErr)?;
        Ok(bytes)
    }
}

impl StreamingAssetHeader {
    pub fn parse_from_io(handle: &mut File) -> Result<Self> {
        let header = Self::read_from_io(handle).context(HeaderReadErr)?;

        ensure!(
            header.magic.as_ref() == &MAGIC_SIGNATURE_BYTES,
            HeaderMagicMismatchErr
        );

        Ok(header)
    }
}

impl StreamingAssetIndex {
    pub fn parse_from_io(handle: &mut File, header: &StreamingAssetHeader) -> Result<Self> {
        let version = header.version.get();
        ensure!(
            version == STREAMING_ASSET_VERSION,
            UnsupportedVersionErr { version }
        );

        let asset_count = header.asset_count.get() as usize;
        let index_size = asset_count * size_of::<StreamingAssetIndexEntryRaw>();
        let start_offset = size_of::<StreamingAssetHeader>() as u64;

        handle
            .seek(SeekFrom::Start(start_offset))
            .context(IndexReadErr)?;

        let mut entries_buffer = vec![0_u8; index_size];
        handle
            .read_exact(&mut entries_buffer)
            .context(IndexReadErr)?;

        let mut data = HashMap::new();
        let mut remaining = entries_buffer.as_slice();

        for _ in 0..asset_count {
            let (entry, suffix) = StreamingAssetIndexEntryRaw::try_read_from_prefix(remaining)
                .map_err(|_| AssetStreamingError::InvalidIndexEntry)?;

            let mut path_bytes = vec![0_u8; entry.path_len.get() as usize];
            handle
                .seek(SeekFrom::Start(entry.path_offset.get()))
                .context(PathReadErr)?;
            handle.read_exact(&mut path_bytes).context(PathReadErr)?;
            let relative_path =
                String::from_utf8(path_bytes).map_err(|_| AssetStreamingError::InvalidPathData)?;

            data.insert(
                entry.hash.get(),
                StreamingAssetIndexEntry {
                    asset_type: entry.asset_type,
                    offset: entry.offset.get(),
                    size: entry.size.get(),
                    hash: entry.hash.get(),
                    relative_path: Some(relative_path),
                },
            );
            remaining = suffix;
        }

        Ok(Self { data })
    }
}

impl StreamingAssetBlobIndex {
    pub fn parse_from_io(handle: &mut File, header: &StreamingAssetHeader) -> Result<Self> {
        let blob_count = header.blob_count.get() as usize;
        if blob_count == 0 {
            return Ok(Self {
                data: HashMap::new(),
            });
        }

        let start_offset = header.blob_index_offset.get();
        handle
            .seek(SeekFrom::Start(start_offset))
            .context(BlobIndexReadErr)?;

        let index_size = blob_count * size_of::<StreamingAssetBlobIndexEntryRaw>();
        let mut entries_buffer = vec![0_u8; index_size];
        handle
            .read_exact(&mut entries_buffer)
            .context(BlobIndexReadErr)?;

        let mut data: HashMap<u64, Vec<StreamingAssetBlobInfo>> = HashMap::new();
        let mut remaining = entries_buffer.as_slice();

        for _ in 0..blob_count {
            let (entry, suffix) = StreamingAssetBlobIndexEntryRaw::try_read_from_prefix(remaining)
                .map_err(|_| AssetStreamingError::InvalidBlobIndexEntry)?;
            let kind = StreamingAssetBlobKind::from_u8(entry.kind)
                .ok_or(AssetStreamingError::InvalidBlobIndexEntry)?;

            data.entry(entry.owner_hash.get())
                .or_default()
                .push(StreamingAssetBlobInfo {
                    kind,
                    offset: entry.offset.get(),
                    size: entry.size.get(),
                    element_count: entry.element_count.get() as usize,
                });
            remaining = suffix;
        }

        for blobs in data.values_mut() {
            blobs.sort_by_key(|blob| blob.offset);
        }

        Ok(Self { data })
    }
}

pub fn hash_relative_path(relative_path: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    relative_path.hash(&mut hasher);
    hasher.finish()
}

pub fn normalize_asset_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.starts_with("./") {
        normalized = normalized[2..].to_string();
    }
    while normalized.starts_with('/') {
        normalized.remove(0);
    }
    normalized
}
