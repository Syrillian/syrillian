use crate::Reflect;
use crate::World;
use crate::components::Component;
use crate::core::EventType;

type ButtonClickHandler = Box<dyn FnMut(&mut World) + 'static>;

#[derive(Default, Reflect)]
pub struct Button {
    click_handler: Vec<ButtonClickHandler>,
}

impl Component for Button {
    fn init(&mut self, world: &mut World) {
        self.parent().notify_for(world, EventType::CLICK);
    }

    fn on_click(&mut self, world: &mut World) {
        for handler in &mut self.click_handler {
            handler(world);
        }
    }
}

impl Button {
    pub fn add_click_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&mut World) + 'static,
    {
        self.click_handler.push(Box::new(handler));
    }
}
