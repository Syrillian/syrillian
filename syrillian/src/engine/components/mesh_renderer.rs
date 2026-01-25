use crate::assets::HMaterial;
use crate::components::{Component, SkeletalComponent};
use crate::core::{Bone, Vertex3D};
use crate::engine::assets::HMesh;
use crate::engine::rendering::CPUDrawCtx;
use crate::rendering::proxies::SceneProxy;
use crate::rendering::proxies::mesh_proxy::MeshSceneProxy;
use crate::{MAX_BONES, Reflect, World, proxy_data_mut};
use nalgebra::{Matrix4, Vector3};
use tracing::warn;

#[derive(Debug, Default, Clone)]
pub struct BoneData {
    pub(crate) bones: Vec<Bone>,
}

impl BoneData {
    #[rustfmt::skip]
    pub const DUMMY: [Bone; MAX_BONES] = [Bone {
        transform: Matrix4::new(
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        )
    }; MAX_BONES];

    pub fn new_full_identity() -> Self {
        Self {
            bones: vec![
                Bone {
                    transform: Matrix4::identity()
                };
                MAX_BONES
            ],
        }
    }

    pub fn set_first_n(&mut self, mats: &[Matrix4<f32>]) {
        for (i, m) in mats.iter().take(self.bones.len()).enumerate() {
            self.bones[i].transform = *m;
        }
    }

    pub fn count(&self) -> usize {
        self.bones.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.bones)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugVertexNormal {
    position: Vector3<f32>,
    normal: Vector3<f32>,
}

#[derive(Debug, Reflect)]
pub struct MeshRenderer {
    mesh: HMesh,
    materials: Vec<HMaterial>,
    dirty_mesh: bool,
    dirty_materials: bool,
}

impl Default for MeshRenderer {
    fn default() -> Self {
        // TODO: Null Asset Handles
        MeshRenderer {
            mesh: HMesh::UNIT_CUBE,
            materials: vec![],
            dirty_mesh: false,
            dirty_materials: false,
        }
    }
}

impl Component for MeshRenderer {
    fn create_render_proxy(&mut self, world: &World) -> Option<Box<dyn SceneProxy>> {
        let Some(mesh) = world.assets.meshes.try_get(self.mesh) else {
            warn!(
                "Mesh Renderer couldn't create its proxy because the mesh wasn't found in the asset store"
            );
            return None;
        };

        Some(Box::new(MeshSceneProxy {
            mesh: self.mesh,
            materials: self.materials.clone(),
            material_ranges: mesh.material_ranges.clone(),
            bone_data: BoneData::new_full_identity(),
            bones_dirty: false,
            bounding: mesh.bounding_sphere,
        }))
    }

    fn update_proxy(&mut self, world: &World, mut ctx: CPUDrawCtx) {
        if let Some(mut skel) = self.parent().get_component::<SkeletalComponent>()
            && skel.update_palette()
        {
            let palette = skel.palette().to_vec();
            ctx.send_proxy_update(move |sc| {
                let data: &mut MeshSceneProxy = proxy_data_mut!(sc);

                // TODO: The copy is expensive, but it only happens if the skeleton actually got updated
                data.bone_data.set_first_n(&palette);
                data.bones_dirty = true;
            });
        }

        if !self.dirty_mesh && !self.dirty_materials {
            return;
        }

        let Some(mesh) = world.assets.meshes.try_get(self.mesh) else {
            warn!(
                "Mesh Renderer couldn't update its proxy because the mesh wasn't found in the asset store"
            );
            return;
        };

        if self.dirty_mesh {
            let h_mesh = self.mesh;
            let bounds = mesh.bounding_sphere;
            ctx.send_proxy_update(move |sc| {
                let data: &mut MeshSceneProxy = proxy_data_mut!(sc);
                data.mesh = h_mesh;
                data.bounding = bounds;
            })
        }

        if self.dirty_materials {
            let materials = self.materials.clone();
            let material_ranges = mesh.material_ranges.clone();
            ctx.send_proxy_update(move |sc| {
                let data: &mut MeshSceneProxy = proxy_data_mut!(sc);
                data.materials = materials;
                data.material_ranges = material_ranges;
            });
        }
    }
}

impl MeshRenderer {
    pub fn change_mesh(&mut self, mesh: HMesh, materials: Option<Vec<HMaterial>>) {
        let materials = materials.unwrap_or_default();
        self.set_mesh(mesh);
        self.set_materials(materials);
    }

    pub fn set_mesh(&mut self, mesh: HMesh) {
        self.mesh = mesh;
        self.dirty_mesh = true;
    }

    pub fn set_materials(&mut self, materials: Vec<HMaterial>) {
        self.materials = materials;
        self.dirty_materials = true;
    }

    pub fn set_material_slot(&mut self, idx: usize, material: HMaterial) {
        let size = idx + 1;
        if self.materials.len() < size {
            self.materials.resize(size, HMaterial::FALLBACK);
        }
        self.materials[idx] = material;
        self.dirty_materials = true;
    }

    pub fn mesh(&self) -> HMesh {
        self.mesh
    }
}

impl From<&Vertex3D> for DebugVertexNormal {
    fn from(value: &Vertex3D) -> Self {
        DebugVertexNormal {
            position: value.position,
            normal: value.normal,
        }
    }
}
