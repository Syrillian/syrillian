use std::time::Instant;
use syrillian::components::Component;
use syrillian::math::{Vec2, Vec3, vec2, vec4};
use syrillian::rendering::UiContext;
use syrillian::strobe::ui_element::Padding;
use syrillian::strobe::{UiLine, UiText};
use syrillian::utils::FrameCounter;
use syrillian::{ViewportId, World};

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
            });
        });
    }
}
