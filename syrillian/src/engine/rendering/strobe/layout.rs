use crate::ViewportId;
use crate::math::Vec2;
use crate::rendering::strobe::ui_element::{Rect, UiElement};
use crate::rendering::strobe::{CacheId, UiDrawContext};
use crate::strobe::UiSpacing;
use crate::strobe::style::Style;
use crate::strobe::ui_element::Padding;
use glamx::vec2;

#[derive(Debug, Clone, Copy)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
    Stack,
}

pub trait ContextWithId {
    fn set_id(&mut self, id: u32);
}

pub trait LayoutElement<C: ?Sized> {
    fn measure(&self, ctx: &mut C) -> Vec2;
    fn render_layout(&self, ctx: &mut C, rect: Rect);
}

impl LayoutElement<UiDrawContext<'_, '_, '_, '_, '_>> for Box<dyn UiElement> {
    fn measure(&self, ctx: &mut UiDrawContext) -> Vec2 {
        (**self).measure(ctx)
    }

    fn render_layout(&self, ctx: &mut UiDrawContext, rect: Rect) {
        (**self).render(ctx, rect)
    }
}

pub struct StrobeNode<T = Box<dyn UiElement>> {
    pub direction: LayoutDirection,
    pub padding: Padding,
    pub children: Vec<StrobeNode<T>>,
    pub element: Option<T>,
    pub id: u32,
}

impl<T> Default for StrobeNode<T> {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Vertical,
            padding: Padding::default(),
            children: Vec::new(),
            element: None,
            id: 0,
        }
    }
}

impl<T> StrobeNode<T> {
    pub fn new(direction: LayoutDirection) -> Self {
        Self {
            direction,
            padding: Padding::default(),
            children: Vec::new(),
            element: None,
            id: 0,
        }
    }

    pub fn leaf(element: T) -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            padding: Padding::default(),
            children: Vec::new(),
            element: Some(element),
            id: 0,
        }
    }
}

impl<T, C: ?Sized + ContextWithId> LayoutElement<C> for StrobeNode<T>
where
    T: LayoutElement<C>,
{
    fn measure(&self, ctx: &mut C) -> Vec2 {
        if let Some(element) = &self.element {
            return element.measure(ctx);
        }

        let mut width = 0.0;
        let mut height = 0.0f32;

        let calc_size: &mut dyn FnMut(Vec2) = match self.direction {
            LayoutDirection::Horizontal => &mut |size: Vec2| {
                width += size.x;
                height = height.max(size.y);
            },
            LayoutDirection::Vertical => &mut |size: Vec2| {
                width = width.max(size.x);
                height += size.y;
            },
            LayoutDirection::Stack => &mut |size: Vec2| {
                width = width.max(size.x);
                height = height.max(size.y);
            },
        };

        for child in &self.children {
            let size = child.measure(ctx);
            calc_size(size);
        }

        Vec2::new(width, height)
    }

    fn render_layout(&self, ctx: &mut C, mut rect: Rect) {
        rect.size.x -= self.padding.left + self.padding.right;
        rect.size.y -= self.padding.top + self.padding.bottom;
        rect.position.x += self.padding.left;
        rect.position.y += self.padding.top;

        rect.size = rect.size.max(vec2(0.0, 0.0));

        if let Some(element) = &self.element {
            element.render_layout(ctx, rect);
            return;
        }

        for child in &self.children {
            let size = child.measure(ctx);

            let rect = match self.direction {
                LayoutDirection::Horizontal => {
                    let pos_x = rect.position.x;
                    rect.position.x += size.x;

                    Rect::new(Vec2::new(pos_x, rect.position.y), Vec2::new(size.x, size.y))
                }
                LayoutDirection::Vertical => {
                    let pos_y = rect.position.y;
                    rect.position.y += size.y;

                    Rect::new(Vec2::new(rect.position.x, pos_y), Vec2::new(size.x, size.y))
                }
                LayoutDirection::Stack => rect,
            };

            ctx.set_id(child.id);
            child.render_layout(ctx, rect);
        }
    }
}

pub struct UiBuilder<'a, T = Box<dyn UiElement>> {
    node: &'a mut StrobeNode<T>,
    pub style: Style,
    size: Vec2,
    current_id: u32,
}

impl<'a, T> UiBuilder<'a, T> {
    pub fn new(node: &'a mut StrobeNode<T>, size: Vec2) -> Self {
        Self {
            node,
            style: Style::default(),
            size,
            current_id: 0,
        }
    }

    pub fn vertical(&mut self, f: impl FnOnce(&mut UiBuilder<T>)) {
        let mut node = StrobeNode::new(LayoutDirection::Vertical);
        node.id = self.current_id;
        node.padding = self.style.padding;

        self.current_id += 1;

        let mut builder = self.enter(&mut node);
        f(&mut builder);
        self.node.children.push(node);
    }

    pub fn horizontal(&mut self, f: impl FnOnce(&mut UiBuilder<T>)) {
        let mut node = StrobeNode::new(LayoutDirection::Horizontal);
        node.id = self.current_id;
        node.padding = self.style.padding;

        self.current_id += 1;

        let mut builder = self.enter(&mut node);
        f(&mut builder);
        self.node.children.push(node);
    }

    pub fn stack(&mut self, f: impl FnOnce(&mut UiBuilder<T>)) {
        let mut node = StrobeNode::new(LayoutDirection::Stack);
        node.id = self.current_id;
        node.padding = self.style.padding;

        self.current_id += 1;

        let mut builder = self.enter(&mut node);
        f(&mut builder);
        self.node.children.push(node);
    }

    pub fn add(&mut self, element: T) {
        let mut node = StrobeNode::leaf(element);
        node.id = self.current_id;
        node.padding = self.style.padding;

        self.current_id += 1;

        self.node.children.push(node);
    }

    pub fn window_size(&self) -> Vec2 {
        self.size
    }

    fn enter<'b>(&self, node: &'b mut StrobeNode<T>) -> UiBuilder<'b, T> {
        UiBuilder {
            node,
            style: Style::default(),
            size: self.size,
            current_id: self.current_id,
        }
    }
}

impl<'a> UiBuilder<'a, Box<dyn UiElement>> {
    pub fn spacing(&mut self, size: Vec2) {
        self.add(UiSpacing::new(size).into());
    }
}

pub struct StrobeRoot {
    pub root: StrobeNode<Box<dyn UiElement>>,
    pub target: ViewportId,
    pub cache_id: CacheId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct MockContext;

    impl ContextWithId for MockContext {
        fn set_id(&mut self, _id: u32) {}
    }

    #[derive(Clone)]
    struct MockElement {
        size: Vec2,
        layout_log: Rc<RefCell<Vec<Rect>>>,
    }

    impl MockElement {
        fn new(w: f32, h: f32, log: Rc<RefCell<Vec<Rect>>>) -> Self {
            Self {
                size: Vec2::new(w, h),
                layout_log: log,
            }
        }
    }

    impl LayoutElement<MockContext> for MockElement {
        fn measure(&self, _ctx: &mut MockContext) -> Vec2 {
            self.size
        }

        fn render_layout(&self, _ctx: &mut MockContext, rect: Rect) {
            self.layout_log.borrow_mut().push(rect);
        }
    }

    #[test]
    fn test_vertical_layout() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root: StrobeNode<MockElement> = StrobeNode::default();
        let mut builder = UiBuilder::new(&mut root, Vec2::ZERO);

        builder.vertical(|ui| {
            ui.add(MockElement::new(100.0, 50.0, log.clone()));
            ui.add(MockElement::new(100.0, 30.0, log.clone()));
        });

        let mut ctx = MockContext;

        let rect = Rect::new(Vec2::ZERO, Vec2::new(500.0, 500.0));
        root.render_layout(&mut ctx, rect);

        let calls = log.borrow();
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].position, Vec2::new(0.0, 0.0));
        assert_eq!(calls[0].size.y, 50.0);

        assert_eq!(calls[1].position, Vec2::new(0.0, 50.0));
        assert_eq!(calls[1].size.y, 30.0);
    }

    #[test]
    fn test_horizontal_layout() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root: StrobeNode<MockElement> = StrobeNode::new(LayoutDirection::Horizontal);
        let mut builder = UiBuilder::new(&mut root, Vec2::ZERO);

        builder.add(MockElement::new(50.0, 100.0, log.clone()));
        builder.add(MockElement::new(30.0, 100.0, log.clone()));

        let mut ctx = MockContext;
        let rect = Rect::new(Vec2::ZERO, Vec2::new(500.0, 500.0));
        root.render_layout(&mut ctx, rect);

        let calls = log.borrow();
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].position.x, 0.0);
        assert_eq!(calls[0].size.x, 50.0);

        assert_eq!(calls[1].position.x, 50.0);
        assert_eq!(calls[1].size.x, 30.0);
    }
}
