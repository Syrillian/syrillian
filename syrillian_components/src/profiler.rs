use syrillian::World;
use syrillian::assets::{HMaterialInstance, HTexture2D, MaterialInstance, Texture2D};
use syrillian::components::{Component, UiContext};
use syrillian::core::ObjectHash;
use syrillian::input::KeyCode;
use syrillian::math::{Vec2, Vec3, vec2, vec4};
use syrillian::utils::FrameCounter;
use syrillian_render::rendering::TextureFormat;
use syrillian_render::rendering::message::GBufferDebugTargets;
use syrillian_render::rendering::viewport::ViewportId;
use syrillian_render::strobe::ui_element::Padding;
use syrillian_render::strobe::{UiBuilder, UiImage, UiLine, UiText};
use web_time::Instant;

pub struct Profiler {
    frames: FrameCounter,
    frames_instant: FrameCounter,
    last_update: Instant,
    show_gbuffers: bool,
    gbuffer_dirty: bool,
    gbuffer_view: Option<GBufferView>,
}

struct GBufferView {
    size: (u32, u32),
    normal_texture: HTexture2D,
    material_texture: HTexture2D,
    normal_material: HMaterialInstance,
    material_material: HMaterialInstance,
}

impl Default for Profiler {
    fn default() -> Self {
        Profiler {
            frames: FrameCounter::default(),
            frames_instant: FrameCounter::default(),
            last_update: Instant::now(),
            show_gbuffers: false,
            gbuffer_dirty: false,
            gbuffer_view: None,
        }
    }
}

impl Component for Profiler {
    fn init(&mut self, world: &mut World) {
        if cfg!(feature = "physics_profiler") {
            world.physics.physics_pipeline.counters.enable();
        }
    }

    fn on_gui(&mut self, world: &mut World, ctx: UiContext) {
        let object_hash = self.parent().object_hash();

        self.frames_instant.new_frame_from_world(world);

        if world.input.is_key_down(KeyCode::F11) {
            self.show_gbuffers = !self.show_gbuffers;
            self.gbuffer_dirty = self.show_gbuffers;

            if !self.show_gbuffers {
                let _ = world.set_gbuffer_debug_targets(ViewportId::PRIMARY, None);
            }
        }

        if self.last_update.elapsed().as_secs_f32() > 1.0 {
            self.frames = self.frames_instant.clone();
            self.last_update = Instant::now();
        }

        let low = self.frames.fps_low();
        let mean = self.frames.fps_mean();
        let high = self.frames.fps_high();

        let counters = &world.physics.physics_pipeline.counters;
        let dt = world.physics.integration_parameters.dt as f64 * 1000.0;
        let step_time = counters.step_time_ms();

        if self.show_gbuffers {
            self.sync_gbuffer_targets(world);
        }

        ctx.draw(world, ViewportId::PRIMARY, |ui| {
            ui.vertical(|ui| {
                ui.style.padding = Padding::all(5.0);

                ui.add(
                    UiText::new(format!("FPS: L {low:.2} | ~ {mean:.2} | H {high:.2}"))
                        .font_size(11.0)
                        .color(Vec3::ONE)
                        .click_listener(object_hash)
                        .into(),
                );

                ui.spacing(Vec2::new(0.0, 5.0));

                ui.add(
                    UiLine::new(vec2(120.0, 0.0))
                        .gradient(vec4(1.0, 1.0, 1.0, 5.0), vec4(1.0, 1.0, 1.0, 0.5))
                        .thickness(1.0)
                        .click_listener(object_hash)
                        .into(),
                );

                // profiling info is disabled in release mode
                if cfg!(feature = "physics_profiler") {
                    Self::draw_physics_profiling(object_hash, dt, step_time, ui);
                }

                if self.show_gbuffers {
                    self.draw_gbuffer_view(object_hash, ui);
                }
            });
        });
    }
}

impl Profiler {
    fn sync_gbuffer_targets(&mut self, world: &mut World) {
        let Some(size) = world.viewport_size(ViewportId::PRIMARY) else {
            self.gbuffer_view = None;
            self.gbuffer_dirty = false;
            let _ = world.set_gbuffer_debug_targets(ViewportId::PRIMARY, None);
            return;
        };

        let size = (size.width.max(1), size.height.max(1));
        let rebuild = self
            .gbuffer_view
            .as_ref()
            .map_or(true, |view| view.size != size);

        if rebuild {
            self.gbuffer_view = Some(Self::create_gbuffer_view(world, size));
            self.gbuffer_dirty = true;
        }

        if self.gbuffer_dirty {
            if let Some(view) = &self.gbuffer_view {
                let _ = world.set_gbuffer_debug_targets(
                    ViewportId::PRIMARY,
                    Some(GBufferDebugTargets {
                        normal: view.normal_texture,
                        material: view.material_texture,
                    }),
                );
            }
            self.gbuffer_dirty = false;
        }
    }

    fn create_gbuffer_view(world: &World, size: (u32, u32)) -> GBufferView {
        let normal_texture = Self::make_debug_texture(world, size, TextureFormat::Rg16Float);
        let material_texture = Self::make_debug_texture(world, size, TextureFormat::Bgra8Unorm);

        let normal_material = MaterialInstance::builder()
            .name("GBuffer Normal Preview")
            .diffuse_texture(normal_texture)
            .store(world);

        let material_material = MaterialInstance::builder()
            .name("GBuffer Material Preview")
            .diffuse_texture(material_texture)
            .store(world);

        GBufferView {
            size,
            normal_texture,
            material_texture,
            normal_material,
            material_material,
        }
    }

    fn make_debug_texture(world: &World, size: (u32, u32), format: TextureFormat) -> HTexture2D {
        let store = <World as AsRef<syrillian::assets::store::Store<Texture2D>>>::as_ref(world);
        let base = store.get(HTexture2D::FALLBACK_DIFFUSE).clone();

        let texture = Texture2D {
            width: size.0,
            height: size.1,
            format,
            data: None,
            repeat_mode: base.repeat_mode,
            filter_mode: base.filter_mode,
            mip_filter_mode: base.mip_filter_mode,
            has_transparency: false,
        };

        store.add(texture)
    }

    fn draw_gbuffer_view(&self, object_hash: ObjectHash, ui: &mut UiBuilder) {
        let Some(view) = &self.gbuffer_view else {
            ui.spacing(Vec2::new(0.0, 5.0));
            ui.add(
                UiText::new("G-Buffers unavailable (viewport not ready)")
                    .font_size(11.0)
                    .color(Vec3::ONE)
                    .click_listener(object_hash)
                    .into(),
            );
            return;
        };

        let preview_size = vec2(160.0, 90.0);

        ui.spacing(Vec2::new(0.0, 6.0));

        ui.add(
            UiText::new("G-Buffers (F11 to toggle)")
                .font_size(11.0)
                .color(Vec3::ONE)
                .click_listener(object_hash)
                .into(),
        );

        ui.spacing(Vec2::new(0.0, 4.0));

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add(
                    UiText::new("Normals")
                        .font_size(10.0)
                        .color(Vec3::ONE)
                        .click_listener(object_hash)
                        .into(),
                );
                ui.add(
                    UiImage::new(view.normal_material)
                        .size(preview_size)
                        .object(object_hash)
                        .into(),
                );
            });

            ui.spacing(vec2(8.0, 0.0));

            ui.vertical(|ui| {
                ui.add(
                    UiText::new("Material")
                        .font_size(10.0)
                        .color(Vec3::ONE)
                        .click_listener(object_hash)
                        .into(),
                );
                ui.add(
                    UiImage::new(view.material_material)
                        .size(preview_size)
                        .object(object_hash)
                        .into(),
                );
            });
        });
    }

    fn draw_physics_profiling(
        object_hash: ObjectHash,
        dt: f64,
        step_time: f64,
        ui: &mut UiBuilder,
    ) {
        ui.spacing(Vec2::new(0.0, 5.0));

        ui.add(
            UiText::new("Physics:")
                .font_size(11.0)
                .color(Vec3::ONE)
                .click_listener(object_hash)
                .into(),
        );

        ui.add(
            UiText::new(format!("dt: {dt:.4}ms",))
                .font_size(11.0)
                .color(Vec3::ONE)
                .click_listener(object_hash)
                .into(),
        );

        ui.add(
            UiText::new(format!(
                "step time: {:.4}ms, load: {:.1}%",
                step_time,
                (step_time / dt * 100.0).min(100.0)
            ))
            .font_size(11.0)
            .color(Vec3::ONE)
            .click_listener(object_hash)
            .into(),
        );
    }
}
