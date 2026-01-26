//! Example that renders a textured spinning cube and some 2d images.

use nalgebra::Vector3;
use rapier3d::prelude::QueryFilter;
use std::error::Error;
use syrillian::assets::{Material, StoreType, Texture};
use syrillian::core::{GameObjectExt, GameObjectId};
use syrillian::strobe::ImageScalingMode;
use syrillian::{AppState, World};
use syrillian_components::prefabs::CubePrefab;
use syrillian_components::{Button, Collider3D, Image, RotateComponent};
use syrillian_macros::SyrillianApp;
use tracing::{info, warn};
use winit::event::MouseButton;

const NECO_IMAGE: &[u8; 1293] = include_bytes!("assets/neco.jpg");

#[derive(Debug, SyrillianApp)]
struct NecoArc {
    dragging: Option<GameObjectId>,
    drag_offset: Vector3<f32>,
    drag_distance: f32,
}

impl Default for NecoArc {
    fn default() -> Self {
        NecoArc {
            dragging: None,
            drag_offset: Vector3::zeros(),
            drag_distance: 0.0,
        }
    }
}

impl AppState for NecoArc {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.new_camera();

        let texture = Texture::load_image_from_memory(NECO_IMAGE)?.store(world);

        let material = world.assets.materials.add(
            Material::builder()
                .name("Neco Arc")
                .diffuse_texture(texture)
                .build(),
        );

        world
            .spawn(&CubePrefab::new(material))
            .at(0.0, 0.0, -5.0)
            .build_component::<RotateComponent>()
            .build_component::<Collider3D>();

        let mut image_obj = world.new_object("Image");
        let mut image = image_obj.add_component::<Image>();
        image.set_scaling_mode(ImageScalingMode::RelativeStretch {
            left: 0.0,
            right: 1.0,
            top: 1.0,
            bottom: 0.8,
        });
        image.set_material(material);
        world.add_child(image_obj);

        let mut image_obj = world.new_object("Image 2");
        let mut image = image_obj.add_component::<Image>();
        let mut button = image_obj.add_component::<Button>();
        let image_2 = image.clone().downgrade();
        button.add_click_handler(move |w| {
            if let Some(image) = image_2.upgrade(w) {
                image.parent().remove_component(image, w);
            }
            info!("Image clicked!");
        });

        image.set_scaling_mode(ImageScalingMode::RelativeStretch {
            left: 0.0,
            right: 1.0,
            top: 0.2,
            bottom: 0.0,
        });
        image.set_material(material);
        world.add_child(image_obj);

        Ok(())
    }

    fn update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.input.auto_quit_on_escape();
        self.handle_click(world);
        Ok(())
    }
}

impl NecoArc {
    fn handle_click(&mut self, world: &mut World) {
        if world.input.is_button_down(MouseButton::Left) {
            let Some(ray) = world.physics.cursor_ray(world) else {
                warn!("No cursor ray available");
                return;
            };

            match world
                .physics
                .cast_ray(&ray, 100., false, QueryFilter::new())
            {
                None => {
                    info!("No click ray hit");
                    return;
                }
                Some((toi, obj)) => {
                    self.dragging = Some(obj);
                    self.drag_offset = ray.point_at(toi).coords - obj.transform.position();
                    self.drag_distance = toi;
                    info!("Click ray hit: {:?} after {toi}", obj.name);
                }
            };
            return;
        } else if world.input.is_button_released(MouseButton::Left) {
            self.dragging = None;
            self.drag_distance = 0.0;
        }

        if let Some(mut dragging) = self.dragging {
            let Some(ray) = world.physics.cursor_ray(world) else {
                warn!("No cursor ray available");
                return;
            };

            let new_pos = ray.point_at(self.drag_distance).coords;
            dragging
                .transform
                .set_position_vec(new_pos - self.drag_offset);
        }
    }
}
