//! Example that renders a textured spinning cube and some 2d images.

use std::error::Error;
use syrillian::SyrillianApp;
use syrillian::assets::store::StoreType;
use syrillian::assets::{HMaterialInstance, MaterialInstance, Texture2D};
use syrillian::components::UiContext;
use syrillian::core::{GameObjectExt, GameObjectId};
use syrillian::input::MouseButton;
use syrillian::math::{Vec2, Vec3};
use syrillian::physics::QueryFilter;
use syrillian::rendering::rendering::viewport::ViewportId;
use syrillian::shadergen::function::{
    ExpressionInput, ExpressionTexture, MaterialExpression, MaterialExpressionValue,
};
use syrillian::shadergen::value::MaterialValueType;
use syrillian::shadergen::{MaterialCompiler, NodeId};
use syrillian::strobe::UiImage;
use syrillian::tracing::{info, warn};
use syrillian::{AppState, World};
use syrillian_components::prefabs::CubePrefab;
use syrillian_components::{Collider3D, RotateComponent};

const NECO_IMAGE: &[u8; 1293] = include_bytes!("assets/neco.jpg");

struct TextureDownsizeMaterial {
    diffuse: ExpressionInput<Vec3>,
    use_diffuse_texture: ExpressionInput<bool>,
    diffuse_texture: ExpressionTexture,
    use_normal_texture: ExpressionInput<bool>,
    normal_texture: ExpressionTexture,
    roughness: ExpressionInput<f32>,
    use_roughness_texture: ExpressionInput<bool>,
    roughness_texture: ExpressionTexture,
    metallic: ExpressionInput<f32>,
    alpha: ExpressionInput<f32>,
    lit: ExpressionInput<bool>,
    cast_shadows: ExpressionInput<bool>,
    grayscale_diffuse: ExpressionInput<bool>,
}

impl Default for TextureDownsizeMaterial {
    fn default() -> Self {
        Self {
            diffuse: ExpressionInput::material("diffuse"),
            use_diffuse_texture: ExpressionInput::material("use_diffuse_texture"),
            diffuse_texture: ExpressionTexture::material("diffuse"),
            use_normal_texture: ExpressionInput::material("use_normal_texture"),
            normal_texture: ExpressionTexture::material("normal"),
            roughness: ExpressionInput::material("roughness"),
            use_roughness_texture: ExpressionInput::material("use_roughness_texture"),
            roughness_texture: ExpressionTexture::material("roughness"),
            metallic: ExpressionInput::material("metallic"),
            alpha: ExpressionInput::material("alpha"),
            lit: ExpressionInput::material("lit"),
            cast_shadows: ExpressionInput::material("cast_shadows"),
            grayscale_diffuse: ExpressionInput::material("grayscale_diffuse"),
        }
    }
}

impl MaterialExpression for TextureDownsizeMaterial {
    fn bind_inputs(&mut self, compiler: &mut MaterialCompiler) {
        self.diffuse.bind(compiler);
        self.use_diffuse_texture.bind(compiler);
        self.diffuse_texture.bind(compiler);
        self.use_normal_texture.bind(compiler);
        self.normal_texture.bind(compiler);
        self.roughness.bind(compiler);
        self.use_roughness_texture.bind(compiler);
        self.roughness_texture.bind(compiler);
        self.metallic.bind(compiler);
        self.alpha.bind(compiler);
        self.lit.bind(compiler);
        self.cast_shadows.bind(compiler);
        self.grayscale_diffuse.bind(compiler);
    }

    fn outputs(&self) -> Vec<MaterialExpressionValue> {
        vec![MaterialExpressionValue {
            name: "out",
            value_type: MaterialValueType::Vec4,
        }]
    }

    fn compile(&self, compiler: &mut MaterialCompiler, _output_index: u32) -> NodeId {
        let scale = compiler.constant_f32(10.0);
        let uv = compiler.vertex_uv();
        let uv_scaled = compiler.mul(uv, scale);

        let diffuse = compiler.base_color(
            uv_scaled,
            &self.diffuse,
            &self.use_diffuse_texture,
            &self.diffuse_texture,
        );

        let normal = compiler.normal(uv_scaled, &self.use_normal_texture, &self.normal_texture);
        let roughness = compiler.roughness(
            uv_scaled,
            &self.roughness,
            &self.use_roughness_texture,
            &self.roughness_texture,
        );
        let metallic = self.metallic.node();
        let alpha = self.alpha.node();
        let lit = self.lit.node();
        let cast_shadows = self.cast_shadows.node();
        let grayscale = self.grayscale_diffuse.node();

        compiler.pbr_shader(
            diffuse,
            normal,
            roughness,
            metallic,
            alpha,
            lit,
            cast_shadows,
            grayscale,
        )
    }
}

#[derive(Debug, SyrillianApp)]
struct NecoArc {
    dragging: Option<GameObjectId>,
    drag_offset: Vec3,
    drag_distance: f32,
    necoarc: HMaterialInstance,
}

impl Default for NecoArc {
    fn default() -> Self {
        NecoArc {
            dragging: None,
            drag_offset: Vec3::ZERO,
            drag_distance: 0.0,
            necoarc: HMaterialInstance::DEFAULT,
        }
    }
}

impl AppState for NecoArc {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.new_camera();

        let texture = Texture2D::load_image_from_memory(NECO_IMAGE)?.store(world);

        let custom_material = world
            .assets
            .register_custom_material("Checkered Material", TextureDownsizeMaterial::default());

        self.necoarc = MaterialInstance::builder()
            .name("Neco Arc")
            .material(custom_material)
            .diffuse_texture(texture)
            .build()
            .store(world);

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
