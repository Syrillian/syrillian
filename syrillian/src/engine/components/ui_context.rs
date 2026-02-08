use crate::World;
use crate::components::TypedComponentId;
use crate::core::ObjectHash;
use glamx::vec2;
use std::hash::{DefaultHasher, Hash, Hasher};
use syrillian_render::rendering::viewport::ViewportId;
use syrillian_render::strobe::{CacheId, StrobeNode, StrobeRoot, UiBuilder};

pub struct UiContext {
    current_id: CacheId,
}

impl UiContext {
    pub fn new(object_hash: ObjectHash, component_id: TypedComponentId) -> UiContext {
        let mut hasher = DefaultHasher::default();
        object_hash.hash(&mut hasher);
        component_id.type_id().hash(&mut hasher);
        component_id.component_id().hash(&mut hasher);

        UiContext {
            current_id: hasher.finish(),
        }
    }

    pub fn draw(
        &self,
        world: &mut World,
        target: ViewportId,
        ui: impl FnOnce(&mut UiBuilder),
    ) -> bool {
        let Some(size) = world.viewport_size(target) else {
            return false;
        };

        let mut root = StrobeNode::default();
        let mut builder = UiBuilder::new(&mut root, vec2(size.width as f32, size.height as f32));
        ui(&mut builder);

        if root.children.is_empty() && root.element.is_none() {
            return true;
        }

        world.strobe.strobe_roots.push(StrobeRoot {
            root,
            target,
            cache_id: self.current_id,
        });

        true
    }
}
