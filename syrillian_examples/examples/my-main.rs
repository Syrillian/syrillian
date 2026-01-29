//! A showcase of various engine features.
//!
//! w: I use this as my main test environment, which allows me to expand this and experiment
//!    with new features. Therefore, it should contain the latest and greatest. I can recommend
//!    using this for reference.

use std::error::Error;
use syrillian::assets::{HMaterial, HSound, Sound, StoreType};
use syrillian::assets::{Material, Shader};
use syrillian::audio::effect::reverb::ReverbBuilder;
use syrillian::audio::track::SpatialTrackBuilder;
use syrillian::components::{CRef, CameraComponent};
use syrillian::core::reflection::ReflectSerialize;
use syrillian::core::reflection::serializer::JsonSerializer;
use syrillian::core::{GameObjectExt, GameObjectId, GameObjectRef};
use syrillian::input::Button;
use syrillian::input::{KeyCode, MouseButton};
use syrillian::math::{Pose, Quat, Vec3};
use syrillian::physics::Ray;
use syrillian::physics::rapier3d::prelude::{ColliderHandle, QueryFilter};
use syrillian::prefabs::Prefab;
#[cfg(debug_assertions)]
use syrillian::rendering::DebugRenderer;
use syrillian::rendering::lights::Light;
use syrillian::strobe::TextAlignment;
use syrillian::tracing::{error, info};
use syrillian::utils::FrameCounter;
use syrillian::{AppRuntime, AppState, World};
use syrillian_components::prefabs::{CubePrefab, FirstPersonPlayerPrefab};
use syrillian_components::{
    AudioEmitter, Collider3D, FirstPersonCameraController, FreecamController, PointLightComponent,
    RigidBodyComponent, RopeJoint, RotateComponent, SpotLightComponent, SpringJoint, Text3D,
};
use syrillian_scene::SceneLoader;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};
// const NECO_IMAGE: &[u8; 1293] = include_bytes!("assets/neco.jpg");

const SHADER1: &str = include_str!("dynamic_shader/shader.wgsl");
const SHADER2: &str = include_str!("dynamic_shader/shader2.wgsl");
const SHADER3: &str = include_str!("dynamic_shader/shader3.wgsl");

struct DynamicMaterials {
    primary: HMaterial,
    accent: HMaterial,
    glass: HMaterial,
}

#[derive(Debug)]
struct MyMain {
    frame_counter: FrameCounter,
    player: GameObjectRef,
    player_rb: CRef<RigidBodyComponent>,
    picked_up: Option<GameObjectRef>,
    text3d: GameObjectRef,
    light1: CRef<SpotLightComponent>,
    light2: CRef<SpotLightComponent>,
    pop_sound: Option<HSound>,
    sound_cube_emitter: CRef<AudioEmitter>,
    sound_cube2_emitter: CRef<AudioEmitter>,
    viewport_camera: Option<CRef<CameraComponent>>,
}

impl Default for MyMain {
    fn default() -> Self {
        unsafe {
            Self {
                frame_counter: FrameCounter::default(),
                player: GameObjectRef::null(),
                player_rb: CRef::null(),
                picked_up: None,
                text3d: GameObjectRef::null(),
                light1: CRef::null(),
                light2: CRef::null(),
                pop_sound: None,
                sound_cube_emitter: CRef::null(),
                sound_cube2_emitter: CRef::null(),
                viewport_camera: None,
            }
        }
    }
}

impl AppState for MyMain {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.spawn(&City);

        let materials = Self::build_dynamic_materials(world);
        Self::spawn_dynamic_cubes(world, &materials);

        self.setup_audio_demo(world, materials.primary)?;
        self.text3d = Self::spawn_3d_text(world);
        Self::spawn_spring_demo(world);
        // Self::cleanup_color_pads(world);

        let (light1, light2) = Self::spawn_spotlights(world);
        self.light1 = light1;
        self.light2 = light2;

        let (player, player_rb) = Self::spawn_player(world);
        self.player = player;
        self.player_rb = player_rb;

        let camera = self.spawn_viewport_camera(world);
        self.viewport_camera = Some(camera.clone());
        // world.set_active_camera_for_target(RenderTargetId::PRIMARY, camera);

        let serialized = ReflectSerialize::serialize(world);
        let serialized = JsonSerializer::value_to_string(&serialized);
        println!("{serialized}");

        Ok(())
    }

    fn update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        self.frame_counter.new_frame_from_world(world);
        world.set_default_window_title(self.format_title(false));

        self.update_world_text(world);
        self.update_audio_controls(world);
        self.spawn_on_demand_cubes(world);
        self.update_camera_zoom(world);
        self.handle_player_toggle(world);
        self.update_pickup_interaction(world);
        self.handle_debug_overlays(world);
        world.input.auto_quit_on_escape();

        Ok(())
    }
}

impl MyMain {
    fn spawn_player(world: &mut World) -> (GameObjectRef, CRef<RigidBodyComponent>) {
        let id = world.spawn(&FirstPersonPlayerPrefab);
        let mut player = world
            .get_object_ref(id)
            .expect("player prefab should return a valid object");
        let player_rb = player
            .get_component::<RigidBodyComponent>()
            .expect("player prefab should include a rigid body");
        player.at(0.0, 20.0, 0.0);
        (player, player_rb)
    }

    fn spawn_viewport_camera(&self, world: &mut World) -> CRef<CameraComponent> {
        let camera = world.new_camera();
        let mut camera_obj = camera.parent();
        camera_obj.transform.set_position(10.0, 15.0, 22.0);
        camera_obj
            .transform
            .set_euler_rotation_deg(-15.0, 200.0, 0.0);
        camera_obj.add_component::<FreecamController>();
        camera
    }

    fn build_dynamic_materials(world: &World) -> DynamicMaterials {
        let funky = Shader::new_fragment("Funky Shader", SHADER1).store(world);
        let funky_alt = Shader::new_fragment("Funky Shader 2", SHADER2).store(world);
        let funky_glass = Shader::new_fragment("Funky Shader 3", SHADER3).store(world);

        DynamicMaterials {
            primary: Material::builder()
                .name("Cube Material 1")
                .shader(funky)
                .store(world),
            accent: Material::builder()
                .name("Cube Material 2")
                .shader(funky_alt)
                .store(world),
            glass: Material::builder()
                .name("Cube Material 3")
                .shader(funky_glass)
                .store(world),
        }
    }

    fn spawn_dynamic_cubes(world: &mut World, mats: &DynamicMaterials) {
        let cube_primary = CubePrefab::new(mats.primary);
        let cube_accent = CubePrefab::new(mats.accent);
        let cube_glass = CubePrefab::new(mats.glass);

        let mut rotating_cube = world.spawn(&cube_accent);
        let mut floating_cube = world.spawn(&cube_accent);
        let mut rope_cube = world.spawn(&cube_accent);
        let mut big_cube_left = world.spawn(&cube_primary);
        let mut big_cube_right = world.spawn(&cube_glass);

        rotating_cube
            .at(20., 3.9, -20.)
            .build_component::<PointLightComponent>()
            .build_component::<Collider3D>()
            .mass(1.0)
            .restitution(0.9)
            .build_component::<RotateComponent>()
            .scaling(1.);

        floating_cube
            .at(5.0, 6.9, -20.0)
            .build_component::<PointLightComponent>()
            .build_component::<Collider3D>()
            .mass(1.0)
            .restitution(0.9)
            .build_component::<RigidBodyComponent>()
            .enable_ccd()
            .gravity_scale(0.0)
            .angular_damping(0.5)
            .linear_damping(0.5);

        rope_cube
            .at(5.0, 3.9, -20.0)
            .build_component::<PointLightComponent>()
            .build_component::<Collider3D>()
            .build_component::<RigidBodyComponent>()
            .enable_ccd()
            .build_component::<RopeJoint>()
            .connect_to(floating_cube);

        big_cube_left.at(100.0, 10.0, 200.0).scale(100.);
        big_cube_right.at(-100.0, 10.0, 200.0).scale(100.);
    }

    fn setup_audio_demo(
        &mut self,
        world: &mut World,
        material: HMaterial,
    ) -> Result<(), Box<dyn Error>> {
        let pop_sound_data = include_bytes!("../examples/assets/pop.wav");
        let mut pop_sound = Sound::load_sound_data(pop_sound_data.to_vec())?;
        pop_sound.set_start_position(0.2);
        let pop_sound = pop_sound.store(world);
        self.pop_sound = Some(pop_sound);

        let sound_cube_prefab = CubePrefab::new(material);

        let mut dry_cube = world.spawn(&sound_cube_prefab);
        dry_cube
            .at(10.0, 150.0, 10.0)
            .build_component::<Collider3D>()
            .build_component::<RigidBodyComponent>()
            .enable_ccd();
        self.sound_cube_emitter = dry_cube.add_component::<AudioEmitter>();
        self.sound_cube_emitter.set_sound(pop_sound);

        let mut wet_cube = world.spawn(&sound_cube_prefab);
        wet_cube
            .at(10.0, 150.0, 10.0)
            .build_component::<Collider3D>()
            .build_component::<RigidBodyComponent>()
            .enable_ccd();

        let mut reverb_track = SpatialTrackBuilder::new();
        reverb_track.add_effect(ReverbBuilder::new());
        self.sound_cube2_emitter = wet_cube.add_component::<AudioEmitter>();
        self.sound_cube2_emitter
            .set_track(world, reverb_track)
            .set_sound(pop_sound);

        Ok(())
    }

    fn spawn_3d_text(world: &mut World) -> GameObjectRef {
        let id = world.new_object("Text 3D");
        let mut text = world
            .get_object_ref(id)
            .expect("newly created text object should exist");
        let mut text3d = text.add_component::<Text3D>();

        text3d.set_size(1.0);
        text3d.set_alignment(TextAlignment::Center);
        text.transform.set_position(-15., 2., 2.);
        text.transform.set_euler_rotation_deg(0., 90., 0.);
        text3d.set_rainbow_mode(true);

        world.add_child(&text);
        text
    }

    fn spawn_spring_demo(world: &mut World) {
        let mut spring_bottom = world
            .spawn(&CubePrefab::new(HMaterial::DEFAULT))
            .at(-5., 10., -20.)
            .build_component::<Collider3D>()
            .mass(1.0)
            .build_component::<RigidBodyComponent>()
            .enable_ccd()
            .id;
        let spring_top = world
            .spawn(&CubePrefab::new(HMaterial::DEFAULT))
            .at(-5., 20., -20.)
            .build_component::<Collider3D>()
            .mass(1.0)
            .build_component::<RigidBodyComponent>()
            .enable_ccd()
            .id;

        let mut spring = spring_bottom.add_component::<SpringJoint>();
        spring.connect_to(spring_top);
        spring.set_rest_length(10.);
    }

    // fn cleanup_color_pads(world: &mut World) {
    //     for mut object in &world.objects {
    //         if object.1.name.starts_with("Plane") {
    //             object.0.delete();
    //         }
    //     }
    // }

    fn spawn_spotlights(world: &mut World) -> (CRef<SpotLightComponent>, CRef<SpotLightComponent>) {
        let mut spot = world.new_object("Spot");
        spot.at(5., 5., -5.)
            .transform
            .set_euler_rotation_deg(0., 80., 0.);
        let mut light1 = spot.add_component::<SpotLightComponent>();
        light1.set_color(1.0, 0.2, 0.2);
        light1.set_intensity(10000.);
        light1.set_inner_angle(20.);
        light1.set_outer_angle(30.);

        let mut spot2 = world.new_object("Spot 2");
        spot2
            .at(5., 5., -10.)
            .transform
            .set_euler_rotation_deg(0., 100., 0.);
        let mut light2 = spot2.add_component::<SpotLightComponent>();
        light2.set_color(0.2, 0.2, 1.0);
        light2.set_intensity(10000.);
        light2.set_inner_angle(20.);
        light2.set_outer_angle(30.);

        (light1, light2)
    }

    fn update_camera_zoom(&self, world: &mut World) {
        let mut zoom_down = world.input.gamepad.button(Button::LeftTrigger2);
        if world.input.is_button_pressed(MouseButton::Right) {
            zoom_down = 1.0;
        }

        if let Some(mut camera) = world
            .active_camera()
            .upgrade(world)
            .and_then(|cam| cam.parent().get_component::<FirstPersonCameraController>())
        {
            camera.set_zoom(zoom_down);
        }
    }

    fn update_world_text(&self, world: &mut World) {
        if let Some(mut text3d) = self.text3d.get_component::<Text3D>() {
            text3d.set_text(format!(
                "There are {} Objects in the World",
                world.objects.len(),
            ));
        }
    }

    fn handle_player_toggle(&mut self, world: &World) {
        if world.input.is_key_down(KeyCode::KeyF) {
            let is_kinematic = self.player_rb.is_kinematic();
            self.player_rb.set_kinematic(!is_kinematic);
        }
    }

    fn format_title(&self, is_viewport: bool) -> String {
        let debug_or_release = if cfg!(debug_assertions) {
            "[DEBUG]"
        } else {
            "[RELEASE]"
        };

        let viewport = if is_viewport { "(Viewport)" } else { "" };

        format!(
            "{debug_or_release} {} {viewport} - FPS: [ {} ]",
            syrillian::ENGINE_STR,
            self.frame_counter.fps(),
        )
    }

    fn update_pickup_interaction(&mut self, world: &mut World) {
        let Some(camera) = world.active_camera().upgrade(world) else {
            return;
        };
        let Some(camera_obj) = world.get_object_ref(camera.parent()) else {
            return;
        };

        let pick_up = world.input.gamepad.is_button_down(Button::RightTrigger)
            || world.input.is_button_down(MouseButton::Left);
        let drop = world.input.gamepad.is_button_released(Button::RightTrigger)
            || world.input.is_button_released(MouseButton::Left);

        if pick_up {
            let Some(collider) = self.player.get_component::<Collider3D>() else {
                return;
            };
            let player_collider = collider.phys_handle.unwrap_or_else(ColliderHandle::invalid);
            let ray = Ray::new(
                camera_obj.transform.position(),
                camera_obj.transform.forward(),
            );
            let intersect = world.physics.cast_ray(
                &ray,
                5.,
                false,
                QueryFilter::only_dynamic().exclude_collider(player_collider),
            );

            #[cfg(debug_assertions)]
            {
                let mut camera = camera;
                camera.push_debug_ray(ray, 5.);
            }

            match intersect {
                None => info!("No ray intersection"),
                Some((dt, obj)) => {
                    if let Some(obj_ref) = world.get_object_ref(obj)
                        && let Some(mut rb) = obj.get_component::<RigidBodyComponent>()
                    {
                        rb.set_kinematic(true);
                        let body = rb.body_mut().unwrap();
                        body.set_linvel(Vec3::ZERO, true);
                        body.set_angvel(Vec3::ZERO, true);

                        info!("Intersection after {dt}s, against: {}", obj_ref.name);
                        self.picked_up = Some(obj_ref);
                    }
                }
            }
        } else if drop {
            if let Some(obj) = self.picked_up.as_mut()
                && let Some(mut rb) = obj.get_component::<RigidBodyComponent>()
            {
                rb.set_kinematic(false);
            }
            self.picked_up = None;
        }

        if let Some(obj) = self.picked_up.as_mut() {
            let delta = world.delta_time().as_secs_f32();
            let scale = obj.transform.scale();
            let target_position = camera_obj.transform.position()
                + camera_obj.transform.forward() * scale.length().max(1.) * 2.;
            let position = obj.transform.position();
            let rotation: Quat = obj.transform.rotation();
            let camera_rotation: Quat = camera_obj.transform.rotation();
            let target_rotation = rotation.slerp(camera_rotation, 10.0 * delta);
            // let next_rot = smooth_rot(rotation, target_rotation, delta, 15.0);
            let next_pos = position.lerp(target_position, 100.03 * delta);
            if let Some(mut rb) = obj.get_component::<RigidBodyComponent>() {
                rb.set_kinematic(true);
                rb.body_mut()
                    .unwrap()
                    .set_next_kinematic_position(Pose::from_parts(next_pos, target_rotation));
            }
        }
    }

    fn update_audio_controls(&mut self, world: &World) {
        if world.input.is_key_down(KeyCode::KeyU) {
            self.sound_cube_emitter.toggle_looping();
        }
        if world.input.is_key_down(KeyCode::KeyI) {
            self.sound_cube2_emitter.toggle_looping();
        }
        if world.input.is_key_down(KeyCode::KeyP) {
            if world.input.is_key_pressed(KeyCode::ShiftLeft) {
                self.sound_cube_emitter.stop();
            } else {
                self.sound_cube_emitter.play();
            }
        }
        if world.input.is_key_down(KeyCode::KeyO) {
            if world.input.is_key_pressed(KeyCode::ShiftLeft) {
                self.sound_cube2_emitter.stop();
            } else {
                self.sound_cube2_emitter.play();
            }
        }
    }

    fn spawn_on_demand_cubes(&mut self, world: &mut World) {
        let Some(camera) = world.active_camera().upgrade(world) else {
            return;
        };
        let Some(camera_obj) = world.get_object_ref(camera.parent()) else {
            return;
        };
        self.spawn_on_demand_cubes_with_camera(world, &camera_obj);
    }

    fn spawn_on_demand_cubes_with_camera(&self, world: &mut World, camera_obj: &GameObjectRef) {
        if !world.input.is_key_down(KeyCode::KeyC)
            && !world.input.gamepad.is_button_down(Button::West)
        {
            return;
        }

        let pos = camera_obj.transform.position() + camera_obj.transform.forward() * 3.;
        world
            .spawn(&CubePrefab {
                material: HMaterial::DEFAULT,
            })
            .at_vec(pos)
            .build_component::<Collider3D>()
            .build_component::<RigidBodyComponent>();

        let sleeping_bodies = world
            .physics
            .rigid_body_set
            .iter()
            .filter(|c| c.1.is_sleeping())
            .count();
        println!("{sleeping_bodies} Bodies are currently sleeping");
    }

    fn handle_debug_overlays(&mut self, world: &mut World) {
        #[cfg(debug_assertions)]
        if world.input.is_key_down(KeyCode::KeyL) {
            let mode = DebugRenderer::next_mode();
            if let Some(mut collider) = self.player.get_component::<Collider3D>() {
                if collider.is_local_debug_render_enabled() {
                    collider.set_local_debug_render_enabled(false);
                } else if mode == 0 {
                    collider.set_local_debug_render_enabled(true);
                }
            }
        }

        #[cfg(not(debug_assertions))]
        let _ = world;
    }
}

fn main() {
    // let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new().build();
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
        //     .with(chrome_layer)
        .init();

    let app = MyMain::configure("MyMain", 1280, 720);

    if let Err(e) = app.run() {
        error!("{e}");
    }
}

pub struct City;
impl Prefab for City {
    fn prefab_name(&self) -> &'static str {
        "City"
    }

    fn build(&self, world: &mut World) -> GameObjectId {
        let testmap = include_bytes!("../../syrillian/testmodels/testmap/testmap.glb");
        let mut city = SceneLoader::load_buffer(world, testmap).expect("Failed to load city file");

        // add colliders to city
        city.add_child_components_then(Collider3D::please_use_mesh);

        city
    }
}

pub fn smooth_rot(
    current: Quat,
    target: Quat,
    dt: f32,
    responsiveness: f32, // e.g. 15..30
) -> Quat {
    let a = 1.0 - (-responsiveness * dt).exp();
    current.slerp(target, a)
}
