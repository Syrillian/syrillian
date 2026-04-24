// TODO: Refactor & Merge First Person Controller

use crate::{Collider3D, RigidBodyComponent};
use syrillian::Reflect;
use syrillian::World;
use syrillian::components::{CRef, CWeak, CameraComponent, Component};
use syrillian::gilrs::Axis;
use syrillian::input::KeyCode;
use syrillian::math::{FloatExt, Pose, Quat, Vec2, Vec3};
use syrillian::physics::rapier3d::control::{
    CharacterAutostep, CharacterLength, KinematicCharacterController,
};
use syrillian::physics::rapier3d::dynamics::RigidBodyHandle;
use syrillian::physics::rapier3d::geometry::{ColliderHandle, SharedShape};
use syrillian::physics::rapier3d::parry::query::{
    DefaultQueryDispatcher, ShapeCastOptions, ShapeCastStatus,
};
use syrillian::physics::rapier3d::pipeline::QueryFilter;
use syrillian::tracing::warn;
use syrillian_render::rendering::viewport::ViewportId;

#[derive(Debug, Reflect)]
#[reflect_all]
#[reflect(default)]
pub struct ThirdPersonCharacterController {
    pub move_speed: f32,
    pub sprint_multiplier: f32,
    pub acceleration: f32,
    pub air_control: f32,
    pub jump_speed: f32,
    pub gravity: f32,
    pub ground_stick_force: f32,

    pub capsule_half_height: f32,
    pub capsule_radius: f32,
    pub controller_offset: f32,
    pub step_height: f32,
    pub step_min_width: f32,
    pub snap_to_ground: f32,
    pub max_slope_climb_angle: f32,
    pub min_slope_slide_angle: f32,
    pub normal_nudge_factor: f32,

    pub camera_height: f32,
    pub camera_distance: f32,
    pub camera_min_distance: f32,
    pub camera_max_distance: f32,
    pub camera_collision_radius: f32,
    pub camera_smoothing: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub mouse_sensitivity: Vec2,
    pub controller_sensitivity: Vec2,
    pub stick_deadzone: f32,
    pub turn_sharpness: f32,

    pub velocity: Vec3,
    pub is_grounded: bool,

    #[dont_reflect]
    rigid_body: CWeak<RigidBodyComponent>,
    #[dont_reflect]
    camera: CWeak<CameraComponent>,
    #[dont_reflect]
    self_collider_handle: Option<ColliderHandle>,
    #[dont_reflect]
    warned_missing_rigidbody: bool,
    #[dont_reflect]
    warned_missing_camera: bool,

    camera_yaw: f32,
    camera_pitch: f32,
    body_yaw: f32,
    smoothed_camera_distance: f32,
}

impl Default for ThirdPersonCharacterController {
    fn default() -> Self {
        Self {
            move_speed: 5.5,
            sprint_multiplier: 1.6,
            acceleration: 20.0,
            air_control: 0.35,
            jump_speed: 7.5,
            gravity: 20.0,
            ground_stick_force: 2.0,

            capsule_half_height: 0.9,
            capsule_radius: 0.3,
            controller_offset: 0.02,
            step_height: 0.45,
            step_min_width: 0.2,
            snap_to_ground: 0.3,
            max_slope_climb_angle: 50.0,
            min_slope_slide_angle: 55.0,
            normal_nudge_factor: 1.0e-4,

            camera_height: 1.45,
            camera_distance: 4.0,
            camera_min_distance: 1.2,
            camera_max_distance: 7.0,
            camera_collision_radius: 0.2,
            camera_smoothing: 15.0,
            min_pitch: -75.0,
            max_pitch: 65.0,
            mouse_sensitivity: Vec2::new(1.2, 1.2),
            controller_sensitivity: Vec2::new(1.0, 1.0),
            stick_deadzone: 0.15,
            turn_sharpness: 14.0,

            velocity: Vec3::ZERO,
            is_grounded: false,

            rigid_body: CWeak::null(),
            camera: CWeak::null(),
            self_collider_handle: None,
            warned_missing_rigidbody: false,
            warned_missing_camera: false,

            camera_yaw: 0.0,
            camera_pitch: -12.0,
            body_yaw: 0.0,
            smoothed_camera_distance: 4.0,
        }
    }
}

impl Component for ThirdPersonCharacterController {
    fn init(&mut self, world: &mut World) {
        let mut rigid = self.parent().get_component::<RigidBodyComponent>();
        if let Some(rigid) = &mut rigid {
            rigid.set_kinematic(true);

            if let Some(body) = rigid.body_mut() {
                body.enable_ccd(true);
                body.set_linvel(Vec3::ZERO, true);
                body.set_angvel(Vec3::ZERO, true);
            }
        }
        self.rigid_body = rigid.map(CRef::downgrade).unwrap_or_default();

        let mut collider = self.parent().get_component::<Collider3D>();
        self.self_collider_handle = collider.as_ref().and_then(|c| c.phys_handle);
        if let Some(collider) = &mut collider
            && let Some(collider) = collider.collider_mut()
        {
            collider.set_shape(self.capsule_shape());
        }

        self.camera = self
            .parent()
            .get_child_component::<CameraComponent>()
            .map(CRef::downgrade)
            .unwrap_or_default();

        self.sync_initial_angles();
        self.camera_pitch = self.camera_pitch.clamp(self.min_pitch, self.max_pitch);
        self.smoothed_camera_distance = self
            .camera_distance
            .clamp(self.camera_min_distance, self.camera_max_distance);

        if !self.rigid_body.exists(world) {
            self.warned_missing_rigidbody = true;
            warn!("ThirdPersonCharacterController requires RigidBodyComponent");
        }

        if !self.camera.exists(world) {
            self.warned_missing_camera = true;
            warn!("ThirdPersonCharacterController requires a child camera");
        }

        self.resolve_spawn_penetration(world);
        self.update_camera_transform(world, 1.0);
    }

    fn update(&mut self, world: &mut World) {
        world.input.set_active_target(self.render_target(world));

        if !world.input.is_window_focused() {
            return;
        }

        world.input.auto_cursor_lock();

        self.update_look(world);
    }

    fn fixed_update(&mut self, world: &mut World) {
        world.input.set_active_target(self.render_target(world));

        if !world.input.is_window_focused() {
            return;
        }

        let dt = world.physics.integration_parameters.dt.max(f32::EPSILON);

        let Some((rigid_handle, current_pose)) = self.current_pose(world) else {
            if !self.warned_missing_rigidbody {
                self.warned_missing_rigidbody = true;
                warn!("ThirdPersonCharacterController could not find a valid rigid body");
            }
            return;
        };

        let move_input = self.read_movement_input(world);
        let move_dir = self.camera_relative_move_dir(move_input);
        let sprint_factor = if world.input.is_sprinting() {
            self.sprint_multiplier
        } else {
            1.0
        };
        let target_horizontal = move_dir * (self.move_speed * sprint_factor);

        let control = if self.is_grounded {
            1.0
        } else {
            self.air_control.clamp(0.0, 1.0)
        };
        let accel_t = (self.acceleration * control * dt).clamp(0.0, 1.0);
        self.velocity.x = self.velocity.x.lerp(target_horizontal.x, accel_t);
        self.velocity.z = self.velocity.z.lerp(target_horizontal.z, accel_t);

        if self.is_grounded && self.velocity.y < 0.0 {
            self.velocity.y = -self.ground_stick_force;
        } else {
            self.velocity.y -= self.gravity * dt;
        }

        if self.is_grounded && world.input.is_jump_down() {
            self.velocity.y = self.jump_speed;
            self.is_grounded = false;
        }

        let desired_translation = self.velocity * dt;
        let shape = self.capsule_shape();
        let character_controller = self.build_character_controller();
        let filter = self.query_filter(Some(rigid_handle));

        let query_pipeline = world.physics.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &world.physics.rigid_body_set,
            &world.physics.collider_set,
            filter,
        );

        let movement = character_controller.move_shape(
            dt,
            &query_pipeline,
            shape.as_ref(),
            &current_pose,
            desired_translation,
            |_| {},
        );

        if move_dir.length_squared() > 1.0e-4 {
            let target_yaw = yaw_from_direction(move_dir);
            let turn_t = (self.turn_sharpness * dt).clamp(0.0, 1.0);
            self.body_yaw = move_towards_angle(self.body_yaw, target_yaw, turn_t);
        }

        let body_rotation = Quat::from_axis_angle(Vec3::Y, self.body_yaw.to_radians());

        let mut next_pose = current_pose;
        next_pose.translation += movement.translation;
        next_pose.rotation = body_rotation;

        self.is_grounded = movement.grounded;
        if self.is_grounded && self.velocity.y < 0.0 {
            self.velocity.y = -self.ground_stick_force;
        }

        if let Some(mut rigid) = self.rigid_body.upgrade(world)
            && let Some(body) = rigid.body_mut()
        {
            body.set_next_kinematic_position(next_pose);
            body.set_linvel(movement.translation / dt, true);
            body.set_angvel(Vec3::ZERO, true);
        }
    }

    fn post_update(&mut self, world: &mut World) {
        let dt = world.delta_time().as_secs_f32();
        self.update_camera_transform(world, (self.camera_smoothing * dt).clamp(0.0, 1.0));
    }
}

impl ThirdPersonCharacterController {
    fn read_movement_input(&self, world: &World) -> Vec2 {
        let mut fb: f32 = 0.0;
        if world.input.is_key_pressed(KeyCode::KeyW) {
            fb += 1.0;
        }
        if world.input.is_key_pressed(KeyCode::KeyS) {
            fb -= 1.0;
        }

        let mut lr: f32 = 0.0;
        if world.input.is_key_pressed(KeyCode::KeyA) {
            lr -= 1.0;
        }
        if world.input.is_key_pressed(KeyCode::KeyD) {
            lr += 1.0;
        }

        if fb.abs() < f32::EPSILON {
            fb = deadzone(
                world.input.gamepad.axis(Axis::LeftStickY),
                self.stick_deadzone,
            );
        }
        if lr.abs() < f32::EPSILON {
            lr = deadzone(
                world.input.gamepad.axis(Axis::LeftStickX),
                self.stick_deadzone,
            );
        }

        let input = Vec2::new(lr, fb);
        if input.length_squared() > 1.0 {
            input.normalize()
        } else {
            input
        }
    }

    fn update_look(&mut self, world: &World) {
        let dt = world.delta_time().as_secs_f32();

        if world.input.is_cursor_locked() {
            let mouse_delta = world.input.mouse_delta();
            self.camera_yaw += mouse_delta.x * self.mouse_sensitivity.x * 2.0 * dt;
            self.camera_pitch += mouse_delta.y * self.mouse_sensitivity.y * 2.0 * dt;
        }

        let stick_x = deadzone(
            world.input.gamepad.axis(Axis::RightStickX),
            self.stick_deadzone,
        );
        let stick_y = deadzone(
            world.input.gamepad.axis(Axis::RightStickY),
            self.stick_deadzone,
        );
        self.camera_yaw += -stick_x * self.controller_sensitivity.x * 100.0 * dt;
        self.camera_pitch += stick_y * self.controller_sensitivity.y * 100.0 * dt;
        self.camera_pitch = self.camera_pitch.clamp(self.min_pitch, self.max_pitch);
    }

    fn render_target(&self, world: &World) -> ViewportId {
        self.camera
            .upgrade(world)
            .map(|camera| camera.render_target())
            .unwrap_or(ViewportId::PRIMARY)
    }

    fn camera_relative_move_dir(&self, input: Vec2) -> Vec3 {
        let yaw_rotation = Quat::from_axis_angle(Vec3::Y, self.camera_yaw.to_radians());
        let forward = yaw_rotation * Vec3::NEG_Z;
        let right = yaw_rotation * Vec3::X;
        let mut dir = forward * input.y + right * input.x;
        dir.y = 0.0;

        let len2 = dir.length_squared();
        if len2 > 1.0 {
            dir /= len2.sqrt();
        }

        dir
    }

    fn build_character_controller(&self) -> KinematicCharacterController {
        KinematicCharacterController {
            up: Vec3::Y,
            offset: CharacterLength::Absolute(self.controller_offset.max(0.001)),
            slide: true,
            autostep: (self.step_height > 0.0).then_some(CharacterAutostep {
                max_height: CharacterLength::Absolute(self.step_height),
                min_width: CharacterLength::Absolute(self.step_min_width.max(0.0)),
                include_dynamic_bodies: false,
            }),
            max_slope_climb_angle: self.max_slope_climb_angle.to_radians(),
            min_slope_slide_angle: self.min_slope_slide_angle.to_radians(),
            snap_to_ground: Some(CharacterLength::Absolute(self.snap_to_ground.max(0.0))),
            normal_nudge_factor: self.normal_nudge_factor.max(1.0e-6),
        }
    }

    fn query_filter(&self, rigid_handle: Option<RigidBodyHandle>) -> QueryFilter<'static> {
        let mut filter = QueryFilter::new();

        if let Some(rigid_handle) = rigid_handle {
            filter = filter.exclude_rigid_body(rigid_handle);
        }

        if let Some(collider_handle) = self.self_collider_handle {
            filter = filter.exclude_collider(collider_handle);
        }

        filter
    }

    fn capsule_shape(&self) -> SharedShape {
        SharedShape::capsule_y(
            self.capsule_half_height.max(0.001),
            self.capsule_radius.max(0.001),
        )
    }

    fn sync_initial_angles(&mut self) {
        let forward = self.parent().transform.forward();
        let planar = Vec3::new(forward.x, 0.0, forward.z);
        if planar.length_squared() > 1.0e-5 {
            let yaw = yaw_from_direction(planar);
            self.body_yaw = yaw;
            self.camera_yaw = yaw;
        }
    }

    fn resolve_spawn_penetration(&mut self, world: &mut World) {
        let Some((rigid_handle, mut pose)) = self.current_pose(world) else {
            return;
        };

        let shape = self.capsule_shape();
        if !self.resolve_penetration_pose(world, &shape, &mut pose, Some(rigid_handle)) {
            return;
        }

        if let Some(mut rigid) = self.rigid_body.upgrade(world)
            && let Some(body) = rigid.body_mut()
        {
            body.set_position(pose, false);
            body.set_next_kinematic_position(pose);
            body.set_linvel(Vec3::ZERO, true);
            body.set_angvel(Vec3::ZERO, true);
        }
    }

    fn resolve_penetration_pose(
        &self,
        world: &World,
        shape: &SharedShape,
        pose: &mut Pose,
        rigid_handle: Option<RigidBodyHandle>,
    ) -> bool {
        let filter = self.query_filter(rigid_handle);
        let query_pipeline = world.physics.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &world.physics.rigid_body_set,
            &world.physics.collider_set,
            filter,
        );

        let options = ShapeCastOptions {
            max_time_of_impact: 0.0,
            target_distance: 0.0,
            stop_at_penetration: false,
            compute_impact_geometry_on_penetration: true,
        };

        let mut moved = false;
        for _ in 0..6 {
            let Some((_, hit)) = query_pipeline.cast_shape(pose, Vec3::Y, shape.as_ref(), options)
            else {
                break;
            };

            if hit.status != ShapeCastStatus::PenetratingOrWithinTargetDist {
                break;
            }

            let mut normal = hit.normal1;
            if normal.length_squared() < 1.0e-5 {
                normal = Vec3::Y;
            } else {
                normal = normal.normalize();
            }

            pose.translation += normal * (self.controller_offset + 0.03);
            moved = true;
        }

        moved
    }

    fn update_camera_transform(&mut self, world: &mut World, smoothing: f32) {
        let Some(camera) = self.camera.upgrade(world) else {
            if !self.warned_missing_camera {
                self.warned_missing_camera = true;
                warn!("ThirdPersonCharacterController could not find a camera");
            }
            return;
        };

        let pivot = self.parent().transform.position() + Vec3::Y * self.camera_height;
        let yaw_rot = Quat::from_axis_angle(Vec3::Y, self.camera_yaw.to_radians());
        let pitch_rot = Quat::from_axis_angle(Vec3::X, self.camera_pitch.to_radians());
        let global_look_rotation = yaw_rot * pitch_rot;
        let backward = global_look_rotation * Vec3::Z;

        let max_camera_distance = self
            .camera_distance
            .clamp(self.camera_min_distance, self.camera_max_distance);
        let collision_radius = self.camera_collision_radius.max(0.001);
        let mut target_distance = max_camera_distance;

        if let Some((hit, _)) = world.physics.cast_sphere(
            collision_radius,
            max_camera_distance,
            Pose::from_translation(pivot),
            backward,
            self.query_filter(self.rigid_body_handle(world)),
        ) {
            target_distance = hit.time_of_impact.max(self.camera_min_distance);
        }

        let smoothing = smoothing.clamp(0.0, 1.0);
        self.smoothed_camera_distance = self
            .smoothed_camera_distance
            .lerp(target_distance, smoothing);

        let parent_rotation = self.parent().transform.rotation();
        let local_look_rotation = parent_rotation.inverse() * global_look_rotation;
        let local_backward = local_look_rotation * Vec3::Z;
        let local_camera_position =
            Vec3::Y * self.camera_height + local_backward * self.smoothed_camera_distance;

        let mut camera_object = camera.parent();
        camera_object
            .transform
            .set_local_position_vec(local_camera_position);
        camera_object
            .transform
            .set_local_rotation(local_look_rotation);
    }

    fn current_pose(&self, world: &World) -> Option<(RigidBodyHandle, Pose)> {
        let rigid = self.rigid_body.upgrade(world)?;
        let body = rigid.body()?;
        Some((rigid.handle(), *body.position()))
    }

    fn rigid_body_handle(&self, world: &World) -> Option<RigidBodyHandle> {
        self.rigid_body
            .upgrade(world)
            .and_then(|rigid| rigid.body().map(|_| rigid.handle()))
    }
}

fn deadzone(value: f32, deadzone: f32) -> f32 {
    if value.abs() < deadzone { 0.0 } else { value }
}

fn yaw_from_direction(direction: Vec3) -> f32 {
    (-direction.x).atan2(-direction.z).to_degrees()
}

fn move_towards_angle(current: f32, target: f32, t: f32) -> f32 {
    let delta = shortest_angle_delta(current, target);
    current + delta * t.clamp(0.0, 1.0)
}

fn shortest_angle_delta(current: f32, target: f32) -> f32 {
    let mut delta = (target - current + 180.0).rem_euclid(360.0) - 180.0;
    if delta <= -180.0 {
        delta += 360.0;
    }
    delta
}
