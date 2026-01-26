//! Example to showcase dynamic shader switching / cache refresh.
//!
//! Hotkeys:
//! - Use L to toggle / switch to the next debug rendering mode

use crossbeam_channel::{Receiver, TryRecvError, unbounded};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use slotmap::Key;
use std::error::Error;
use std::fs;
use syrillian::assets::{HMaterial, HShader, Material, Shader, StoreType};
use syrillian::core::GameObjectId;
use syrillian::utils::validate_wgsl_source;
use syrillian::{AppState, World};
use syrillian_components::RotateComponent;
use syrillian_components::prefabs::CubePrefab;
use syrillian_macros::SyrillianApp;
use tracing::{debug, error, info};
use web_time::Instant;

#[cfg(debug_assertions)]
use syrillian::rendering::DebugRenderer;

const SHADER_PATH: &str = "examples/dynamic_shader/shader.wgsl";
const DEFAULT_VERT: &str =
    include_str!("../../src/engine/assets/shader/shaders/default_vertex3d.wgsl");

#[derive(SyrillianApp)]
struct DynamicShaderExample {
    last_successful_shader: Option<String>,
    last_refresh_time: Instant,
    shader_id: HShader,
    material_id: HMaterial,
    _watcher: RecommendedWatcher,
    file_events: Receiver<notify::Result<Event>>,
    cube: GameObjectId,
}

impl Default for DynamicShaderExample {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .expect("failed to create watcher");
        watcher
            .watch(SHADER_PATH.as_ref(), RecursiveMode::NonRecursive)
            .expect("failed to start watcher");
        watcher
            .configure(Config::default().with_compare_contents(true))
            .expect("failed to configure notify watcher");

        DynamicShaderExample {
            last_successful_shader: None,
            last_refresh_time: Instant::now(),
            shader_id: HShader::FALLBACK,
            material_id: HMaterial::FALLBACK,
            _watcher: watcher,
            file_events: rx,
            cube: GameObjectId::null(),
        }
    }
}

impl DynamicShaderExample {
    fn check_valid(source: &str) -> Result<(), String> {
        let code = Shader::new_default("Dynamic Shader", source).gen_code();

        validate_wgsl_source(&code).map_err(|e| e.emit_to_string(source))?;

        Ok(())
    }

    fn activate_shader(&mut self, world: &mut World, source: String) {
        let source_2 = source.clone(); // not the real one lol
        self.last_successful_shader = Some(source);

        if self.shader_id == HShader::FALLBACK {
            let shader = Shader::new_default("Dynamic Shader", source_2).store(world);
            self.shader_id = shader;
        } else {
            world
                .assets
                .shaders
                .get_mut(self.shader_id)
                .set_code(source_2);
        }

        match world.assets.materials.try_get_mut(self.material_id) {
            None => {
                self.material_id = Material::builder()
                    .name("Dynamic Shader Material")
                    .shader(self.shader_id)
                    .store(world);
            }
            Some(mut mat) => mat.shader = self.shader_id,
        }
    }

    fn try_load_shader(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        let mut source = fs::read_to_string(SHADER_PATH)?;
        source.insert_str(0, DEFAULT_VERT);

        if let Err(msg) = Self::check_valid(&source) {
            error!("{}", msg);
            Err(msg)?
        }

        self.activate_shader(world, source);

        Ok(())
    }

    fn refresh_shader(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        self.try_load_shader(world)?;
        self.respawn_cube(world);
        info!("Shader refreshed");

        Ok(())
    }

    fn poll(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        debug!("Polling for changes..");

        match self.file_events.try_recv() {
            Ok(event) => event?,
            Err(TryRecvError::Disconnected) => panic!("file events channel closed"),
            Err(TryRecvError::Empty) => {
                debug!("No changes");
                return Ok(());
            }
        };

        self.refresh_shader(world)?;

        Ok(())
    }

    fn respawn_cube(&mut self, world: &mut World) {
        let mut iter = 90.;
        let mut y_rot = 45.;
        if self.cube.exists() {
            let old_comp = self.cube.get_component::<RotateComponent>().unwrap();
            iter = old_comp.iteration;
            y_rot = old_comp.y_rot;

            self.cube.delete();
        }

        self.cube = world.spawn(&CubePrefab {
            material: self.material_id,
        });

        self.cube.transform.set_scale(2.0);
        self.cube.transform.set_position(0., 0., -5.0);
        let mut new_comp = self.cube.add_component::<RotateComponent>();
        new_comp.iteration = iter;
        new_comp.y_rot = y_rot;
        new_comp.rotate_speed = 0.0;
    }
}

impl AppState for DynamicShaderExample {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        _ = self.try_load_shader(world);
        self.respawn_cube(world);

        world.new_camera();

        #[cfg(debug_assertions)]
        DebugRenderer::off();

        Ok(())
    }
    fn update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        if self.last_refresh_time.elapsed().as_secs() > 0 {
            self.poll(world)?;
            self.last_refresh_time = Instant::now();
        }

        #[cfg(debug_assertions)]
        {
            use winit::keyboard::KeyCode;

            if world.input.is_key_down(KeyCode::KeyL) {
                DebugRenderer::next_mode();
            }
        }

        Ok(())
    }
}
