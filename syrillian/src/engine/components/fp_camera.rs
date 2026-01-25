use crate::Reflect;
use crate::World;
use crate::components::{CameraComponent, Component};
use crate::core::Transform;
use crate::input::InputManager;
use crate::utils::FloatMathExt;
use crate::windowing::RenderTargetId;
use gilrs::Axis;
use nalgebra::{UnitQuaternion, Vector2, Vector3};
use tracing::warn;

/// All tweakable parameters for the FPS Camera
#[derive(Debug, Clone, Reflect)]
#[reflect_all]
pub struct FPSCameraConfig {
    /// Mouse sensitivity coefficient. Default: X & Y = 0.6
    pub mouse_sensitivity: Vector2<f32>,
    /// Gamepad (right stick) sensitivity coefficient. Default: X & Y = 1.0
    pub controller_sensitivity: Vector2<f32>,
    /// Maximum up-down (pitch) angle. Default: 89.9
    pub max_pitch: f32,
    /// Maximum tilt (in degrees) when turning. Default: 1.0
    pub max_roll: f32,
    /// Look smoothing speed. Default: 12.0
    pub look_smoothing: f32,
    /// Bobbing amplitude on X and Y axes. Default: X = 0.015, Y = 0.03, Z = 0.0
    pub bob_amplitude: Vector3<f32>,
    /// Bobbing base frequency (walk cycles per second) for x/y. Default: (1.8, 3.0)
    pub bob_frequency: Vector2<f32>,
    /// How much bob increases while sprinting. Default: 0.35
    pub sprint_bob_scale: f32,
    /// Interpolation speed for bobbing and roll. Default: 12.0
    pub smoothing_speed: f32,
    /// Vertical bob on jump. Default: 0.6
    pub jump_bob_height: f32,
    /// How fast jump bob resets. Default: 6.0
    pub jump_bob_speed: f32,
    /// Idle sway amplitude. Default: 0.08
    pub idle_sway_amplitude: f32,
    /// Idle sway frequency. Default: 0.8
    pub idle_sway_frequency: f32,
    /// The normal unzoomed fov
    pub normal_fov: f32,
    /// Maximum zoom FOV
    pub zoom_fov: f32,
    /// Additional FOV kick while sprinting. Default: 3.0
    pub sprint_fov_kick: f32,
    /// How fast FOV changes interpolate. Default: 10.0
    pub fov_lerp_speed: f32,
    /// Enable the zoom feature
    pub enable_zoom: bool,
}

#[derive(Debug, Reflect)]
pub struct FirstPersonCameraController {
    #[reflect]
    pub config: FPSCameraConfig,

    #[reflect]
    yaw: f32,
    #[reflect]
    pitch: f32,
    #[reflect]
    smooth_roll: f32,
    bob_offset: Vector3<f32>,
    bob_phase: Vector3<f32>,

    pub vel: Vector3<f32>,

    jump_offset: f32,
    jump_bob_interp: f32,
    jump_bob_interp_t: f32,
    #[reflect]
    is_grounded: bool,
    #[reflect]
    zoom_factor: f32,

    #[reflect]
    pub base_position: Vector3<f32>,
    interp_yaw: f32,
    interp_pitch: f32,
    movement_speed_fraction: f32,
    is_sprinting: bool,
    idle_phase: f32,
}

impl Default for FPSCameraConfig {
    fn default() -> Self {
        // Make sure to change the document comments if you change these
        FPSCameraConfig {
            mouse_sensitivity: Vector2::new(0.6, 0.6),
            controller_sensitivity: Vector2::new(1.0, 1.0),
            max_pitch: 89.9,
            max_roll: 1.0,
            look_smoothing: 12.0,
            bob_amplitude: Vector3::new(0.05, 0.05, 0.0),
            bob_frequency: Vector2::new(3.0, 6.0),
            sprint_bob_scale: 0.35,
            smoothing_speed: 12.0,
            jump_bob_height: 0.6,
            jump_bob_speed: 6.0,
            idle_sway_amplitude: 0.08,
            idle_sway_frequency: 0.8,
            normal_fov: 60.0,
            zoom_fov: 30.0,
            sprint_fov_kick: 3.0,
            fov_lerp_speed: 10.0,
            enable_zoom: true,
        }
    }
}

impl Default for FirstPersonCameraController {
    fn default() -> Self {
        FirstPersonCameraController {
            config: FPSCameraConfig::default(),
            yaw: 0.0,
            pitch: 0.0,
            smooth_roll: 0.0,
            bob_offset: Vector3::zeros(),
            bob_phase: Vector3::zeros(),

            vel: Vector3::zeros(),

            jump_offset: 0.0,
            jump_bob_interp: 0.0,
            jump_bob_interp_t: 0.,
            is_grounded: false,
            zoom_factor: 0.0,

            base_position: Vector3::zeros(),
            interp_yaw: 0.0,
            interp_pitch: 0.0,
            movement_speed_fraction: 0.0,
            is_sprinting: false,
            idle_phase: 0.0,
        }
    }
}

impl Component for FirstPersonCameraController {
    fn init(&mut self, _world: &mut World) {
        self.base_position = *self.parent().transform.local_position();
    }

    fn update(&mut self, world: &mut World) {
        let mut parent = self.parent();
        let target = parent
            .get_component::<CameraComponent>()
            .map(|c| c.render_target())
            .unwrap_or(RenderTargetId::PRIMARY);
        world.input.set_active_target(target);

        let transform = &mut parent.transform;
        let delta_time = world.delta_time().as_secs_f32();

        if !world.input.is_window_focused() {
            return;
        }

        world.input.auto_cursor_lock();

        if !world.input.is_cursor_locked() {
            return;
        }

        self.calculate_jump_bob(delta_time);
        self.update_jump_bob(transform);

        let mouse_delta = world.input.mouse_delta();
        self.calculate_rotation(&world.input, delta_time, mouse_delta);
        self.update_rotation(transform, delta_time, mouse_delta);
        self.update_zoom();
    }
}

impl FirstPersonCameraController {
    pub fn set_zoom(&mut self, zoom_factor: f32) {
        self.zoom_factor = zoom_factor;
    }

    pub fn update_roll(&mut self, delta: f32, max: f32) {
        self.smooth_roll = (self.smooth_roll + delta / 70.0).clamp(-max, max);
    }

    pub fn apply_movement_state(&mut self, speed_fraction: f32, sprinting: bool) {
        self.movement_speed_fraction = speed_fraction.clamp(0.0, 3.0);
        self.is_sprinting = sprinting;
    }

    pub fn update_bob(&mut self, speed_factor: f32, is_sprinting: bool, dt: f32) {
        const TAU: f32 = std::f32::consts::TAU;

        let freq_scale = self.movement_speed_fraction.clamp(0.1, 1.4);

        let mul = (speed_factor / 4.).clamp(0.0, 2.5);
        let sprint_scale = if is_sprinting {
            1.0 + self.config.sprint_bob_scale
        } else {
            1.0
        };

        let freq_x = self.config.bob_frequency.x * freq_scale;
        let freq_y = self.config.bob_frequency.y * freq_scale;

        self.bob_phase.x = (self.bob_phase.x + dt * freq_x * mul) % TAU;
        self.bob_phase.y = (self.bob_phase.y + dt * freq_y * mul) % TAU;

        if mul < 0.05 {
            self.idle_phase = (self.idle_phase + dt * self.config.idle_sway_frequency) % TAU;
            let sway = self.idle_phase.sin() * self.config.idle_sway_amplitude;
            self.bob_offset = self.bob_offset.lerp(
                &Vector3::new(sway * 0.15, sway, 0.0),
                0.2 * dt * self.config.smoothing_speed,
            );
            return;
        }

        let sin_tx = self.bob_phase.x.sin();
        let sin_ty = self.bob_phase.y.sin();
        let target = Vector3::new(
            sin_tx * self.config.bob_amplitude.x * mul * sprint_scale,
            sin_ty * self.config.bob_amplitude.y * mul * sprint_scale,
            0.0,
        );

        self.bob_offset = self.bob_offset.lerp(&target, 0.04 * mul);
    }

    pub fn signal_jump(&mut self) {
        self.is_grounded = false;
        self.jump_offset = self.config.jump_bob_height;
    }

    pub fn signal_ground(&mut self) {
        self.is_grounded = true;
        self.jump_offset = 0.;
    }

    fn update_rotation(
        &mut self,
        transform: &mut Transform,
        delta_time: f32,
        mouse_delta: &Vector2<f32>,
    ) {
        if self.vel.xz().magnitude() < 0.01
            || self.vel.xz().normalize().dot(&transform.forward().xz()) > 0.9
        {
            self.update_roll(mouse_delta.x, self.config.max_roll);
        }

        self.interp_yaw = self
            .interp_yaw
            .lerp(self.yaw, self.config.look_smoothing * delta_time);
        self.interp_pitch = self
            .interp_pitch
            .lerp(self.pitch, self.config.look_smoothing * delta_time);

        self.smooth_roll = self
            .smooth_roll
            .lerp(0., self.config.smoothing_speed * delta_time);
        let roll_rotation =
            UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.smooth_roll.to_radians());

        let yaw_rot =
            UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.interp_yaw.to_radians());
        let pitch_rot =
            UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.interp_pitch.to_radians());

        transform.set_local_rotation(pitch_rot * roll_rotation);
        self.bob_offset = self
            .bob_offset
            .lerp(&Vector3::zeros(), self.config.smoothing_speed * delta_time);

        if let Some(mut parent) = *self.parent().parent() {
            parent.transform.set_local_rotation(yaw_rot);
        }
    }

    fn calculate_rotation(
        &mut self,
        input: &InputManager,
        delta_time: f32,
        mouse_delta: &Vector2<f32>,
    ) {
        let controller_x = -input.gamepad.axis(Axis::RightStickX)
            * self.config.controller_sensitivity.x
            * 100.
            * delta_time;
        let controller_y = input.gamepad.axis(Axis::RightStickY)
            * self.config.controller_sensitivity.y
            * 100.
            * delta_time;
        let mouse_x = mouse_delta.x * self.config.mouse_sensitivity.x / 30.0;
        let mouse_y = mouse_delta.y * self.config.mouse_sensitivity.y / 30.0;
        let max_pitch = self.config.max_pitch;

        self.yaw += mouse_x + controller_x;
        self.pitch = (self.pitch + mouse_y + controller_y).clamp(-max_pitch, max_pitch);
    }

    fn calculate_jump_bob(&mut self, delta_time: f32) {
        if !self.is_grounded {
            self.jump_offset = self.vel.y.clamp(-3.5, 3.5) / 3.5 * self.config.jump_bob_height;
        }

        self.jump_bob_interp_t = self
            .jump_bob_interp_t
            .lerp(self.config.jump_bob_speed, delta_time * 5.);
        self.jump_bob_interp = self
            .jump_bob_interp
            .lerp(self.jump_offset, self.jump_bob_interp_t * delta_time);
        self.jump_offset.lerp(0., 0.1);
    }

    fn update_jump_bob(&mut self, transform: &mut Transform) {
        let right = transform.right();
        let up = Vector3::y();
        let bob_offset =
            (right * self.bob_offset.x) + up * (self.bob_offset.y + self.jump_bob_interp);

        transform.set_local_position_vec(self.base_position + bob_offset);
    }

    fn calculate_zoom(&self) -> f32 {
        if !self.config.enable_zoom {
            return self.config.normal_fov;
        }
        let delta = self.config.normal_fov - self.config.zoom_fov;
        let zoomed = self.config.normal_fov - delta * self.zoom_factor.clamp(0.0, 1.0);
        let sprint_kick = self.config.sprint_fov_kick
            * if self.is_sprinting {
                self.movement_speed_fraction.clamp(0.0, 1.5)
            } else {
                0.0
            };

        zoomed + sprint_kick
    }

    fn update_zoom(&mut self) {
        let Some(mut camera) = self.parent().get_component::<CameraComponent>() else {
            warn!("Camera component not found");
            return;
        };

        camera.zoom_speed = self.config.fov_lerp_speed;
        camera.set_fov_target(self.calculate_zoom());
    }
}
