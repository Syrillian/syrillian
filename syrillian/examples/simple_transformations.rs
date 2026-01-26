//! Basics of moving, rotating and scaling game objects.
//!
//! Also uses a simple Prefab as an example on - well, how to do that.

use nalgebra::UnitQuaternion;
use std::error::Error;
use syrillian::AppState;
use syrillian::core::GameObjectId;
use syrillian::prefabs::Prefab;
use syrillian::world::World;
use syrillian_macros::SyrillianApp;
use syrillian_scene::SceneLoader;

#[derive(Debug, Default, SyrillianApp)]
struct SimpleTransformations;

impl AppState for SimpleTransformations {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.new_camera();
        world.spawn(&NineCubes);

        world.print_objects();

        Ok(())
    }
}

struct NineCubes;

impl Prefab for NineCubes {
    fn prefab_name(&self) -> &'static str {
        "Nine Cubes"
    }

    fn build(&self, world: &mut World) -> GameObjectId {
        let Ok(mut scene) = SceneLoader::load(world, "testmodels/simple_trans.fbx") else {
            panic!(
                "Failed to load the city file. Please run this example from the project root directory."
            );
        };

        scene.transform.set_position(0.0, 0.0, -10.0);
        scene
            .transform
            .set_rotation(UnitQuaternion::from_euler_angles(0.0, 90.0, 0.0));
        scene.transform.set_scale(0.01);

        world.add_child(scene);

        scene
    }
}
