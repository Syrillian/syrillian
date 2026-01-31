use crate::ViewportId;
use crate::strobe::ui_element::UiElement;
use crate::strobe::{UiDrawContext, UiImageDraw, UiLineDraw, UiTextDraw};

pub type CacheId = u64;

pub struct UiDraw(CacheId, ViewportId, Box<dyn UiElement>);

#[derive(Default)]
pub struct StrobeFrame {
    pub draws: Vec<UiDraw>,
}

impl UiDraw {
    pub fn image(cache_id: CacheId, target: ViewportId, image: Box<UiImageDraw>) -> Self {
        UiDraw(cache_id, target, image)
    }

    pub fn text(cache_id: CacheId, target: ViewportId, text: Box<UiTextDraw>) -> Self {
        UiDraw(cache_id, target, text)
    }

    pub fn line(cache_id: CacheId, target: ViewportId, line: Box<UiLineDraw>) -> Self {
        UiDraw(cache_id, target, line)
    }

    pub fn cache_id(&self) -> CacheId {
        self.0
    }

    pub fn draw_target(&self) -> ViewportId {
        self.1
    }

    pub fn draw_order(&self) -> u32 {
        self.2.draw_order()
    }

    pub fn render(&self, ctx: &mut UiDrawContext) {
        self.2.render(ctx);
    }
}

impl StrobeFrame {
    pub fn sort(&mut self) {
        self.draws.sort_by_key(|d| (d.1, d.2.draw_order()));
    }
}
