use crate::MeshRenderer;
use syrillian::World;
use syrillian::assets::{HMaterialInstance, HMesh};
use syrillian::core::GameObjectId;
use syrillian::prefabs::Prefab;

pub struct SpherePrefab {
    pub material: HMaterialInstance,
}

impl Default for SpherePrefab {
    fn default() -> Self {
        Self {
            material: HMaterialInstance::DEFAULT,
        }
    }
}

impl SpherePrefab {
    pub const fn new(material: HMaterialInstance) -> Self {
        Self { material }
    }
}

impl Prefab for SpherePrefab {
    #[inline]
    fn prefab_name(&self) -> &'static str {
        "Sphere"
    }

    fn build(&self, world: &mut World) -> GameObjectId {
        let mut sphere = world.new_object(self.prefab_name());
        sphere
            .add_component::<MeshRenderer>()
            .change_mesh(HMesh::SPHERE, Some(vec![self.material]));

        sphere
    }
}
