use crate::MeshRenderer;
use syrillian::World;
use syrillian::assets::{HMaterial, HMesh};
use syrillian::core::GameObjectId;
use syrillian::prefabs::Prefab;

pub struct CubePrefab;

impl Prefab for CubePrefab {
    #[inline]
    fn prefab_name(&self) -> &'static str {
        "Cube"
    }

    fn build(&self, world: &mut World) -> GameObjectId {
        let mut cube = world.new_object("Cube");
        cube.add_component::<MeshRenderer>()
            .change_mesh(HMesh::UNIT_CUBE, Some(vec![HMaterial::DEFAULT]));

        cube
    }
}
