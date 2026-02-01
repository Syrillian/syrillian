use crate::math::Vec2;
use crate::strobe::UiDrawContext;
use std::ops::Div;

#[derive(Copy, Clone, Debug, Default)]
pub struct Rect {
    pub position: Vec2,
    pub size: Vec2,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Padding {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Rect {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self { position, size }
    }

    pub fn min(&self) -> Vec2 {
        self.position
    }

    pub fn max(&self) -> Vec2 {
        self.position + self.size
    }
}

impl Div<Vec2> for Rect {
    type Output = Rect;

    fn div(mut self, rhs: Vec2) -> Self::Output {
        self.position /= rhs;
        self.size /= rhs;
        self
    }
}

impl Padding {
    pub fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Padding {
            top,
            bottom,
            left,
            right,
        }
    }

    pub fn all(px: f32) -> Self {
        Self::new(px, px, px, px)
    }

    pub fn top(px: f32) -> Self {
        Self::new(px, 0.0, 0.0, 0.0)
    }

    pub fn bottom(px: f32) -> Self {
        Self::new(0.0, px, 0.0, 0.0)
    }

    pub fn left(px: f32) -> Self {
        Self::new(0.0, 0.0, px, 0.0)
    }

    pub fn right(px: f32) -> Self {
        Self::new(0.0, 0.0, 0.0, px)
    }
}

pub trait UiElement: Send + Sync + 'static {
    fn draw_order(&self) -> u32;
    fn render(&self, ctx: &mut UiDrawContext, rect: Rect);

    fn measure(&self, _ctx: &mut UiDrawContext) -> Vec2 {
        Vec2::ZERO
    }
}

impl<E: UiElement> From<E> for Box<dyn UiElement> {
    fn from(value: E) -> Self {
        Box::new(value)
    }
}
