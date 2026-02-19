use crate::store::streaming_asset_store::{AssetType, StreamingAssetBlobKind};
use crate::{AnimationClip, Mesh, PrefabAsset, PrefabMaterial, Texture2D};

#[derive(Debug, Clone)]
pub struct PackagedScene {
    pub virtual_root: String,
    pub meshes: Vec<PackagedMeshAsset>,
    pub textures: Vec<PackagedTextureAsset>,
    pub materials: Vec<PackagedMaterialAsset>,
    pub animations: Vec<PackagedAnimationAsset>,
    pub prefab: PackagedPrefabAsset,
}

#[derive(Debug, Clone)]
pub struct PackagedMeshAsset {
    pub virtual_path: String,
    pub mesh: Mesh,
}

#[derive(Debug, Clone)]
pub struct PackagedTextureAsset {
    pub virtual_path: String,
    pub texture: Texture2D,
}

#[derive(Debug, Clone)]
pub struct PackagedMaterialAsset {
    pub virtual_path: String,
    pub material: PrefabMaterial,
}

#[derive(Debug, Clone)]
pub struct PackagedAnimationAsset {
    pub virtual_path: String,
    pub clip: AnimationClip,
}

#[derive(Debug, Clone)]
pub struct PackagedPrefabAsset {
    pub virtual_path: String,
    pub prefab: PrefabAsset,
}

pub struct PackedAsset {
    pub asset_type: AssetType,
    pub relative_path: String,
    pub payload: Vec<u8>,
    pub blobs: Vec<PackedBlob>,
}

pub struct PackedBlob {
    pub kind: StreamingAssetBlobKind,
    pub element_count: u64,
    pub data: Vec<u8>,
}

pub struct BuiltPayload {
    pub payload: Vec<u8>,
    pub blobs: Vec<PackedBlob>,
}
