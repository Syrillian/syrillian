use crate::MeshRenderer;
use syrillian::World;
use syrillian::assets::{HMaterial, HMesh};
use syrillian::core::GameObjectId;
use syrillian::prefabs::Prefab;

pub struct SpherePrefab;

impl Prefab for SpherePrefab {
    #[inline]
    fn prefab_name(&self) -> &'static str {
        "Sphere"
    }

    fn build(&self, world: &mut World) -> GameObjectId {
        let mut sphere = world.new_object(self.prefab_name());
        sphere
            .add_component::<MeshRenderer>()
            .change_mesh(HMesh::SPHERE, Some(vec![HMaterial::DEFAULT]));

        sphere
    }
}
