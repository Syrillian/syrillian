//! Example that renders a textured spinning cube and some 2d images.

use std::error::Error;
use syrillian::assets::{HMaterial, Material, StoreType, Texture2D};
use syrillian::core::{GameObjectExt, GameObjectId};
use syrillian::input::MouseButton;
use syrillian::math::{Vec2, Vec3};
use syrillian::physics::QueryFilter;
use syrillian::rendering::UiContext;
use syrillian::strobe::UiImage;
use syrillian::tracing::{info, warn};
use syrillian::{AppState, World};
use syrillian::{SyrillianApp, ViewportId};
use syrillian_components::prefabs::CubePrefab;
use syrillian_components::{Collider3D, RotateComponent};

const NECO_IMAGE: &[u8; 1293] = include_bytes!("assets/neco.jpg");

#[derive(Debug, SyrillianApp)]
struct NecoArc {
    dragging: Option<GameObjectId>,
    drag_offset: Vec3,
    drag_distance: f32,
    necoarc: HMaterial,
}

impl Default for NecoArc {
    fn default() -> Self {
        NecoArc {
            dragging: None,
            drag_offset: Vec3::ZERO,
            drag_distance: 0.0,
            necoarc: HMaterial::DEFAULT,
        }
    }
}

impl AppState for NecoArc {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.new_camera();

        let texture = Texture2D::load_image_from_memory(NECO_IMAGE)?.store(world);

        self.necoarc = world.assets.materials.add(
            Material::builder()
                .name("Neco Arc")
                .diffuse_texture(texture)
                .build(),
        );

        world
            .spawn(&CubePrefab::new(self.necoarc))
            .at(0.0, 0.0, -5.0)
            .build_component::<RotateComponent>()
            .build_component::<Collider3D>();

        Ok(())
    }

    fn update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.input.auto_quit_on_escape();
        self.handle_click(world);
        Ok(())
    }

    fn on_gui(&mut self, world: &mut World, ctx: &UiContext) -> Result<(), Box<dyn Error>> {
        ctx.draw(world, ViewportId::PRIMARY, |ui| {
            ui.vertical(|ui| {
                let total = ui.window_size();
                let image = UiImage::new(self.necoarc).size(Vec2::new(total.x, total.y / 4.0));

                ui.add(image.clone().into());
                ui.spacing(Vec2::new(0.0, total.y / 2.0));
                ui.add(image.clone().into());
            });
        });

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
                    self.drag_offset = ray.point_at(toi) - obj.transform.position();
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

            let new_pos = ray.point_at(self.drag_distance);
            dragging
                .transform
                .set_position_vec(new_pos - self.drag_offset);
        }
    }
}
