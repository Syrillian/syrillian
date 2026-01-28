use crate::{Image, Text2D};
use syrillian::World;
use syrillian::components::Component;
use syrillian::math::{Mat4, Vec2, Vec3};
use syrillian::rendering::strobe::ImageScalingMode;
use syrillian::{Reflect, ViewportId};

#[derive(Debug, Clone)]
pub struct UiRectLayout {
    pub top_left_px: Vec2,
    pub size_px: Vec2,
    pub screen: Vec2,
    pub target: ViewportId,
    pub depth: f32,
    pub draw_order: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum UiSize {
    Pixels { width: f32, height: f32 },
    Percent { width: f32, height: f32 },
}

impl UiSize {
    pub fn resolve(&self, screen: Vec2) -> Vec2 {
        match *self {
            UiSize::Pixels { width, height } => Vec2::new(width.max(0.0), height.max(0.0)),
            UiSize::Percent { width, height } => {
                Vec2::new((width * screen.x).max(0.0), (height * screen.y).max(0.0))
            }
        }
    }
}

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct UiRect {
    anchor: Vec2,
    pivot: Vec2,
    offset: Vec2,
    size: UiSize,
    pub depth: f32,
    #[dont_reflect]
    render_target: ViewportId,
}

impl UiRect {
    pub fn anchor(&self) -> Vec2 {
        self.anchor
    }

    pub fn set_anchor(&mut self, anchor: Vec2) {
        self.anchor = anchor;
    }

    pub fn pivot(&self) -> Vec2 {
        self.pivot
    }

    pub fn set_pivot(&mut self, pivot: Vec2) {
        self.pivot = pivot;
    }

    pub fn offset(&self) -> Vec2 {
        self.offset
    }

    pub fn set_offset(&mut self, offset: Vec2) {
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

    pub fn render_target(&self) -> ViewportId {
        self.render_target
    }

    pub fn set_render_target(&mut self, target: ViewportId) {
        self.render_target = target;
    }

    pub fn layout(&self, world: &World) -> Option<UiRectLayout> {
        let screen = world.viewport_size(self.render_target)?;
        let screen_vec = Vec2::new(screen.width as f32, screen.height as f32);
        self.layout_in_region(Vec2::ZERO, screen_vec, screen_vec)
    }

    pub fn layout_in_region(
        &self,
        parent_origin: Vec2,
        parent_size: Vec2,
        screen: Vec2,
    ) -> Option<UiRectLayout> {
        let size_px = self.size.resolve(parent_size);
        let anchor_px = Vec2::new(self.anchor.x * parent_size.x, self.anchor.y * parent_size.y);
        let pivot_offset = Vec2::new(self.pivot.x * size_px.x, self.pivot.y * size_px.y);
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

                let translation = Mat4::from_translation(Vec3::new(0.0, 0.0, layout.depth));
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
            anchor: Vec2::ZERO,
            pivot: Vec2::ZERO,
            offset: Vec2::ZERO,
            size: UiSize::Pixels {
                width: 100.0,
                height: 100.0,
            },
            depth: 0.5,
            render_target: ViewportId::PRIMARY,
        }
    }
}

impl Component for UiRect {}

#[cfg(test)]
mod tests {
    use super::*;
    use syrillian::math::Vec2;
    use syrillian::windowing::PhysicalSize;

    fn world_with_viewport() -> Box<World> {
        let (mut world, ..) = World::fresh();
        world.set_viewport_size(ViewportId::PRIMARY, PhysicalSize::new(800, 600));
        world
    }

    #[test]
    fn layout_in_region_resolves_anchor_and_pivot() {
        let mut rect = UiRect::default();
        rect.set_anchor(Vec2::new(0.5, 0.5));
        rect.set_pivot(Vec2::new(1.0, 1.0));
        rect.set_offset(Vec2::new(10.0, -5.0));
        rect.set_size(UiSize::Percent {
            width: 0.25,
            height: 0.5,
        });

        let layout = rect
            .layout_in_region(
                Vec2::new(20.0, 30.0),
                Vec2::new(400.0, 200.0),
                Vec2::new(800.0, 600.0),
            )
            .expect("layout should be produced");

        assert_eq!(layout.size_px, Vec2::new(100.0, 100.0));
        assert_eq!(layout.top_left_px, Vec2::new(130.0, 25.0));
        assert_eq!(layout.screen, Vec2::new(800.0, 600.0));
        assert_eq!(layout.target, ViewportId::PRIMARY);
        assert_eq!(layout.depth, rect.depth);
        assert_eq!(layout.draw_order, 0);
    }

    #[test]
    fn apply_to_components_sets_scaling_and_draw_order() {
        let mut world = world_with_viewport();
        let mut obj = world.new_object("ui");
        world.add_child(obj);

        let mut rect = obj.add_component::<UiRect>();
        rect.set_offset(Vec2::new(12.0, 18.0));
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
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.25))
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
            top_left_px: Vec2::new(10.0, 20.0),
            size_px: Vec2::ZERO,
            screen: Vec2::new(100.0, 100.0),
            target: ViewportId::PRIMARY,
            depth: 0.5,
            draw_order: 3,
        };

        let before = image.scaling_mode();
        rect.apply_to_components(&mut world, &mut layout);
        assert_eq!(layout.draw_order, 4);
        assert_eq!(image.scaling_mode(), before);
        assert_eq!(image.draw_order(), 3);
        assert!((image.translation().w_axis.z - 0.5).abs() < f32::EPSILON);
    }
}
