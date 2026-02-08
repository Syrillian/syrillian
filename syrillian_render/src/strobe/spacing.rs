use crate::strobe::UiDrawContext;
use crate::strobe::ui_element::{Rect, UiElement};
use glamx::Vec2;

pub struct UiSpacing {
    size: Vec2,
}

impl UiSpacing {
    pub fn new(size: Vec2) -> Self {
        Self { size }
    }
}

impl UiElement for UiSpacing {
    fn draw_order(&self) -> u32 {
        0
    }

    fn render(&self, _ctx: &mut UiDrawContext, _rect: Rect) {}

    fn measure(&self, _ctx: &mut UiDrawContext) -> Vec2 {
        self.size
    }
}
