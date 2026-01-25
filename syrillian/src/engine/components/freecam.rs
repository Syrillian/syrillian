use crate::Reflect;
use crate::World;
use crate::components::{CameraComponent, Component};
use crate::input::InputManager;
use crate::windowing::RenderTargetId;
use gilrs::{Axis, Button};
use nalgebra::{UnitQuaternion, Vector2, Vector3};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct FreecamController {
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for FreecamController {
    fn default() -> Self {
        FreecamController {
            move_speed: 12.0f32,
            look_sensitivity: 0.12f32,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

impl Component for FreecamController {
    fn update(&mut self, world: &mut World) {
        let target = self
            .parent()
            .get_component::<CameraComponent>()
            .map(|c| c.render_target())
            .unwrap_or(RenderTargetId::PRIMARY);
        world.input.set_active_target(target);

        if !world.input.is_window_focused() || !world.input.is_button_pressed(MouseButton::Left) {
            return;
        }

        let delta_time = world.delta_time().as_secs_f32();
        let input = &world.input;

        if world.input.is_button_pressed(MouseButton::Left) {
            self.update_view(input);
        }

        self.update_movement(delta_time, input);
    }
}

impl FreecamController {
    fn update_movement(&mut self, delta_time: f32, input: &InputManager) {
        let transform = &mut self.parent().transform;
        let mut fb_movement: f32 = 0.;
        if input.is_key_pressed(KeyCode::KeyW) {
            fb_movement += 1.;
        }

        if input.is_key_pressed(KeyCode::KeyS) {
            fb_movement -= 1.;
        }

        let mut lr_movement: f32 = 0.;
        if input.is_key_pressed(KeyCode::KeyA) {
            lr_movement -= 1.;
        }

        if input.is_key_pressed(KeyCode::KeyD) {
            lr_movement += 1.;
        }

        let mut ud_movement: f32 = 0.;
        if input.is_key_pressed(KeyCode::Space) {
            ud_movement = 1.0;
        }
        if input.is_key_pressed(KeyCode::ControlLeft) {
            ud_movement = -1.0;
        }

        let axis_x = input.gamepad.axis(Axis::LeftStickX);
        let axis_y = input.gamepad.axis(Axis::LeftStickY);
        let axis_z = input.gamepad.button(Button::RightTrigger2);
        if lr_movement.abs() < f32::EPSILON {
            lr_movement = axis_x;
        }
        if fb_movement.abs() < f32::EPSILON {
            fb_movement = axis_y;
        }
        if ud_movement.abs() < f32::EPSILON {
            let invert = input.gamepad.is_button_pressed(Button::East);
            if invert {
                ud_movement = -axis_z;
            } else {
                ud_movement = axis_z
            }
        }

        let mut direction = transform.right() * lr_movement
            + transform.up() * ud_movement
            + transform.forward() * fb_movement;

        let move_speed = if input.is_key_pressed(KeyCode::ShiftLeft) {
            self.move_speed * 3.0
        } else {
            let controller_extra_speed =
                input.gamepad.button(Button::LeftTrigger2) + (1. / 10.0) / 10.0;
            self.move_speed * (controller_extra_speed * 5.)
        };

        if direction.magnitude() > f32::EPSILON {
            direction.normalize_mut();
            transform.translate(direction * move_speed * delta_time);
        }
    }

    fn update_view(&mut self, input: &InputManager) {
        let transform = &mut self.parent().transform;

        let gamepad_delta = Vector2::new(
            -input.gamepad.axis(Axis::RightStickX),
            input.gamepad.axis(Axis::RightStickY),
        );
        let mut delta = input.mouse_delta() + gamepad_delta * 80.0;
        delta *= self.look_sensitivity;
        self.yaw += delta.x;
        self.pitch += delta.y;

        self.pitch = self.pitch.clamp(-89.0f32, 89.0f32);

        let yaw_rotation =
            UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.yaw.to_radians());
        let pitch_rotation =
            UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.pitch.to_radians());
        let rotation = yaw_rotation * pitch_rotation;

        transform.set_local_rotation(rotation);
    }
}
