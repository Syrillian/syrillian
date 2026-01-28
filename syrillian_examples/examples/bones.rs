//! Skeletal Mesh and Animation experimentation example.
//!
//! The goal of this is to test if bones are working as expected and to
//! aid in the development in the first place.

use std::error::Error;
use syrillian::math::{Quat, Vec3};
use syrillian::{AppState, SyrillianApp, World};
use syrillian_components::SkeletalComponent;
use syrillian_scene::SceneLoader;

use syrillian::components::Component;
#[cfg(debug_assertions)]
use syrillian::input::KeyCode;

#[derive(Debug, Default, SyrillianApp)]
struct BonesExample;

impl AppState for BonesExample {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.new_camera();

        let mut boney_obj = SceneLoader::load(world, "./testmodels/hampter/hampter.glb")?;
        boney_obj.name = "Boney thing".to_owned();

        boney_obj.transform.set_position(0.0, -5.0, -20.0);

        world
            .find_object_by_name("Cube")
            .unwrap()
            .add_component::<BoneChainWave>();

        world.add_child(boney_obj);

        world.print_objects();

        Ok(())
    }

    #[cfg(debug_assertions)]
    fn update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        use syrillian::rendering::DebugRenderer;
        if world.input.is_key_down(KeyCode::KeyL) {
            DebugRenderer::next_mode();
        }
        // renderer.debug.off();

        Ok(())
    }
}

pub struct BoneChainWave {
    t: f32,
}

impl Default for BoneChainWave {
    fn default() -> Self {
        Self { t: 0.0 }
    }
}

impl Component for BoneChainWave {
    fn update(&mut self, world: &mut World) {
        self.t += world.delta_time().as_secs_f32() * 2.0;
        if let Some(mut skel) = self.parent().get_component::<SkeletalComponent>() {
            let n = skel.bone_count();
            for i in 0..n {
                let phase = self.t + i as f32 * 0.35;
                let angle = (phase).sin() * 20.0_f32.to_radians();
                skel.set_local_rotation(i, Quat::from_axis_angle(Vec3::Z, angle));
            }
        }
    }
}
