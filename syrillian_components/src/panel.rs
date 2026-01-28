use crate::UiRect;
use crate::ui_rect::UiRectLayout;
use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;
use syrillian::core::GameObjectId;
use syrillian::math::Vec2;

/// Basic container for 2D UI elements.
#[derive(Debug, Reflect)]
#[reflect_all]
pub struct Panel {
    padding: Vec2,
}

impl Panel {
    pub fn set_padding(&mut self, padding: Vec2) {
        self.padding = padding;
    }
}

impl Default for Panel {
    fn default() -> Self {
        Panel {
            padding: Vec2::new(5.0, 5.0),
        }
    }
}

impl Component for Panel {
    fn update(&mut self, world: &mut World) {
        let Some(mut rect) = self.parent().get_component::<UiRect>() else {
            return;
        };

        let Some(mut container_layout) = rect.layout(world) else {
            return;
        };

        container_layout.top_left_px += self.padding;
        container_layout.size_px -= self.padding * 2.0;

        rect.apply_to_components(world, &mut container_layout);

        layout_children(self.parent().children(), &container_layout, world);
    }
}

fn layout_children(children: &[GameObjectId], parent_layout: &UiRectLayout, world: &mut World) {
    for &child in children {
        let rect = child.get_component::<UiRect>();
        let layout_from_rect = rect.as_ref().and_then(|rect| {
            rect.layout_in_region(
                parent_layout.top_left_px,
                parent_layout.size_px,
                parent_layout.screen,
            )
        });

        #[allow(clippy::unnecessary_lazy_evaluations)]
        let mut layout = layout_from_rect.unwrap_or_else(|| UiRectLayout {
            top_left_px: parent_layout.top_left_px,
            size_px: parent_layout.size_px,
            screen: parent_layout.screen,
            target: parent_layout.target,
            depth: parent_layout.depth,
            draw_order: parent_layout.draw_order,
        });

        layout.draw_order = parent_layout.draw_order;
        layout.depth = parent_layout.depth;

        if let Some(mut rect) = rect {
            rect.apply_to_components(world, &mut layout);
        }

        if !child.children().is_empty() {
            layout_children(child.children(), &layout, world);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ui_rect::UiSize;
    use crate::{Image, Panel, Text2D, UiRect};
    use more_asserts::assert_lt;
    use syrillian::components::Component;
    use syrillian::math::Vec2;
    use syrillian::strobe::ImageScalingMode;
    use syrillian::{PhysicalSize, ViewportId, World};

    fn world_with_viewport() -> Box<World> {
        let (mut world, ..) = World::fresh();
        world.set_viewport_size(ViewportId::PRIMARY, PhysicalSize::new(800, 600));
        world
    }

    #[test]
    fn panel_lays_out_children_with_depth_bias() {
        let mut world = world_with_viewport();

        let mut panel = world.new_object("panel");
        world.add_child(panel);

        let mut panel_rect = panel.add_component::<UiRect>();
        panel_rect.set_offset(Vec2::new(5.0, 10.0));
        panel_rect.set_size(UiSize::Pixels {
            width: 200.0,
            height: 100.0,
        });
        panel_rect.set_depth(0.2);

        let panel_image = panel.add_component::<Image>();
        let panel_text = panel.add_component::<Text2D>();
        let mut panel_comp = panel.add_component::<Panel>();

        let mut child = world.new_object("child");
        let mut child_rect = child.add_component::<UiRect>();
        child_rect.set_anchor(Vec2::new(0.5, 0.5));
        child_rect.set_pivot(Vec2::new(0.5, 0.5));
        child_rect.set_size(UiSize::Percent {
            width: 0.5,
            height: 0.5,
        });
        child_rect.set_offset(Vec2::new(10.0, -5.0));
        let child_image = child.add_component::<Image>();

        let mut grandchild = world.new_object("grandchild");
        let _grandchild_rect = grandchild.add_component::<UiRect>();
        let grandchild_image = grandchild.add_component::<Image>();

        panel.add_child(child);
        child.add_child(grandchild);

        panel_comp.update(&mut world);

        assert_eq!(panel_image.draw_order(), 0);
        assert_eq!(panel_text.draw_order(), 1);

        match child_image.scaling_mode() {
            ImageScalingMode::Absolute {
                left,
                right,
                top,
                bottom,
            } => assert_eq!((left, right, top, bottom), (67.0, 163.0, 568.0, 522.0)),
            other => panic!("expected absolute scaling for child, got {other:?}"),
        }
        assert_eq!(child_image.draw_order(), 2);
        assert_lt!(
            (child_image.translation().w_axis.z - 0.200).abs(),
            f32::EPSILON
        );

        match grandchild_image.scaling_mode() {
            ImageScalingMode::Absolute {
                left,
                right,
                top,
                bottom,
            } => assert_eq!((left, right, top, bottom), (67.0, 168.0, 568.0, 467.0)),
            other => panic!("expected absolute scaling for grandchild, got {other:?}"),
        }
        assert_eq!(grandchild_image.draw_order(), 3);
        assert_lt!(
            (grandchild_image.translation().w_axis.z - 0.200).abs(),
            1e-6
        );
    }
}
