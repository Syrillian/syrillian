use crate::strobe::ui_element::Rect;
use glamx::Vec2;
use std::collections::HashMap;

pub type UiNodeId = u32;

#[derive(Debug, Clone, Copy, Default)]
pub struct UiInteraction {
    pub hovered: bool,
    pub pressed: bool,
    pub clicked: bool,
    pub drag_delta: Vec2,
}

#[derive(Debug, Clone)]
pub struct HitRect {
    pub rect: Rect,
    pub node_id: UiNodeId,
}

#[derive(Default)]
pub struct StrobeInputState {
    hit_rects: Vec<HitRect>,
    interactions: HashMap<UiNodeId, UiInteraction>,
    active_node: Option<UiNodeId>,
    press_origin: Option<Vec2>,
    last_mouse_pos: Vec2,
}

impl StrobeInputState {
    pub fn update_hit_rects(&mut self, rects: Vec<HitRect>) {
        self.hit_rects = rects;
    }

    pub fn begin_frame(
        &mut self,
        mouse_pos: Vec2,
        mouse_down: bool,
        just_pressed: bool,
        just_released: bool,
    ) {
        self.interactions.clear();

        // Hit test back-to-front (last = topmost)
        let hovered_node = self
            .hit_rects
            .iter()
            .rev()
            .find(|hr| hr.rect.contains(mouse_pos))
            .map(|hr| hr.node_id);

        // Handle press start
        if just_pressed {
            self.active_node = hovered_node;
            self.press_origin = Some(mouse_pos);
            self.last_mouse_pos = mouse_pos;
        }

        // Compute drag delta
        let drag_delta = if self.active_node.is_some() && mouse_down {
            mouse_pos - self.last_mouse_pos
        } else {
            Vec2::ZERO
        };

        // Handle release
        let clicked_node = if just_released {
            let clicked = if self.active_node == hovered_node {
                hovered_node
            } else {
                None
            };
            self.active_node = None;
            self.press_origin = None;
            clicked
        } else {
            None
        };

        // Build interaction entries
        if let Some(id) = hovered_node {
            let entry = self.interactions.entry(id).or_default();
            entry.hovered = true;
        }
        if let Some(id) = self.active_node {
            let entry = self.interactions.entry(id).or_default();
            entry.pressed = true;
            entry.drag_delta = drag_delta;
        }
        if let Some(id) = clicked_node {
            let entry = self.interactions.entry(id).or_default();
            entry.clicked = true;
        }

        self.last_mouse_pos = mouse_pos;
    }

    pub fn interaction(&self, node_id: UiNodeId) -> UiInteraction {
        self.interactions.get(&node_id).copied().unwrap_or_default()
    }

    pub fn active_node(&self) -> Option<UiNodeId> {
        self.active_node
    }
}
