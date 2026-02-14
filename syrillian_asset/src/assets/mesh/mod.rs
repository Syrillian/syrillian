pub mod bone;
pub mod buffer;
pub mod builder;
pub mod vertex;

pub use bone::{Bone, Bones};
pub use builder::MeshBuilder;
pub use vertex::Vertex3D;

use crate::HMesh;
use crate::mesh::buffer::UNIT_SQUARE_VERT;
use crate::store::{H, HandleName, Store, StoreDefaults, StoreType};
use crate::store_add_checked;
use glamx::{Vec2, Vec3};
use itertools::izip;
use obj::{IndexTuple, ObjError};
use snafu::Snafu;
use std::fmt::Debug;
use std::ops::Range;
use std::sync::Arc;
use syrillian_utils::{BoundingBox, BoundingSphere};

const CUBE_OBJ: &[u8] = include_bytes!("preset_meshes/cube.obj");
const DEBUG_ARROW: &[u8] = include_bytes!("preset_meshes/debug_arrow.obj");
const SPHERE: &[u8] = include_bytes!("preset_meshes/small_sphere.obj");

#[derive(Debug, Snafu)]
pub enum MeshError {
    #[snafu(display("The loaded mesh did not have any normals"))]
    NormalsMissing,
    #[snafu(display("The loaded mesh did not have any uv coordinates"))]
    UVMissing,
    #[snafu(display("The loaded mesh was not previously triangulated"))]
    NonTriangulated,
    #[snafu(transparent)]
    Obj { source: ObjError },
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub data: Arc<MeshVertexData<Vertex3D>>,
    pub material_ranges: Vec<Range<u32>>,
    pub bones: Bones,
    pub bounding_sphere: BoundingSphere,
}

#[derive(Debug, Clone)]
pub struct MeshVertexData<T: Debug + Clone> {
    pub vertices: Vec<T>,
    pub indices: Option<Vec<u32>>,
}

impl Mesh {
    pub fn builder(vertices: Vec<Vertex3D>) -> MeshBuilder {
        MeshBuilder::new(vertices)
    }

    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.data.vertices.len()
    }

    #[inline]
    pub fn indices_count(&self) -> usize {
        self.indices().map_or(0, <[u32]>::len)
    }

    #[inline]
    pub fn triangle_count(&self) -> usize {
        if self.has_indices() {
            self.indices_count() / 3
        } else {
            self.vertex_count() / 3
        }
    }

    #[inline]
    pub fn vertices(&self) -> &[Vertex3D] {
        &self.data.vertices
    }

    #[inline]
    pub fn indices(&self) -> Option<&[u32]> {
        self.data.indices.as_deref()
    }

    #[inline]
    pub fn has_indices(&self) -> bool {
        self.data.indices.is_some()
    }

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
            .map(|(v, u, n)| Vertex3D::basic(v, u, n))
            .collect::<Vec<_>>();
        let bounding_sphere = bounding_sphere_from_vertices(&vertices);

        Ok(Mesh {
            data: Arc::new(MeshVertexData::new(vertices, None)),
            material_ranges,
            bones: Bones::none(),
            bounding_sphere,
        })
    }

    pub fn calculate_bounding_box(&self) -> BoundingBox {
        if self.vertices().is_empty() {
            return BoundingBox::default();
        }

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for v in self.vertices() {
            let p = v.position;
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            min.z = min.z.min(p.z);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            max.z = max.z.max(p.z);
        }

        BoundingBox { min, max }
    }
}

impl MeshVertexData<Vertex3D> {
    pub fn new(vertices: Vec<Vertex3D>, indices: Option<Vec<u32>>) -> Self {
        MeshVertexData { vertices, indices }
    }

    pub fn make_triangle_indices(&self) -> Vec<[u32; 3]> {
        match &self.indices {
            None => (0u32..self.vertices.len() as u32)
                .collect::<Vec<_>>()
                .as_chunks()
                .0
                .to_vec(),
            Some(indices) => indices.as_chunks().0.to_vec(),
        }
    }

    pub fn make_point_cloud(&self) -> Vec<Vec3> {
        self.vertices.iter().map(|v| v.position).collect()
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
        let unit_square = Mesh::builder(UNIT_SQUARE_VERT.to_vec()).build();
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
    const NAME: &str = "Mesh";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HMesh::UNIT_SQUARE_ID => HandleName::Static("Unit Square"),
            HMesh::UNIT_CUBE_ID => HandleName::Static("Unit Cube"),
            HMesh::DEBUG_ARROW_ID => HandleName::Static("Debug Arrow"),
            HMesh::SPHERE_ID => HandleName::Static("Sphere"),
            _ => HandleName::Id(handle),
        }
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<Self>::MAX_BUILTIN_ID
    }
}

pub fn bounding_sphere_from_vertices(vertices: &[Vertex3D]) -> BoundingSphere {
    if vertices.is_empty() {
        return BoundingSphere::default();
    }

    let mut min = vertices[0].position;
    let mut max = vertices[0].position;

    for v in vertices.iter().skip(1) {
        let p = v.position;
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }

    let center = (min + max) * 0.5;
    let mut radius = 0.0f32;
    for v in vertices {
        radius = radius.max((v.position - center).length());
    }

    BoundingSphere { center, radius }
}
