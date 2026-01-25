use crate::Reflect;
use crate::World;
use crate::components::{
    CRef, CWeak, CameraComponent, Component, FirstPersonCameraController, RigidBodyComponent,
};
use crate::windowing::RenderTargetId;
use gilrs::Axis;
use nalgebra::Vector3;
use num_traits::Zero;
use rapier3d::prelude::{LockedAxes, QueryFilter, RigidBody, vector};
use tracing::warn;
use winit::keyboard::KeyCode;

#[derive(Debug, Reflect)]
pub struct FirstPersonMovementController {
    pub move_speed: f32,
    pub jump_factor: f32,
    rigid_body: CWeak<RigidBodyComponent>,
    camera_controller: CWeak<FirstPersonCameraController>,
    pub velocity: Vector3<f32>,
    pub sprint_multiplier: f32,
    velocity_interp_t: f32,
    is_grounded: bool,
    pub air_control: f32,
    feet_height: f32,
}

impl Default for FirstPersonMovementController {
    fn default() -> Self {
        FirstPersonMovementController {
            move_speed: 5.0,
            jump_factor: 100.0,
            rigid_body: CWeak::null(),
            camera_controller: CWeak::null(),
            velocity: Vector3::zero(),
            sprint_multiplier: 2.0,
            velocity_interp_t: 6.0,
            is_grounded: false,
            air_control: 0.1,
            feet_height: -1.0,
        }
    }
}

impl Component for FirstPersonMovementController {
    fn init(&mut self, world: &mut World) {
        let mut rigid = self.parent().get_component::<RigidBodyComponent>();
        if let Some(rigid) = &mut rigid
            && let Some(rigid) = rigid.body_mut()
        {
            rigid.set_locked_axes(LockedAxes::ROTATION_LOCKED, false);
            rigid.enable_ccd(true);
        }
        self.rigid_body = rigid.map(CRef::downgrade).unwrap_or_default();

        self.camera_controller = self
            .parent()
            .get_child_component::<FirstPersonCameraController>()
            .map(CRef::downgrade)
            .unwrap_or_default();

        self.update_grounded(world);
    }

    fn update(&mut self, world: &mut World) {
        let was_grounded = self.is_grounded;
        self.update_grounded(world);

        let target = self
            .parent()
            .get_component::<CameraComponent>()
            .map(|c| c.render_target())
            .unwrap_or(RenderTargetId::PRIMARY);

        world.input.set_active_target(target);

        let mut rigid = match self.rigid_body.upgrade(world) {
            None => {
                warn!("Rigid body not set!");
                return;
            }
            Some(rigid) => rigid,
        };

        let body = match rigid.body_mut() {
            None => {
                warn!("Rigid body not in set");
                return;
            }
            Some(rigid) => rigid,
        };

        if !world.input.is_window_focused() || !world.input.is_cursor_locked() {
            return;
        }

        let jumping = world.input.is_jump_down();
        if jumping && self.is_grounded {
            body.apply_impulse(vector![0.0, 0.2 * self.jump_factor, 0.0], true);
        }

        let (lr_movement, fb_movement, speed_factor, max_speed) =
            self.recalculate_velocity(world, body);

        if let Some(mut camera) = self.camera_controller.upgrade(world) {
            if was_grounded && !self.is_grounded {
                camera.signal_jump();
            } else if !was_grounded && self.is_grounded {
                camera.signal_ground()
            }
            let delta_time = world.delta_time().as_secs_f32();
            camera.update_roll(
                -lr_movement * speed_factor * delta_time * 100.,
                4. - fb_movement.abs() * 2.,
            );
            let speed_fraction = (self.velocity.magnitude() / max_speed).clamp(0.0, 2.0);
            let sprinting = world.input.is_sprinting();
            camera.apply_movement_state(speed_fraction, sprinting);
            if self.is_grounded {
                camera.update_bob(body.linvel().magnitude(), sprinting, delta_time);
            }
            camera.vel = *body.linvel();
            if jumping {
                camera.signal_jump();
            }
        }

        let mut linvel = *body.linvel();
        linvel.x = self.velocity.x;
        linvel.z = self.velocity.z;

        body.set_linvel(linvel, true);
    }
}

impl FirstPersonMovementController {
    pub fn update_grounded(&mut self, world: &mut World) {
        let Some(rigid_body) = self.rigid_body.upgrade(world) else {
            return;
        };

        let Some(body) = rigid_body.body() else {
            return;
        };

        let mut position = *body.position();
        position.translation.y += self.feet_height + 0.05;
        const DIR: Vector3<f32> = Vector3::new(0.0, -1.0, 0.0);
        let filter = QueryFilter::new().exclude_rigid_body(rigid_body.handle());

        self.is_grounded = world
            .physics
            .cast_sphere(0.25, 0.15, &position, &DIR, filter)
            .is_some();
    }

    pub fn recalculate_velocity(
        &mut self,
        world: &World,
        body: &RigidBody,
    ) -> (f32, f32, f32, f32) {
        let parent = self.parent();
        let mut speed_factor = self.move_speed;

        if world.input.is_sprinting() {
            speed_factor *= self.sprint_multiplier;
        }

        let mut target_velocity = Vector3::zero();

        let mut fb_movement: f32 = 0.;
        if world.input.is_key_pressed(KeyCode::KeyW) {
            target_velocity += parent.transform.forward();
            fb_movement += 1.;
        }

        if world.input.is_key_pressed(KeyCode::KeyS) {
            target_velocity -= parent.transform.forward();
            fb_movement -= 1.;
        }

        let mut lr_movement: f32 = 0.;
        if world.input.is_key_pressed(KeyCode::KeyA) {
            target_velocity -= parent.transform.right();
            lr_movement -= 1.;
        }

        if world.input.is_key_pressed(KeyCode::KeyD) {
            target_velocity += parent.transform.right();
            lr_movement += 1.;
        }

        let axis_x = world.input.gamepad.axis(Axis::LeftStickX);
        let axis_y = world.input.gamepad.axis(Axis::LeftStickY);
        if fb_movement.abs() < f32::EPSILON {
            target_velocity += parent.transform.forward() * axis_y;
            fb_movement = axis_y;
        }
        if lr_movement.abs() < f32::EPSILON {
            target_velocity += parent.transform.right() * axis_x;
            lr_movement = axis_x;
        }

        let max_speed = speed_factor;
        if target_velocity.magnitude() > 0.5 {
            target_velocity = target_velocity.normalize();
        }
        target_velocity *= max_speed;

        let mut interp_speed = self.velocity_interp_t * world.delta_time().as_secs_f32();
        if !self.is_grounded {
            interp_speed *= self.air_control;
        }
        if self.is_grounded || body.linvel().xz().norm() > 0.05 {
            self.velocity = self.velocity.lerp(&target_velocity, interp_speed);
        } else {
            self.velocity.x = body.linvel().x;
            self.velocity.z = body.linvel().z;
        }

        (lr_movement, fb_movement, speed_factor, max_speed)
    }
}
