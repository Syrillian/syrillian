use crate::store::streaming::asset_store::{AssetType, StreamingAssetBlobKind};
use crate::{AnimationClip, Mesh, PrefabAsset, PrefabMaterial, SkinnedMesh, Texture2D};

#[derive(Debug, Clone)]
pub struct PackagedScene {
    pub virtual_root: String,
    pub meshes: Vec<PackagedMeshAsset>,
    pub skinned_meshes: Vec<PackagedSkinnedMeshAsset>,
    pub textures: Vec<PackagedTextureAsset>,
    pub materials: Vec<PackagedMaterialAsset>,
    pub animations: Vec<PackagedAnimationAsset>,
    pub prefab: PackagedPrefabAsset,
}

#[derive(Debug, Clone)]
pub struct PackagedAsset<T> {
    pub virtual_path: String,
    pub asset: T,
}

pub type PackagedMeshAsset = PackagedAsset<Mesh>;
pub type PackagedSkinnedMeshAsset = PackagedAsset<SkinnedMesh>;
pub type PackagedTextureAsset = PackagedAsset<Texture2D>;
pub type PackagedMaterialAsset = PackagedAsset<PrefabMaterial>;
pub type PackagedAnimationAsset = PackagedAsset<AnimationClip>;
pub type PackagedPrefabAsset = PackagedAsset<PrefabAsset>;

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
    pub payload: String,
    pub blobs: Vec<PackedBlob>,
}
