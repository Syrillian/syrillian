use std::time::Instant;
use syrillian::assets::HFont;
use syrillian::components::Component;
use syrillian::math::{Vec2, Vec3, vec2, vec4};
use syrillian::rendering::UiContext;
use syrillian::strobe::{TextAlignment, UiLineDraw, UiTextDraw};
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

        ctx.text(
            world,
            ViewportId::PRIMARY,
            UiTextDraw {
                draw_order: 0,
                font: HFont::DEFAULT,
                alignment: TextAlignment::Left,
                letter_spacing_em: 0.0,
                position: Vec2::new(5.0, 10.0),
                size_em: 11.0,
                color: Vec3::ONE,
                rainbow: false,
                text: format!("FPS: L {low:.2} | Ø {mean:.2} | H {high:.2}"),
                object_hash,
            },
        );

        ctx.line(
            world,
            ViewportId::PRIMARY,
            UiLineDraw {
                draw_order: 0,
                from: vec2(5.0, 26.0),
                to: vec2(120.0, 26.0),
                from_color: vec4(1.0, 1.0, 1.0, 5.0),
                to_color: vec4(1.0, 1.0, 1.0, 0.5),
                thickness: 1.0,
                object_hash,
            },
        );
    }
}
