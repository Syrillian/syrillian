use syrillian::World;
use syrillian::components::{Component, UiContext};
use syrillian::core::ObjectHash;
use syrillian::math::{Vec2, Vec3, vec2, vec4};
use syrillian::utils::FrameCounter;
use syrillian_render::rendering::viewport::ViewportId;
use syrillian_render::strobe::ui_element::Padding;
use syrillian_render::strobe::{UiBuilder, UiLine, UiText};
use web_time::Instant;

pub struct Profiler {
    frames: FrameCounter,
    frames_instant: FrameCounter,
    last_update: Instant,
}

impl Default for Profiler {
    fn default() -> Self {
        Profiler {
            frames: FrameCounter::default(),
            frames_instant: FrameCounter::default(),
            last_update: Instant::now(),
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

        ctx.draw(world, ViewportId::PRIMARY, |ui| {
            ui.vertical(|ui| {
                ui.style.padding = Padding::all(5.0);

                ui.add(
                    UiText::new(format!("FPS: L {low:.2} | Ø {mean:.2} | H {high:.2}"))
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
            });
        });
    }
}

impl Profiler {
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
