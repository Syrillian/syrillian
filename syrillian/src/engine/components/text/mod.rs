use crate::World;
use crate::assets::HFont;
use crate::components::Component;
use crate::rendering::proxies::SceneProxy;
use crate::rendering::proxies::text_proxy::{TextProxy, ThreeD, TwoD};
use crate::rendering::strobe::{TextAlignment, UiTextDraw};
use crate::rendering::{CPUDrawCtx, UiContext};
use crate::windowing::RenderTargetId;
use delegate::delegate;
use nalgebra::{Vector2, Vector3};
use syrillian_macros::Reflect;

#[derive(Debug, Clone, Reflect)]
pub struct Text2D {
    proxy: TextProxy<2, TwoD>,
}

impl Text2D {
    pub fn size(&self) -> f32 {
        self.proxy.size()
    }

    pub fn draw_order(&self) -> u32 {
        self.proxy.draw_order()
    }

    pub fn render_target(&self) -> RenderTargetId {
        self.proxy.render_target()
    }

    fn strobe_draw(&self) -> UiTextDraw {
        UiTextDraw {
            draw_order: self.proxy.draw_order(),
            font: self.proxy.font(),
            alignment: self.proxy.alignment(),
            letter_spacing_em: self.proxy.letter_spacing(),
            position: self.proxy.position(),
            size_em: self.proxy.size(),
            color: self.proxy.color(),
            rainbow: self.proxy.rainbow_mode(),
            text: self.proxy.text().to_string(),
            object_hash: self.parent().object_hash(),
        }
    }

    delegate! {
        to self.proxy {
            pub fn set_text(&mut self, text: impl Into<String>);
            pub fn set_alignment(&mut self, alignment: TextAlignment);
            pub fn set_font(&mut self, font: HFont);
            pub fn set_letter_spacing(&mut self, spacing_em: f32);
            pub const fn set_position(&mut self, x: f32, y: f32);
            pub const fn set_position_vec(&mut self, pos: Vector2<f32>);
            pub const fn set_color(&mut self, r: f32, g: f32, b: f32);
            pub const fn set_color_vec(&mut self, color: Vector3<f32>);
            pub const fn set_size(&mut self, text_size: f32);
            pub const fn set_rainbow_mode(&mut self, enable: bool);
            pub fn set_draw_order(&mut self, order: u32);
            pub fn set_render_target(&mut self, target: RenderTargetId);
        }
    }
}

impl Default for Text2D {
    fn default() -> Self {
        Self {
            proxy: TextProxy::new("".to_string(), HFont::DEFAULT, 100.0),
        }
    }
}

impl Component for Text2D {
    fn on_gui(&mut self, world: &mut World, ui: UiContext) {
        ui.text(world, self.render_target(), self.strobe_draw());
    }
}

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

    pub fn render_target(&self) -> RenderTargetId {
        self.proxy.render_target()
    }

    delegate! {
        to self.proxy {
            pub fn set_text(&mut self, text: impl Into<String>);
            pub fn set_alignment(&mut self, alignment: TextAlignment);
            pub fn set_font(&mut self, font: HFont);
            pub fn set_letter_spacing(&mut self, spacing_em: f32);
            pub const fn set_position(&mut self, x: f32, y: f32);
            pub const fn set_position_vec(&mut self, pos: Vector2<f32>);
            pub const fn set_color(&mut self, r: f32, g: f32, b: f32);
            pub const fn set_color_vec(&mut self, color: Vector3<f32>);
            pub const fn set_size(&mut self, text_size: f32);
            pub const fn set_rainbow_mode(&mut self, enable: bool);
            pub fn set_draw_order(&mut self, order: u32);
            pub fn set_render_target(&mut self, target: RenderTargetId);
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
