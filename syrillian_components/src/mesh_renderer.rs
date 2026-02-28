use syrillian::assets::{HMaterialInstance, HMesh};
use syrillian::components::Component;
use syrillian::math::Vec3;
use syrillian::tracing::warn;
use syrillian::{Reflect, World};
use syrillian_render::proxies::{MeshSceneProxy, SceneProxy};
use syrillian_render::proxy_data_mut;
use syrillian_render::rendering::CPUDrawCtx;

#[repr(C)]
#[derive(
    Debug,
    Copy,
    Clone,
    zerocopy::Immutable,
    zerocopy::IntoBytes,
    zerocopy::FromBytes,
    zerocopy::KnownLayout,
)]
pub struct DebugVertexNormal {
    position: Vec3,
    normal: Vec3,
}

#[derive(Debug, Reflect)]
pub struct MeshRenderer {
    mesh: HMesh,
    materials: Vec<HMaterialInstance>,
    dirty_mesh: bool,
    dirty_materials: bool,
}

impl Default for MeshRenderer {
    fn default() -> Self {
        MeshRenderer {
            mesh: HMesh::invalid(),
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
            bounding: mesh.bounding_sphere,
        }))
    }

    fn update_proxy(&mut self, world: &World, mut ctx: CPUDrawCtx) {
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
    pub fn change_mesh(&mut self, mesh: HMesh, materials: Option<Vec<HMaterialInstance>>) {
        let materials = materials.unwrap_or_default();
        self.set_mesh(mesh);
        self.set_materials(materials);
    }

    pub fn set_mesh(&mut self, mesh: HMesh) {
        self.mesh = mesh;
        self.dirty_mesh = true;
    }

    pub fn set_materials(&mut self, materials: Vec<HMaterialInstance>) {
        self.materials = materials;
        self.dirty_materials = true;
    }

    pub fn set_material_slot(&mut self, idx: usize, material: HMaterialInstance) {
        let size = idx + 1;
        if self.materials.len() < size {
            self.materials.resize(size, HMaterialInstance::FALLBACK);
        }
        self.materials[idx] = material;
        self.dirty_materials = true;
    }

    pub fn mesh(&self) -> HMesh {
        self.mesh
    }
}
