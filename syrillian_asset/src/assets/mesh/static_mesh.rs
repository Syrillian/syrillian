use crate::mesh::buffer::UNIT_SQUARE_VERT;
use crate::mesh::static_mesh_data::{RawVertexBuffers, VertexBufferExt};
use crate::mesh::{CUBE_OBJ, DEBUG_ARROW, MeshError, PartialMesh, SPHERE};
use crate::store::streaming::asset_store::{
    AssetType, StreamingAssetBlobKind, StreamingAssetFile, StreamingAssetPayload,
};
use crate::store::streaming::decode_helper::{DecodeHelper, MapDecodeHelper, ParseDecode};
use crate::store::streaming::packaged_scene::{BuiltPayload, PackedBlob};
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{
    AssetKey, AssetRefreshMessage, H, HandleName, Store, StoreDefaults, StoreType,
    UpdateAssetMessage, streaming,
};
use crate::{HMesh, store_add_checked};
use crossbeam_channel::Sender;
use obj::IndexTuple;
use std::ops::Range;
use std::sync::Arc;
use syrillian_reflect::serializer::JsonSerializer;
use syrillian_utils::BoundingSphere;

#[derive(Debug, Clone, bon::Builder)]
pub struct Mesh {
    pub data: Arc<RawVertexBuffers>,
    #[builder(default)]
    pub material_ranges: Vec<Range<u32>>,
    pub bounding_sphere: Option<BoundingSphere>,
}

impl Mesh {
    pub fn load_from_obj_slice(data: &[u8]) -> Result<Mesh, MeshError> {
        let data = obj::ObjData::load_buf(data)?;
        let mut buffers = RawVertexBuffers::default();

        let mut material_ranges = Vec::new();

        for obj in data.objects {
            for group in obj.groups {
                let mat_start = buffers.len() as u32;

                for poly in group.polys {
                    if poly.0.len() != 3 {
                        return Err(MeshError::NonTriangulated);
                    }
                    for IndexTuple(pos, uv, normal) in poly.0 {
                        let Some(uv) = uv else {
                            return Err(MeshError::UVMissing);
                        };
                        let Some(normal) = normal else {
                            return Err(MeshError::NormalsMissing);
                        };
                        buffers.positions.push(data.position[pos].into());
                        buffers.uvs.push(data.texture[uv].into());
                        buffers.normals.push(data.normal[normal].into());
                    }
                }

                let mat_end = (mat_start as usize + buffers.positions.len()) as u32;
                material_ranges.push(mat_start..mat_end);
            }
        }

        buffers.interpolate();

        debug_assert!(buffers.is_valid());

        let bounding_sphere = Some(BoundingSphere::from_positions(
            buffers.positions.iter().copied(),
        ));

        Ok(Mesh {
            data: Arc::new(buffers),
            material_ranges,
            bounding_sphere,
        })
    }
}

impl PartialMesh for Mesh {
    fn buffers(&self) -> &RawVertexBuffers {
        &self.data
    }
}

impl H<Mesh> {
    const UNIT_SQUARE_ID: u32 = 0;
    const UNIT_CUBE_ID: u32 = 1;
    const DEBUG_ARROW_ID: u32 = 2;
    const SPHERE_ID: u32 = 3;
    const MAX_BUILTIN_ID: u32 = 3;

    pub const UNIT_SQUARE: HMesh = H::new(Self::UNIT_SQUARE_ID);
    pub const UNIT_CUBE: HMesh = H::new(Self::UNIT_CUBE_ID);
    pub const DEBUG_ARROW: HMesh = H::new(Self::DEBUG_ARROW_ID);
    pub const SPHERE: HMesh = H::new(Self::SPHERE_ID);
}

impl StoreDefaults for Mesh {
    fn populate(store: &mut Store<Self>) {
        let unit_square = Mesh::builder()
            .data(Arc::new(UNIT_SQUARE_VERT.as_ref().into()))
            .build();
        store_add_checked!(store, HMesh::UNIT_SQUARE_ID, unit_square);

        let unit_cube = Mesh::load_from_obj_slice(CUBE_OBJ).expect("Cube Mesh load failed");
        store_add_checked!(store, HMesh::UNIT_CUBE_ID, unit_cube);

        let debug_arrow =
            Mesh::load_from_obj_slice(DEBUG_ARROW).expect("Debug Arrow Mesh load failed");
        store_add_checked!(store, HMesh::DEBUG_ARROW_ID, debug_arrow);

        let sphere = Mesh::load_from_obj_slice(SPHERE).expect("Sphere Mesh load failed");
        store_add_checked!(store, HMesh::SPHERE_ID, sphere);
    }
}

impl StoreType for Mesh {
    const NAME: &str = "Static Mesh";
    const TYPE: AssetType = AssetType::Mesh;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HMesh::UNIT_SQUARE_ID => HandleName::Static("Unit Square"),
            HMesh::UNIT_CUBE_ID => HandleName::Static("Unit Cube"),
            HMesh::DEBUG_ARROW_ID => HandleName::Static("Debug Arrow"),
            HMesh::SPHERE_ID => HandleName::Static("Sphere"),
            _ => HandleName::Id(handle),
        }
    }

    fn refresh_dirty(&self, key: AssetKey, assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        if !self.data.is_valid() {
            return false;
        }

        assets_tx
            .send(AssetRefreshMessage::Updated(
                key,
                UpdateAssetMessage::UpdateMesh(self.clone()),
            ))
            .is_ok()
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<Self>::MAX_BUILTIN_ID
    }
}

impl StreamableAsset for Mesh {
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

        if let Some(raw_indices) = &self.data.indices
            && let Some(indices) =
                PackedBlob::pack_data(StreamingAssetBlobKind::MeshIndices, raw_indices)
        {
            blobs.push(indices);
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

        let buffers = RawVertexBuffers {
            positions,
            uvs,
            normals,
            tangents,
            indices,
        };

        let bounding_sphere = bounding_sphere.or_else(|| {
            Some(BoundingSphere::from_positions(
                buffers.positions.iter().copied(),
            ))
        });

        debug_assert!(buffers.is_valid());

        Ok(Mesh {
            data: Arc::new(buffers),
            material_ranges,
            bounding_sphere,
        })
    }
}
