use crate::rendering::viewport::ViewportId;
use crate::strobe::UiSpacing;
use crate::strobe::input::{StrobeInputState, UiInteraction};
use crate::strobe::style::{Align, Size, Style};
use crate::strobe::ui_element::Padding;
use crate::strobe::ui_element::{Rect, UiElement};
use crate::strobe::{CacheId, UiDrawContext};
use glamx::{Vec2, vec2};

#[derive(Debug, Clone, Copy)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
    Stack,
}

pub trait ContextWithId {
    fn set_id(&mut self, id: u32);
    fn register_hit_rect(&mut self, _rect: Rect, _id: u32) {}
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
    pub width: Size,
    pub height: Size,
    pub align: Align,
}

impl<T> Default for StrobeNode<T> {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Vertical,
            padding: Padding::default(),
            children: Vec::new(),
            element: None,
            id: 0,
            width: Size::Auto,
            height: Size::Auto,
            align: Align::Start,
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
            width: Size::Auto,
            height: Size::Auto,
            align: Align::Start,
        }
    }

    pub fn leaf(element: T) -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            padding: Padding::default(),
            children: Vec::new(),
            element: Some(element),
            id: 0,
            width: Size::Auto,
            height: Size::Auto,
            align: Align::Start,
        }
    }

    fn main_axis_size(&self, direction: LayoutDirection) -> Size {
        match direction {
            LayoutDirection::Horizontal => self.width,
            LayoutDirection::Vertical => self.height,
            LayoutDirection::Stack => Size::Auto,
        }
    }
}

fn main_axis(v: Vec2, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::Horizontal => v.x,
        LayoutDirection::Vertical | LayoutDirection::Stack => v.y,
    }
}

fn cross_axis(v: Vec2, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::Horizontal => v.y,
        LayoutDirection::Vertical | LayoutDirection::Stack => v.x,
    }
}

fn make_vec(main: f32, cross: f32, direction: LayoutDirection) -> Vec2 {
    match direction {
        LayoutDirection::Horizontal => Vec2::new(main, cross),
        LayoutDirection::Vertical | LayoutDirection::Stack => Vec2::new(cross, main),
    }
}

impl<T, C: ?Sized + ContextWithId> LayoutElement<C> for StrobeNode<T>
where
    T: LayoutElement<C>,
{
    fn measure(&self, ctx: &mut C) -> Vec2 {
        if let Some(element) = &self.element {
            let intrinsic = element.measure(ctx);
            let pad = vec2(
                self.padding.left + self.padding.right,
                self.padding.top + self.padding.bottom,
            );
            return intrinsic + pad;
        }

        if matches!(self.direction, LayoutDirection::Stack) {
            let mut width = 0.0f32;
            let mut height = 0.0f32;
            for child in &self.children {
                let size = child.measure(ctx);
                width = width.max(size.x);
                height = height.max(size.y);
            }
            let pad = vec2(
                self.padding.left + self.padding.right,
                self.padding.top + self.padding.bottom,
            );
            return Vec2::new(width, height) + pad;
        }

        let dir = self.direction;
        let mut main_total = 0.0f32;
        let mut cross_max = 0.0f32;

        for child in &self.children {
            match child.main_axis_size(dir) {
                Size::Fill(_) => {
                    // Fill children contribute 0 on main axis during measure
                    let size = child.measure(ctx);
                    cross_max = cross_max.max(cross_axis(size, dir));
                }
                Size::Fixed(px) => {
                    main_total += px;
                    let size = child.measure(ctx);
                    cross_max = cross_max.max(cross_axis(size, dir));
                }
                Size::Auto => {
                    let size = child.measure(ctx);
                    main_total += main_axis(size, dir);
                    cross_max = cross_max.max(cross_axis(size, dir));
                }
            }
        }

        let pad = vec2(
            self.padding.left + self.padding.right,
            self.padding.top + self.padding.bottom,
        );
        make_vec(main_total, cross_max, dir) + pad
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

        if matches!(self.direction, LayoutDirection::Stack) {
            for child in &self.children {
                ctx.set_id(child.id);
                ctx.register_hit_rect(rect, child.id);
                child.render_layout(ctx, rect);
            }
            return;
        }

        let dir = self.direction;
        let inner_main = main_axis(rect.size, dir);
        let inner_cross = cross_axis(rect.size, dir);

        // Step 1: measure non-fill children, collect fill weights
        let mut child_main_sizes: Vec<Option<f32>> = Vec::with_capacity(self.children.len());
        let mut child_cross_sizes: Vec<f32> = Vec::with_capacity(self.children.len());
        let mut non_fill_total = 0.0f32;
        let mut fill_weight_total = 0.0f32;

        for child in &self.children {
            match child.main_axis_size(dir) {
                Size::Fill(w) => {
                    fill_weight_total += w;
                    child_main_sizes.push(None);
                    let size = child.measure(ctx);
                    child_cross_sizes.push(cross_axis(size, dir));
                }
                Size::Fixed(px) => {
                    non_fill_total += px;
                    child_main_sizes.push(Some(px));
                    let size = child.measure(ctx);
                    child_cross_sizes.push(cross_axis(size, dir));
                }
                Size::Auto => {
                    let size = child.measure(ctx);
                    let m = main_axis(size, dir);
                    non_fill_total += m;
                    child_main_sizes.push(Some(m));
                    child_cross_sizes.push(cross_axis(size, dir));
                }
            }
        }

        // Step 2: distribute remaining space to fill children
        let remaining = (inner_main - non_fill_total).max(0.0);
        for (i, child) in self.children.iter().enumerate() {
            if child_main_sizes[i].is_none() {
                let weight = match child.main_axis_size(dir) {
                    Size::Fill(w) => w,
                    _ => 1.0,
                };
                let fill_size = if fill_weight_total > 0.0 {
                    remaining * (weight / fill_weight_total)
                } else {
                    0.0
                };
                child_main_sizes[i] = Some(fill_size);
            }
        }

        // Step 3: position children along main axis with cross-axis alignment
        let mut main_cursor = main_axis(rect.position, dir);

        for (i, child) in self.children.iter().enumerate() {
            let child_main = child_main_sizes[i].unwrap_or(0.0);
            let measured_cross = child_cross_sizes[i];

            let (child_cross_pos, child_cross_size) = match child.align {
                Align::Start => (cross_axis(rect.position, dir), measured_cross),
                Align::Center => {
                    let offset = (inner_cross - measured_cross) * 0.5;
                    (
                        cross_axis(rect.position, dir) + offset.max(0.0),
                        measured_cross,
                    )
                }
                Align::End => {
                    let offset = inner_cross - measured_cross;
                    (
                        cross_axis(rect.position, dir) + offset.max(0.0),
                        measured_cross,
                    )
                }
                Align::Stretch => (cross_axis(rect.position, dir), inner_cross),
            };

            let child_pos = make_vec(main_cursor, child_cross_pos, dir);
            let child_size = make_vec(child_main, child_cross_size, dir);
            let child_rect = Rect::new(child_pos, child_size);

            ctx.set_id(child.id);
            ctx.register_hit_rect(child_rect, child.id);
            child.render_layout(ctx, child_rect);

            main_cursor += child_main;
        }
    }
}

pub struct UiBuilder<'a, T = Box<dyn UiElement>> {
    node: &'a mut StrobeNode<T>,
    pub style: Style,
    size: Vec2,
    current_id: u32,
    input_state: Option<&'a StrobeInputState>,
}

impl<'a, T> UiBuilder<'a, T> {
    pub fn new(node: &'a mut StrobeNode<T>, size: Vec2) -> Self {
        Self {
            node,
            style: Style::default(),
            size,
            current_id: 0,
            input_state: None,
        }
    }

    pub fn new_with_input(
        node: &'a mut StrobeNode<T>,
        size: Vec2,
        input_state: &'a StrobeInputState,
    ) -> Self {
        Self {
            node,
            style: Style::default(),
            size,
            current_id: 0,
            input_state: Some(input_state),
        }
    }

    pub fn interaction(&self, id: u32) -> UiInteraction {
        self.input_state
            .map(|s| s.interaction(id))
            .unwrap_or_default()
    }

    pub fn is_hovered(&self, id: u32) -> bool {
        self.interaction(id).hovered
    }

    pub fn is_pressed(&self, id: u32) -> bool {
        self.interaction(id).pressed
    }

    pub fn was_clicked(&self, id: u32) -> bool {
        self.interaction(id).clicked
    }

    pub fn drag_delta(&self, id: u32) -> Vec2 {
        self.interaction(id).drag_delta
    }

    pub fn vertical(&mut self, f: impl FnOnce(&mut UiBuilder<T>)) {
        let mut node = StrobeNode::new(LayoutDirection::Vertical);
        node.id = self.current_id;
        node.padding = self.style.padding;
        node.width = self.style.width;
        node.height = self.style.height;
        node.align = self.style.align;

        self.current_id += 1;

        let mut builder = self.enter(&mut node);
        f(&mut builder);
        self.current_id = builder.current_id;

        self.node.children.push(node);
    }

    pub fn horizontal(&mut self, f: impl FnOnce(&mut UiBuilder<T>)) {
        let mut node = StrobeNode::new(LayoutDirection::Horizontal);
        node.id = self.current_id;
        node.padding = self.style.padding;
        node.width = self.style.width;
        node.height = self.style.height;
        node.align = self.style.align;

        self.current_id += 1;

        let mut builder = self.enter(&mut node);
        f(&mut builder);
        self.current_id = builder.current_id;

        self.node.children.push(node);
    }

    pub fn stack(&mut self, f: impl FnOnce(&mut UiBuilder<T>)) {
        let mut node = StrobeNode::new(LayoutDirection::Stack);
        node.id = self.current_id;
        node.padding = self.style.padding;
        node.width = self.style.width;
        node.height = self.style.height;
        node.align = self.style.align;

        self.current_id += 1;

        let mut builder = self.enter(&mut node);
        f(&mut builder);
        self.current_id = builder.current_id;

        self.node.children.push(node);
    }

    pub fn add(&mut self, element: T) -> u32 {
        let id = self.current_id;
        let mut node = StrobeNode::leaf(element);
        node.id = id;
        node.padding = self.style.padding;
        node.width = self.style.width;
        node.height = self.style.height;
        node.align = self.style.align;

        self.current_id += 1;

        self.node.children.push(node);
        id
    }

    pub fn window_size(&self) -> Vec2 {
        self.size
    }

    fn enter<'b>(&self, node: &'b mut StrobeNode<T>) -> UiBuilder<'b, T>
    where
        'a: 'b,
    {
        UiBuilder {
            node,
            style: Style::default(),
            size: self.size,
            current_id: self.current_id,
            input_state: self.input_state,
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

    #[test]
    fn nested_builder_ids_are_unique() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root: StrobeNode<MockElement> = StrobeNode::default();
        let mut builder = UiBuilder::new(&mut root, Vec2::ZERO);

        builder.vertical(|ui| {
            for _ in 0..6 {
                ui.horizontal(|ui| {
                    ui.add(MockElement::new(10.0, 10.0, log.clone()));
                    ui.add(MockElement::new(2.0, 0.0, log.clone()));
                    ui.add(MockElement::new(10.0, 10.0, log.clone()));
                    ui.add(MockElement::new(2.0, 0.0, log.clone()));
                    ui.add(MockElement::new(10.0, 10.0, log.clone()));
                    ui.add(MockElement::new(2.0, 0.0, log.clone()));
                    ui.add(MockElement::new(10.0, 10.0, log.clone()));
                });
                ui.add(MockElement::new(0.0, 2.0, log.clone()));
            }
        });

        fn collect_child_ids<T>(node: &StrobeNode<T>, out: &mut Vec<u32>) {
            for child in &node.children {
                out.push(child.id);
                collect_child_ids(child, out);
            }
        }

        let mut ids = Vec::new();
        collect_child_ids(&root, &mut ids);
        let total = ids.len();

        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), total);
        assert!(total > 40);
    }

    #[test]
    fn test_vertical_fill() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root: StrobeNode<MockElement> = StrobeNode::default();
        let mut builder = UiBuilder::new(&mut root, Vec2::ZERO);

        // Add directly to root (which is a Vertical container)
        builder.add(MockElement::new(100.0, 60.0, log.clone()));

        builder.style.height = Size::Fill(1.0);
        builder.add(MockElement::new(100.0, 0.0, log.clone()));

        builder.style.height = Size::Auto;
        builder.add(MockElement::new(100.0, 60.0, log.clone()));

        let mut ctx = MockContext;
        let rect = Rect::new(Vec2::ZERO, Vec2::new(400.0, 500.0));
        root.render_layout(&mut ctx, rect);

        let calls = log.borrow();
        assert_eq!(calls.len(), 3);

        // Header: 60px
        assert_eq!(calls[0].position, Vec2::new(0.0, 0.0));
        assert_eq!(calls[0].size.y, 60.0);

        // Fill: 500 - 60 - 60 = 380px
        assert_eq!(calls[1].position, Vec2::new(0.0, 60.0));
        assert_eq!(calls[1].size.y, 380.0);

        // Footer: 60px
        assert_eq!(calls[2].position, Vec2::new(0.0, 440.0));
        assert_eq!(calls[2].size.y, 60.0);
    }

    #[test]
    fn test_horizontal_fill_weighted() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root: StrobeNode<MockElement> = StrobeNode::new(LayoutDirection::Horizontal);
        let mut builder = UiBuilder::new(&mut root, Vec2::ZERO);

        // Fixed 100px sidebar
        builder.add(MockElement::new(100.0, 50.0, log.clone()));

        // Fill(1.0)
        builder.style.width = Size::Fill(1.0);
        builder.add(MockElement::new(0.0, 50.0, log.clone()));

        // Fill(2.0)
        builder.style.width = Size::Fill(2.0);
        builder.add(MockElement::new(0.0, 50.0, log.clone()));

        let mut ctx = MockContext;
        let rect = Rect::new(Vec2::ZERO, Vec2::new(400.0, 100.0));
        root.render_layout(&mut ctx, rect);

        let calls = log.borrow();
        assert_eq!(calls.len(), 3);

        // Fixed: 100px
        assert_eq!(calls[0].position.x, 0.0);
        assert_eq!(calls[0].size.x, 100.0);

        // Fill(1): (400-100) * 1/3 = 100px
        assert_eq!(calls[1].position.x, 100.0);
        assert!((calls[1].size.x - 100.0).abs() < 0.01);

        // Fill(2): (400-100) * 2/3 = 200px
        assert!((calls[2].position.x - 200.0).abs() < 0.01);
        assert!((calls[2].size.x - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_cross_axis_alignment() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root: StrobeNode<MockElement> = StrobeNode::new(LayoutDirection::Horizontal);
        let mut builder = UiBuilder::new(&mut root, Vec2::ZERO);

        // Start (default)
        builder.add(MockElement::new(50.0, 20.0, log.clone()));

        // Center
        builder.style.align = Align::Center;
        builder.add(MockElement::new(50.0, 20.0, log.clone()));

        // End
        builder.style.align = Align::End;
        builder.add(MockElement::new(50.0, 20.0, log.clone()));

        // Stretch
        builder.style.align = Align::Stretch;
        builder.add(MockElement::new(50.0, 20.0, log.clone()));

        let mut ctx = MockContext;
        let rect = Rect::new(Vec2::ZERO, Vec2::new(400.0, 100.0));
        root.render_layout(&mut ctx, rect);

        let calls = log.borrow();
        assert_eq!(calls.len(), 4);

        // Start: y=0, height=20
        assert_eq!(calls[0].position.y, 0.0);
        assert_eq!(calls[0].size.y, 20.0);

        // Center: y=(100-20)/2=40, height=20
        assert_eq!(calls[1].position.y, 40.0);
        assert_eq!(calls[1].size.y, 20.0);

        // End: y=100-20=80, height=20
        assert_eq!(calls[2].position.y, 80.0);
        assert_eq!(calls[2].size.y, 20.0);

        // Stretch: y=0, height=100
        assert_eq!(calls[3].position.y, 0.0);
        assert_eq!(calls[3].size.y, 100.0);
    }
}
