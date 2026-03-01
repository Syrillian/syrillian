pub mod bone;
pub mod buffer;
pub mod generic_vertex;
pub mod simple_vertex;
pub mod skinned_static_mesh;
pub mod skinned_vertex;
pub mod static_mesh;
pub mod static_mesh_data;
pub mod unskinned_vertex;

pub use skinned_static_mesh::{SkinnedMesh, SkinnedMeshBuilder};
pub use static_mesh::{Mesh, MeshBuilder};

pub use bone::{Bone, Bones};
pub use simple_vertex::SimpleVertex3D;
pub use skinned_vertex::SkinnedVertex3D;
pub use unskinned_vertex::UnskinnedVertex3D;

use crate::mesh::static_mesh_data::{RawVertexBuffers, VertexBufferExt};
use glam::Vec3;
use obj::ObjError;
use snafu::Snafu;
use std::fmt::Debug;
use syrillian_utils::BoundingBox;

const CUBE_OBJ: &[u8] = include_bytes!("preset_meshes/cube.obj");
const DEBUG_ARROW: &[u8] = include_bytes!("preset_meshes/debug_arrow.obj");
const BOUNDS_GIZMO: &[u8] = include_bytes!("preset_meshes/bounds_gizmo.obj");
const SPHERE: &[u8] = include_bytes!("preset_meshes/small_sphere.obj");

#[derive(Debug, Snafu)]
pub enum MeshError {
    #[snafu(display("The loaded mesh did not have any normals"))]
    NormalsMissing,
    #[snafu(display("The loaded mesh did not have any uv coordinates"))]
    UVMissing,
    #[snafu(display("The loaded mesh was not previously triangulated"))]
    NonTriangulated,
    #[snafu(display("The loaded mesh did not contain any valid line segments"))]
    LinesMissing,
    #[snafu(transparent)]
    Obj { source: ObjError },
}

pub trait PartialMesh {
    fn buffers(&self) -> &RawVertexBuffers;

    fn positions(&self) -> &[Vec3] {
        self.buffers().positions()
    }

    fn indices(&self) -> Option<&[u32]> {
        self.buffers().indices()
    }

    #[inline]
    fn len(&self) -> usize {
        self.buffers().len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn triangle_count(&self) -> usize {
        self.len() / 3
    }

    #[inline]
    fn position_count(&self) -> usize {
        self.positions().len()
    }

    #[inline]
    fn indices_count(&self) -> usize {
        self.indices().map_or(0, <[u32]>::len)
    }

    #[inline]
    fn has_indices(&self) -> bool {
        self.indices().is_some()
    }

    fn calculate_bounding_box(&self) -> BoundingBox {
        BoundingBox::from_positions(self.positions().iter().copied())
    }
}
