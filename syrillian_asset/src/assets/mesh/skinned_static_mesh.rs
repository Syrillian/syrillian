use crate::mesh::static_mesh_data::{SkinnedStaticMeshData, StaticMeshData};
use crate::mesh::{Bones, PartialMesh, SkinnedVertex3D};
use crate::store::streaming::asset_store::{
    StreamingAssetBlobKind, StreamingAssetFile, StreamingAssetPayload,
};
use crate::store::streaming::decode_helper::{
    DecodeHelper, MapDecodeHelper, ParseDecode, ParseDecodeWithBlobs,
};
use crate::store::streaming::packaged_scene::{BuiltPayload, PackedBlob};
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{H, HandleName, StoreType, UpdateAssetMessage, streaming};
use crossbeam_channel::Sender;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use syrillian_reflect::serializer::JsonSerializer;
use syrillian_reflect::{ReflectSerialize, Value};
use syrillian_utils::BoundingSphere;
use zerocopy::IntoBytes;

#[derive(Debug, Clone, bon::Builder)]
pub struct SkinnedMesh {
    #[builder(with = |vertices: Vec<SkinnedVertex3D>, indices: Option<Vec<u32>>| Arc::new(SkinnedStaticMeshData::new(vertices, indices))
    )]
    pub data: Arc<SkinnedStaticMeshData>,
    #[builder(default)]
    pub material_ranges: Vec<Range<u32>>,
    pub bones: Bones,
    pub bounding_sphere: Option<BoundingSphere>,
}

impl PartialMesh for SkinnedMesh {
    type VertexType = SkinnedVertex3D;
    fn vertices(&self) -> &[Self::VertexType] {
        &self.data.vertices
    }

    fn indices(&self) -> Option<&[u32]> {
        self.data.indices.as_deref()
    }
}

impl StoreType for SkinnedMesh {
    const NAME: &str = "Skinned Static Mesh";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(
        &self,
        key: crate::store::AssetKey,
        assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        assets_tx
            .send((key, UpdateAssetMessage::UpdateSkinnedMesh(self.clone())))
            .is_ok()
    }

    fn is_builtin(_: H<Self>) -> bool {
        false
    }
}

struct SkinnedMeshMeta<'a>(&'a SkinnedMesh);

struct MeshBonesMeta<'a>(&'a Bones);

impl ReflectSerialize for MeshBonesMeta<'_> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "names".to_string(),
                ReflectSerialize::serialize(&this.0.names),
            ),
            (
                "parents".to_string(),
                ReflectSerialize::serialize(&this.0.parents),
            ),
            (
                "children".to_string(),
                ReflectSerialize::serialize(&this.0.children),
            ),
            (
                "roots".to_string(),
                ReflectSerialize::serialize(&this.0.roots),
            ),
            (
                "index_of".to_string(),
                ReflectSerialize::serialize(&this.0.index_of),
            ),
            (
                "inverse_bind_count".to_string(),
                Value::BigUInt(this.0.inverse_bind.len() as u64),
            ),
            (
                "bind_global_count".to_string(),
                Value::BigUInt(this.0.bind_global.len() as u64),
            ),
            (
                "bind_local_count".to_string(),
                Value::BigUInt(this.0.bind_local.len() as u64),
            ),
        ]))
    }
}

impl ReflectSerialize for SkinnedMeshMeta<'_> {
    fn serialize(this: &Self) -> Value {
        let indices = this.0.indices();

        Value::Object(BTreeMap::from([
            (
                "vertex_count".to_string(),
                Value::BigUInt(this.0.vertices().len() as u64),
            ),
            (
                "vertex_stride".to_string(),
                Value::UInt(size_of::<SkinnedVertex3D>() as u32),
            ),
            ("has_indices".to_string(), Value::Bool(this.0.has_indices())),
            (
                "index_count".to_string(),
                Value::BigUInt(indices.map_or(0, <[u32]>::len) as u64),
            ),
            (
                "index_element_size".to_string(),
                Value::UInt(size_of::<u32>() as u32),
            ),
            (
                "material_ranges".to_string(),
                ReflectSerialize::serialize(&this.0.material_ranges),
            ),
            (
                "bones".to_string(),
                ReflectSerialize::serialize(&MeshBonesMeta(&this.0.bones)),
            ),
            (
                "bounding_sphere".to_string(),
                match this.0.bounding_sphere {
                    None => Value::None,
                    Some(ref b) => Value::Object(BTreeMap::from([
                        ("center".to_string(), ReflectSerialize::serialize(&b.center)),
                        ("radius".to_string(), Value::Float(b.radius)),
                    ])),
                },
            ),
        ]))
    }
}

impl StreamableAsset for SkinnedMesh {
    fn encode(&self) -> BuiltPayload {
        let mut blobs = Vec::new();

        if !self.vertices().is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::MeshVertices,
                element_count: self.vertices().len() as u64,
                data: self.vertices().as_bytes().to_vec(),
            });
        }

        if let Some(indices) = self.indices() {
            if !indices.is_empty() {
                blobs.push(PackedBlob {
                    kind: StreamingAssetBlobKind::MeshIndices,
                    element_count: indices.len() as u64,
                    data: indices.as_bytes().to_vec(),
                });
            }
        }

        let inverse_bind_blob = self.bones.inverse_bind.as_bytes();
        if !inverse_bind_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::BonesInverseBind,
                element_count: self.bones.inverse_bind.len() as u64,
                data: inverse_bind_blob.to_vec(),
            });
        }

        let bind_global_blob = self.bones.bind_global.as_bytes();
        if !bind_global_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::BonesBindGlobal,
                element_count: self.bones.bind_global.len() as u64,
                data: bind_global_blob.to_vec(),
            });
        }

        let bind_local_blob = self.bones.bind_local.as_bytes();
        if !bind_local_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::BonesBindLocal,
                element_count: self.bones.bind_local.len() as u64,
                data: bind_local_blob.to_vec(),
            });
        }

        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(&SkinnedMeshMeta(self)),
            blobs,
        }
    }

    fn decode(
        payload: &StreamingAssetPayload,
        package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<Self> {
        let root = payload.data.expect_object("mesh data")?;

        let vertex_count = root
            .required_field("vertex_count")?
            .expect_usize("mesh vertex count")?;
        let index_count = root
            .required_field("index_count")?
            .expect_usize("mesh index count")?;

        let material_ranges = root
            .required_field("material_ranges")?
            .expect_parse("mesh material ranges")?;

        let bones = root
            .required_field("bones")?
            .expect_parse_blobs(&payload.blob_infos, package)?;

        let bounding_sphere = root
            .optional_field("bounding_sphere")
            .expect_parse("mesh bounding sphere")?;

        let indices = if index_count != 0 {
            let index_blob = payload
                .blob_infos
                .find(StreamingAssetBlobKind::MeshIndices)?;
            Some(index_blob.decode_from_io("mesh indices", index_count, package)?)
        } else {
            None
        };

        let vertex_blob = payload
            .blob_infos
            .find(StreamingAssetBlobKind::MeshVertices)?;

        let vertices =
            vertex_blob.decode_from_io("skinned mesh vertices", vertex_count, package)?;
        Ok(SkinnedMesh {
            data: Arc::new(StaticMeshData::new(vertices, indices)),
            material_ranges,
            bones,
            bounding_sphere,
        })
    }
}
