use crate::World;
use crate::components::{Component, Image, Text2D};
use crate::rendering::strobe::ImageScalingMode;
use crate::windowing::RenderTargetId;
use nalgebra::Vector2;
use syrillian_macros::Reflect;

#[derive(Debug, Clone)]
pub struct UiRectLayout {
    pub top_left_px: Vector2<f32>,
    pub size_px: Vector2<f32>,
    pub screen: Vector2<f32>,
    pub target: RenderTargetId,
    pub depth: f32,
    pub draw_order: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum UiSize {
    Pixels { width: f32, height: f32 },
    Percent { width: f32, height: f32 },
}

impl UiSize {
    pub fn resolve(&self, screen: Vector2<f32>) -> Vector2<f32> {
        match *self {
            UiSize::Pixels { width, height } => Vector2::new(width.max(0.0), height.max(0.0)),
            UiSize::Percent { width, height } => {
                Vector2::new((width * screen.x).max(0.0), (height * screen.y).max(0.0))
            }
        }
    }
}

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct UiRect {
    anchor: Vector2<f32>,
    pivot: Vector2<f32>,
    offset: Vector2<f32>,
    size: UiSize,
    pub depth: f32,
    #[dont_reflect]
    render_target: RenderTargetId,
}

impl UiRect {
    pub fn anchor(&self) -> Vector2<f32> {
        self.anchor
    }

    pub fn set_anchor(&mut self, anchor: Vector2<f32>) {
        self.anchor = anchor;
    }

    pub fn pivot(&self) -> Vector2<f32> {
        self.pivot
    }

    pub fn set_pivot(&mut self, pivot: Vector2<f32>) {
        self.pivot = pivot;
    }

    pub fn offset(&self) -> Vector2<f32> {
        self.offset
    }

    pub fn set_offset(&mut self, offset: Vector2<f32>) {
        self.offset = offset;
    }

    pub fn size(&self) -> UiSize {
        self.size
    }

    pub fn set_size(&mut self, size: UiSize) {
        self.size = size;
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth;
    }

    pub fn render_target(&self) -> RenderTargetId {
        self.render_target
    }

    pub fn set_render_target(&mut self, target: RenderTargetId) {
        self.render_target = target;
    }

    pub fn layout(&self, world: &World) -> Option<UiRectLayout> {
        let screen = world.viewport_size(self.render_target)?;
        let screen_vec = Vector2::new(screen.width as f32, screen.height as f32);
        self.layout_in_region(Vector2::zeros(), screen_vec, screen_vec)
    }

    pub fn layout_in_region(
        &self,
        parent_origin: Vector2<f32>,
        parent_size: Vector2<f32>,
        screen: Vector2<f32>,
    ) -> Option<UiRectLayout> {
        let size_px = self.size.resolve(parent_size);
        let anchor_px = Vector2::new(self.anchor.x * parent_size.x, self.anchor.y * parent_size.y);
        let pivot_offset = Vector2::new(self.pivot.x * size_px.x, self.pivot.y * size_px.y);
        let top_left_px = parent_origin + anchor_px + self.offset - pivot_offset;

        Some(UiRectLayout {
            top_left_px,
            size_px,
            screen,
            target: self.render_target,
            depth: self.depth,
            draw_order: 0,
        })
    }

    pub fn apply_to_components(&mut self, _world: &mut World, layout: &mut UiRectLayout) {
        for component in self.parent().iter_dyn_components() {
            if let Some(mut image) = component.as_a::<Image>() {
                let screen_h = layout.screen.y.max(1.0);

                let left = layout.top_left_px.x.max(0.0).floor();
                let right = (layout.top_left_px.x + layout.size_px.x).max(0.0).ceil();

                let bottom = (screen_h - (layout.top_left_px.y + layout.size_px.y))
                    .max(0.0)
                    .floor();
                let top = (screen_h - layout.top_left_px.y).max(0.0).ceil();

                if top > bottom && right > left {
                    image.set_scaling_mode(ImageScalingMode::Absolute {
                        left,
                        right,
                        top,
                        bottom,
                    });
                }

                image.set_draw_order(layout.draw_order);

                let translation =
                    nalgebra::Translation3::new(0.0, 0.0, layout.depth).to_homogeneous();
                image.set_translation(translation);

                layout.draw_order += 1;
            } else if let Some(mut text) = component.as_a::<Text2D>() {
                text.set_position_vec(layout.top_left_px);
                text.set_draw_order(layout.draw_order);
                text.set_render_target(layout.target);

                layout.draw_order += 1;
            }
        }
    }
}

impl Default for UiRect {
    fn default() -> Self {
        Self {
            anchor: Vector2::new(0.0, 0.0),
            pivot: Vector2::new(0.0, 0.0),
            offset: Vector2::zeros(),
            size: UiSize::Pixels {
                width: 100.0,
                height: 100.0,
            },
            depth: 0.5,
            render_target: RenderTargetId::PRIMARY,
        }
    }
}

impl Component for UiRect {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windowing::RenderTargetId;
    use nalgebra::{Translation3, Vector2};
    use winit::dpi::PhysicalSize;

    fn world_with_viewport() -> Box<World> {
        let (mut world, ..) = World::fresh();
        world.set_viewport_size(RenderTargetId::PRIMARY, PhysicalSize::new(800, 600));
        world
    }

    #[test]
    fn layout_in_region_resolves_anchor_and_pivot() {
        let mut rect = UiRect::default();
        rect.set_anchor(Vector2::new(0.5, 0.5));
        rect.set_pivot(Vector2::new(1.0, 1.0));
        rect.set_offset(Vector2::new(10.0, -5.0));
        rect.set_size(UiSize::Percent {
            width: 0.25,
            height: 0.5,
        });

        let layout = rect
            .layout_in_region(
                Vector2::new(20.0, 30.0),
                Vector2::new(400.0, 200.0),
                Vector2::new(800.0, 600.0),
            )
            .expect("layout should be produced");

        assert_eq!(layout.size_px, Vector2::new(100.0, 100.0));
        assert_eq!(layout.top_left_px, Vector2::new(130.0, 25.0));
        assert_eq!(layout.screen, Vector2::new(800.0, 600.0));
        assert_eq!(layout.target, RenderTargetId::PRIMARY);
        assert_eq!(layout.depth, rect.depth);
        assert_eq!(layout.draw_order, 0);
    }

    #[test]
    fn apply_to_components_sets_scaling_and_draw_order() {
        let mut world = world_with_viewport();
        let mut obj = world.new_object("ui");
        world.add_child(obj);

        let mut rect = obj.add_component::<UiRect>();
        rect.set_offset(Vector2::new(12.0, 18.0));
        rect.set_size(UiSize::Pixels {
            width: 150.0,
            height: 75.0,
        });
        rect.set_depth(0.25);

        let image = obj.add_component::<Image>();
        let text = obj.add_component::<Text2D>();
        let image2 = obj.add_component::<Image>();

        let mut layout = rect.layout(&world).expect("viewport configured");

        rect.apply_to_components(&mut world, &mut layout);
        assert_eq!(layout.draw_order, 3);

        match image.scaling_mode() {
            ImageScalingMode::Absolute {
                left,
                right,
                top,
                bottom,
            } => {
                assert_eq!((left, right, top, bottom), (12.0, 162.0, 582.0, 507.0));
            }
            _ => panic!("expected absolute scaling"),
        }
        assert_eq!(image.draw_order(), 0);
        assert_eq!(text.draw_order(), 1);
        assert_eq!(image2.draw_order(), 2);
        assert_eq!(
            image.translation(),
            Translation3::new(0.0, 0.0, 0.25).to_homogeneous()
        );
    }

    #[test]
    fn apply_to_components_keeps_scaling_when_no_area() {
        let mut world = world_with_viewport();
        let mut obj = world.new_object("ui");
        world.add_child(obj);

        let mut rect = obj.add_component::<UiRect>();
        let mut image = obj.add_component::<Image>();

        image.set_scaling_mode(ImageScalingMode::RelativeStretch {
            left: 0.1,
            right: 0.9,
            top: 0.8,
            bottom: 0.2,
        });

        let mut layout = UiRectLayout {
            top_left_px: Vector2::new(10.0, 20.0),
            size_px: Vector2::zeros(),
            screen: Vector2::new(100.0, 100.0),
            target: RenderTargetId::PRIMARY,
            depth: 0.5,
            draw_order: 3,
        };

        let before = image.scaling_mode();
        rect.apply_to_components(&mut world, &mut layout);
        assert_eq!(layout.draw_order, 4);
        assert_eq!(image.scaling_mode(), before);
        assert_eq!(image.draw_order(), 3);
        assert!((image.translation()[(2, 3)] - 0.5).abs() < 1e-6);
    }
}
