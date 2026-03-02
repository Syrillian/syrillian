use crate::mesh::static_mesh_data::{RawSkinningVertexBuffers, RawVertexBuffers};
use crate::mesh::{Bones, PartialMesh};
use crate::store::streaming::asset_store::{
    AssetType, StreamingAssetBlobKind, StreamingAssetFile, StreamingAssetPayload,
};
use crate::store::streaming::decode_helper::{
    DecodeHelper, MapDecodeHelper, ParseDecode, ParseDecodeWithBlobs,
};
use crate::store::streaming::packaged_scene::{BuiltPayload, PackedBlob};
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{
    AssetKey, AssetRefreshMessage, H, HandleName, StoreType, UpdateAssetMessage, streaming,
};
use crossbeam_channel::Sender;
use std::ops::Range;
use std::sync::Arc;
use syrillian_reflect::serializer::JsonSerializer;
use syrillian_utils::BoundingSphere;

#[derive(Debug, Clone, bon::Builder)]
pub struct SkinnedMesh {
    pub data: Arc<RawVertexBuffers>,
    pub skinning_data: Arc<RawSkinningVertexBuffers>,
    #[builder(default)]
    pub material_ranges: Vec<Range<u32>>,
    pub bones: Bones,
    pub bounding_sphere: Option<BoundingSphere>,
}

impl PartialMesh for SkinnedMesh {
    fn buffers(&self) -> &RawVertexBuffers {
        &self.data
    }
}

impl StoreType for SkinnedMesh {
    const NAME: &str = "Skinned Static Mesh";
    const TYPE: AssetType = AssetType::SkinnedMesh;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(&self, key: AssetKey, assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        if !self.data.is_valid() {
            return false;
        }

        assets_tx
            .send(AssetRefreshMessage::updated(
                key,
                UpdateAssetMessage::UpdateSkinnedMesh(self.clone()),
            ))
            .is_ok()
    }

    fn is_builtin(_: H<Self>) -> bool {
        false
    }
}

impl StreamableAsset for SkinnedMesh {
    fn encode(&self) -> BuiltPayload {
        let mut blobs = Vec::new();

        if let Some(positions) =
            PackedBlob::pack_data(StreamingAssetBlobKind::MeshPositions, &self.data.positions)
        {
            blobs.push(positions);
        }

        if let Some(uvs) = PackedBlob::pack_data(StreamingAssetBlobKind::MeshUVs, &self.data.uvs) {
            blobs.push(uvs);
        }

        if let Some(normals) =
            PackedBlob::pack_data(StreamingAssetBlobKind::MeshNormals, &self.data.normals)
        {
            blobs.push(normals);
        }

        if let Some(tangents) =
            PackedBlob::pack_data(StreamingAssetBlobKind::MeshTangents, &self.data.tangents)
        {
            blobs.push(tangents);
        }

        if let Some(bone_indices) = PackedBlob::pack_data(
            StreamingAssetBlobKind::MeshBoneIndices,
            &self.skinning_data.bone_indices,
        ) {
            blobs.push(bone_indices);
        }

        if let Some(bone_weights) = PackedBlob::pack_data(
            StreamingAssetBlobKind::MeshBoneWeights,
            &self.skinning_data.bone_weights,
        ) {
            blobs.push(bone_weights);
        }

        if let Some(raw_indices) = &self.data.indices
            && let Some(indices) =
                PackedBlob::pack_data(StreamingAssetBlobKind::MeshIndices, raw_indices)
        {
            blobs.push(indices);
        }

        if let Some(inverse_binds) = PackedBlob::pack_data(
            StreamingAssetBlobKind::BonesInverseBind,
            &self.bones.inverse_bind,
        ) {
            blobs.push(inverse_binds);
        }

        if let Some(bind_globals) = PackedBlob::pack_data(
            StreamingAssetBlobKind::BonesBindGlobal,
            &self.bones.bind_global,
        ) {
            blobs.push(bind_globals);
        }

        if let Some(bind_locals) = PackedBlob::pack_data(
            StreamingAssetBlobKind::BonesBindLocal,
            &self.bones.bind_local,
        ) {
            blobs.push(bind_locals);
        }

        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(self),
            blobs,
        }
    }

    fn decode(
        payload: &StreamingAssetPayload,
        package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<Self> {
        let root = payload.data.expect_object("mesh data")?;

        let material_ranges = root
            .required_field("material_ranges")?
            .expect_parse("mesh material ranges")?;

        let bones = root
            .required_field("bones")?
            .expect_parse_blobs(&payload.blob_infos, package)?;

        let bounding_sphere = root
            .optional_field("bounding_sphere")
            .expect_parse("mesh bounding sphere")?;

        let indices = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshIndices)
            .ok()
            .map(|i| i.decode_all_from_io(package))
            .transpose()?;

        let positions = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshPositions)?
            .decode_all_from_io(package)?;
        let uvs = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshUVs)?
            .decode_all_from_io(package)?;
        let normals = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshNormals)?
            .decode_all_from_io(package)?;
        let tangents = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshTangents)?
            .decode_all_from_io(package)?;
        let bone_indices = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshBoneIndices)?
            .decode_all_from_io(package)?;
        let bone_weights = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshBoneWeights)?
            .decode_all_from_io(package)?;

        let buffers = RawVertexBuffers {
            positions,
            uvs,
            normals,
            tangents,
            indices,
        };

        let skinning_buffers = RawSkinningVertexBuffers {
            bone_indices,
            bone_weights,
        };

        debug_assert!(buffers.is_valid());

        Ok(SkinnedMesh {
            data: Arc::new(buffers),
            skinning_data: Arc::new(skinning_buffers),
            material_ranges,
            bones,
            bounding_sphere,
        })
    }
}
