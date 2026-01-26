use crate::MeshRenderer;
use syrillian::World;
use syrillian::assets::{HMaterial, HMesh};
use syrillian::core::GameObjectId;
use syrillian::prefabs::Prefab;

pub struct CubePrefab {
    pub material: HMaterial,
}

impl Default for CubePrefab {
    fn default() -> Self {
        CubePrefab {
            material: HMaterial::DEFAULT,
        }
    }
}

impl CubePrefab {
    pub const fn new(material: HMaterial) -> Self {
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
