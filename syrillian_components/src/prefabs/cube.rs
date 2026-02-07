use crate::MeshRenderer;
use syrillian::World;
use syrillian::assets::{HMaterialInstance, HMesh};
use syrillian::core::GameObjectId;
use syrillian::prefabs::Prefab;

pub struct CubePrefab {
    pub material: HMaterialInstance,
}

impl Default for CubePrefab {
    fn default() -> Self {
        CubePrefab {
            material: HMaterialInstance::DEFAULT,
        }
    }
}

impl CubePrefab {
    pub const fn new(material: HMaterialInstance) -> Self {
        CubePrefab { material }
    }
}

impl Prefab for CubePrefab {
    #[inline]
    fn prefab_name(&self) -> &'static str {
        "Cube"
    }

    fn build(&self, world: &mut World) -> GameObjectId {
        let mut cube = world.new_object("Cube");
        cube.add_component::<MeshRenderer>()
            .change_mesh(HMesh::UNIT_CUBE, Some(vec![self.material]));

        cube
    }
}
