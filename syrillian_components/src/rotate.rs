use syrillian::math::{Quat, Vec3};

use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct RotateComponent {
    pub rotate_speed: f32,
    pub iteration: f32,
    pub y_rot: f32,
    pub scale_coefficient: f32,
    default_scale: Vec3,
}

impl Default for RotateComponent {
    fn default() -> Self {
        RotateComponent {
            rotate_speed: 50.0f32,
            iteration: 0.0,
            y_rot: 0.0,
            scale_coefficient: 0.0,
            default_scale: Vec3::ONE,
        }
    }
}

impl Component for RotateComponent {
    fn init(&mut self, _world: &mut World) {
        self.default_scale = self.parent().transform.scale();
    }
    fn update(&mut self, world: &mut World) {
        let transform = &mut self.parent().transform;
        let delta_time = world.delta_time().as_secs_f32();

        let x_angle_radians = (self.iteration / 100.0).sin() * 45.0f32.to_radians();
        let x_rotation = Quat::from_axis_angle(Vec3::X, x_angle_radians);

        self.y_rot += self.rotate_speed.to_radians() * delta_time;
        let y_rotation = Quat::from_axis_angle(Vec3::Y, self.y_rot);

        let combined_rotation = y_rotation * x_rotation;

        transform.set_rotation(combined_rotation);
        if self.scale_coefficient > f32::EPSILON {
            transform.set_nonuniform_local_scale(
                self.default_scale * (self.iteration.sin() * self.scale_coefficient + 1.),
            );
        }
        self.iteration += delta_time;
    }
}
