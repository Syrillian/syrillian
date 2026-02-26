use crate::mesh::buffer::UNIT_SQUARE_VERT;
use crate::mesh::static_mesh_data::StaticMeshData;
use crate::mesh::{
    CUBE_OBJ, DEBUG_ARROW, MeshError, PartialMesh, SPHERE, SkinnedVertex3D, UnskinnedVertex3D,
};
use crate::store::streaming::asset_store::{
    StreamingAssetBlobKind, StreamingAssetFile, StreamingAssetPayload,
};
use crate::store::streaming::decode_helper::{DecodeHelper, MapDecodeHelper, ParseDecode};
use crate::store::streaming::packaged_scene::{BuiltPayload, PackedBlob};
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{H, HandleName, Store, StoreDefaults, StoreType, UpdateAssetMessage, streaming};
use crate::{HMesh, store_add_checked};
use crossbeam_channel::Sender;
use glamx::{Vec2, Vec3};
use itertools::izip;
use obj::IndexTuple;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use syrillian_reflect::serializer::JsonSerializer;
use syrillian_reflect::{ReflectSerialize, Value};
use syrillian_utils::BoundingSphere;
use zerocopy::IntoBytes;

#[derive(Debug, Clone, bon::Builder)]
pub struct Mesh {
    #[builder(
        with = |vertices: Vec<UnskinnedVertex3D>, indices: Option<Vec<u32>>|
            Arc::new(StaticMeshData::new(vertices, indices))
    )]
    pub data: Arc<StaticMeshData>,
    #[builder(default)]
    pub material_ranges: Vec<Range<u32>>,
    pub bounding_sphere: Option<BoundingSphere>,
}

impl Mesh {
    pub fn load_from_obj_slice(data: &[u8]) -> Result<Mesh, MeshError> {
        let data = obj::ObjData::load_buf(data)?;
        let mut vertices: Vec<Vec3> = Vec::new();
        let mut normals: Vec<Vec3> = Vec::new();
        let mut uvs: Vec<Vec2> = Vec::new();

        let mut material_ranges = Vec::new();

        for obj in data.objects {
            for group in obj.groups {
                let mat_start = vertices.len() as u32;

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
                        vertices.push(data.position[pos].into());
                        uvs.push(data.texture[uv].into());
                        normals.push(data.normal[normal].into());
                    }
                }

                let mat_end = (mat_start as usize + vertices.len()) as u32;
                material_ranges.push(mat_start..mat_end);
            }
        }

        debug_assert!(vertices.len() == uvs.len() && vertices.len() == normals.len());

        let vertices = izip!(vertices, uvs, normals)
            .map(|(v, u, n)| UnskinnedVertex3D::basic(v, u, n))
            .collect::<Vec<_>>();

        Ok(Mesh {
            data: Arc::new(StaticMeshData::new(vertices, None)),
            material_ranges,
            bounding_sphere: None,
        })
    }
}

impl PartialMesh for Mesh {
    type VertexType = UnskinnedVertex3D;
    fn vertices(&self) -> &[Self::VertexType] {
        &self.data.vertices
    }

    fn indices(&self) -> Option<&[u32]> {
        self.data.indices.as_deref()
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
            .data(UNIT_SQUARE_VERT.to_vec(), None)
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

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HMesh::UNIT_SQUARE_ID => HandleName::Static("Unit Square"),
            HMesh::UNIT_CUBE_ID => HandleName::Static("Unit Cube"),
            HMesh::DEBUG_ARROW_ID => HandleName::Static("Debug Arrow"),
            HMesh::SPHERE_ID => HandleName::Static("Sphere"),
            _ => HandleName::Id(handle),
        }
    }

    fn refresh_dirty(
        &self,
        key: crate::store::AssetKey,
        assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        assets_tx
            .send((key, UpdateAssetMessage::UpdateMesh(self.clone())))
            .is_ok()
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<Self>::MAX_BUILTIN_ID
    }
}

struct MeshMeta<'a>(&'a Mesh);

impl ReflectSerialize for MeshMeta<'_> {
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
impl StreamableAsset for Mesh {
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
        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(&MeshMeta(self)),
            blobs,
        }
    }

    fn decode(
        payload: &StreamingAssetPayload,
        package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<Self> {
        let root = payload.data.expect_object("mesh metadata")?;

        let vertex_count = root
            .required_field("vertex_count")?
            .expect_usize("mesh vertex count")?;
        let index_count = root
            .required_field("index_count")?
            .expect_usize("mesh index count")?;

        let material_ranges = root
            .required_field("material_ranges")?
            .expect_parse("material ranges")?;

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
            vertex_blob.decode_from_io("unskinned mesh vertices", vertex_count, package)?;

        Ok(Mesh {
            data: Arc::new(StaticMeshData::new(vertices, indices)),
            material_ranges,
            bounding_sphere,
        })
    }
}
