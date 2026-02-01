use delegate::delegate;
use syrillian::World;
use syrillian::assets::HFont;
use syrillian::components::Component;
use syrillian::math::{Vec2, Vec3};
use syrillian::rendering::CPUDrawCtx;
use syrillian::rendering::proxies::SceneProxy;
use syrillian::rendering::proxies::text_proxy::{TextProxy, ThreeD};
use syrillian::rendering::strobe::TextAlignment;
use syrillian::{Reflect, ViewportId};

#[derive(Debug, Clone, Reflect)]
pub struct Text3D {
    proxy: TextProxy<3, ThreeD>,
}

impl Text3D {
    pub fn size(&self) -> f32 {
        self.proxy.size()
    }

    pub fn draw_order(&self) -> u32 {
        self.proxy.draw_order()
    }

    pub fn render_target(&self) -> ViewportId {
        self.proxy.render_target()
    }

    delegate! {
        to self.proxy {
            pub fn set_text(&mut self, text: impl Into<String>);
            pub fn set_alignment(&mut self, alignment: TextAlignment);
            pub fn set_font(&mut self, font: HFont);
            pub fn set_letter_spacing(&mut self, spacing_em: f32);
            pub const fn set_position(&mut self, x: f32, y: f32);
            pub const fn set_position_vec(&mut self, pos: Vec2);
            pub const fn set_color(&mut self, r: f32, g: f32, b: f32);
            pub const fn set_color_vec(&mut self, color: Vec3);
            pub const fn set_size(&mut self, text_size: f32);
            pub const fn set_rainbow_mode(&mut self, enable: bool);
            pub fn set_draw_order(&mut self, order: u32);
            pub fn set_render_target(&mut self, target: ViewportId);
        }
    }
}

impl Default for Text3D {
    fn default() -> Self {
        Self {
            proxy: TextProxy::new("".to_string(), HFont::DEFAULT, 100.0),
        }
    }
}

impl Component for Text3D {
    fn create_render_proxy(&mut self, _world: &World) -> Option<Box<dyn SceneProxy>> {
        Some(Box::new(self.proxy.clone()))
    }

    fn update_proxy(&mut self, _world: &World, ctx: CPUDrawCtx) {
        self.proxy.update_game_thread(ctx);
    }
}
