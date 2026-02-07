use itertools::iproduct;
use std::error::Error;
use syrillian::SyrillianApp;
use syrillian::assets::{MaterialInstance, StoreType};
use syrillian::math::Vec3;
use syrillian::{AppState, World};
use syrillian_components::prefabs::SunPrefab;
use syrillian_components::{FreecamController, MeshRenderer};

#[derive(Debug, Default, SyrillianApp)]
pub struct PBR {}

impl AppState for PBR {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        let mut camera = world.new_camera().parent();
        camera.add_component::<FreecamController>();
        camera.transform.set_position(0.0, 2.0, 30.0);

        for (x, y, z) in iproduct!(-5..=5, -5..=5, -5..=5) {
            let xs = (x + 5) as f32 / 10.0;
            let ys = (y + 5) as f32 / 10.0;
            let zs = (z + 5) as f32 / 10.0;

            let color = Vec3::new(xs, ys, zs);
            let material = MaterialInstance::builder()
                .roughness(xs)
                .metallic(ys)
                .name("Material")
                .diffuse(color)
                .build()
                .store(world);

            let mut sphere = world.new_object("Sphere");
            let mut mesh_renderer = sphere.add_component::<MeshRenderer>();
            mesh_renderer.set_material_slot(0, material);

            sphere.transform.set_position(x * 3, y * 3, z * 3);

            world.add_child(sphere);
        }

        world.spawn(&SunPrefab);

        #[cfg(debug_assertions)]
        syrillian::rendering::DebugRenderer::off();

        Ok(())
    }
}
