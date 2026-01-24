use nalgebra::Vector3;
use crate::Reflect;
use crate::World;
use crate::components::{Component, NewComponent};
use crate::core::GameObjectId;

#[derive(Debug, Reflect)]
pub struct GravityComponent {
    pub acceleration_per_sec: f32,
    pub velocity: f32,
    pub max_acceleration: f32,
    parent: GameObjectId,
}

impl NewComponent for GravityComponent {
    fn new(parent: GameObjectId) -> Self {
        GravityComponent {
            acceleration_per_sec: 9.80665,
            velocity: 0.0,
            max_acceleration: 100.0,
            parent,
        }
    }
}

impl Component for GravityComponent {
    fn update(&mut self, world: &mut World) {
        let delta_time = world.delta_time().as_secs_f32();

        self.velocity = (self.velocity - self.acceleration_per_sec * delta_time)
            .clamp(-self.max_acceleration, self.max_acceleration);
        let transform = &mut self.parent.transform;
        transform.translate(Vector3::new(0.0, self.velocity, 0.0));
    }
}
