use crate::SunLightComponent;
use syrillian::World;
use syrillian::core::GameObjectId;
use syrillian::prefabs::Prefab;

pub struct SunPrefab;

impl Prefab for SunPrefab {
    fn prefab_name(&self) -> &'static str {
        "Sun"
    }

    fn build(&self, world: &mut World) -> GameObjectId {
        let mut obj = world.new_object(self.prefab_name());
        obj.transform.set_position(-20, 20, -20);
        obj.transform.set_euler_rotation_deg(45, 0, 45);

        obj.add_component::<SunLightComponent>();

        obj
    }
}
