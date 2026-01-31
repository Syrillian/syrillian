//! Demonstrates parenting behavior, as well as testing/using the scene loader.

use std::error::Error;

use syrillian::SyrillianApp;
use syrillian::utils::frame_counter::FrameCounter;
use syrillian::world::World;
use syrillian::{AppState, ENGINE_STR};
use syrillian_components::RotateComponent;
use syrillian_scene::SceneLoader;

#[derive(Debug, Default, SyrillianApp)]
struct ParentingAndObjectTypes {
    frame_counter: FrameCounter,
}

impl AppState for ParentingAndObjectTypes {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        let mut obj2 = SceneLoader::load(world, "testmodels/parenting_and_object_types.fbx")?;
        let mut obj1 = world.new_object("Mow");
        let camera = world.new_camera();

        camera.parent().transform.set_position(0.0, 1.0, 50.0);

        obj2.transform.set_scale(0.03);
        obj2.add_component::<RotateComponent>();
        obj1.add_child(obj2);
        world.add_child(obj1);

        world.print_objects();

        Ok(())
    }

    fn update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        self.frame_counter.new_frame_from_world(world);

        let title = format!(
            "{} - FPS: [ {} ]",
            ENGINE_STR,
            self.frame_counter.fps_mean()
        );
        world.set_default_window_title(title);

        Ok(())
    }
}
