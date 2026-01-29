use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;
use syrillian::math::{EulerRot, Quat, Vec2, Vec3, Vec3Swizzles};

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct FlashlightComponent {
    /// When true, the component captures the parents local transform as the base pose on init.
    pub use_initial_transform: bool,
    /// Local position relative to the parent (camera) when it isn't auto-captured on init.
    pub base_offset: Vec3,
    /// Base local rotation (degrees) when it isn't auto-captured on init.
    pub base_rotation_deg: Vec3,
    /// Mouse-driven rotation sway in degrees per pixel (pitch, yaw).
    pub sway_rotation: Vec2,
    /// Mouse-driven positional sway in units per pixel (x, y).
    pub sway_position: Vec2,
    /// Smoothing speed for sway.
    pub sway_smoothing: f32,
    /// Bob amplitude (x, y).
    pub bob_amplitude: Vec2,
    /// Bob frequency (x, y).
    pub bob_frequency: Vec2,
    /// How much bobbing scales with movement speed.
    pub bob_speed_scale: f32,
    /// Smoothing speed for bob.
    pub bob_smoothing: f32,
    /// Local movement lag in units per m/s (x, y, z).
    pub movement_lag: Vec3,
    /// Smoothing speed for positional lag.
    pub lag_smoothing: f32,
    /// Additional roll from strafing (deg per m/s).
    pub roll_from_strafe: f32,
    /// Max roll angle in degrees.
    pub max_roll: f32,

    #[dont_reflect]
    base_rotation: Quat,
    #[dont_reflect]
    current_pos: Vec3,
    #[dont_reflect]
    current_rot: Quat,
    #[dont_reflect]
    sway_pos: Vec3,
    #[dont_reflect]
    sway_rot: Vec2,
    #[dont_reflect]
    bob_phase: Vec2,
    #[dont_reflect]
    bob_offset: Vec3,
    #[dont_reflect]
    last_parent_pos: Vec3,
    #[dont_reflect]
    initialized: bool,
}

impl Default for FlashlightComponent {
    fn default() -> Self {
        FlashlightComponent {
            use_initial_transform: true,
            base_offset: Vec3::new(0.25, -0.2, -0.55),
            base_rotation_deg: Vec3::new(-2.0, 2.0, 0.0),
            sway_rotation: Vec2::new(0.28, 0.4),
            sway_position: Vec2::new(0.0006, 0.0008),
            sway_smoothing: 10.0,
            bob_amplitude: Vec2::new(0.12, 0.13),
            bob_frequency: Vec2::new(6.0, 12.0),
            bob_speed_scale: 10.35,
            bob_smoothing: 10.0,
            movement_lag: Vec3::new(0.015, 0.008, 0.03),
            lag_smoothing: 12.0,
            roll_from_strafe: 10.8,
            max_roll: 6.0,
            base_rotation: Quat::IDENTITY,
            current_pos: Vec3::ZERO,
            current_rot: Quat::IDENTITY,
            sway_pos: Vec3::ZERO,
            sway_rot: Vec2::ZERO,
            bob_phase: Vec2::ZERO,
            bob_offset: Vec3::ZERO,
            last_parent_pos: Vec3::ZERO,
            initialized: false,
        }
    }
}

impl Component for FlashlightComponent {
    fn init(&mut self, _world: &mut World) {
        let mut object = self.parent();
        if self.use_initial_transform {
            self.base_offset = *object.transform.local_position();
            self.base_rotation = *object.transform.local_rotation();
        } else {
            self.base_rotation = Quat::from_euler(
                EulerRot::XYZ,
                self.base_rotation_deg.x.to_radians(),
                self.base_rotation_deg.y.to_radians(),
                self.base_rotation_deg.z.to_radians(),
            );
            object.transform.set_local_position_vec(self.base_offset);
            object.transform.set_local_rotation(self.base_rotation);
        }

        self.current_pos = self.base_offset;
        self.current_rot = self.base_rotation;

        let parent_pos = (*object.parent())
            .filter(|parent| parent.exists())
            .map(|parent| parent.transform.position())
            .unwrap_or_else(|| object.transform.position());

        self.last_parent_pos = parent_pos;
        self.initialized = true;
    }

    fn update(&mut self, world: &mut World) {
        let dt = world.delta_time().as_secs_f32();
        if dt <= f32::EPSILON {
            return;
        }

        let mut object = self.parent();
        let parent = (*object.parent()).filter(|parent| parent.exists());
        let Some(parent) = parent else {
            return;
        };

        if !self.initialized {
            self.init(world);
        }

        let parent_pos = parent.transform.position();
        let parent_rot = parent.transform.rotation();
        let vel_world = (parent_pos - self.last_parent_pos) / dt;
        self.last_parent_pos = parent_pos;

        let local_vel = parent_rot.inverse() * vel_world;
        let speed = local_vel.xz().length();

        let sway_target = Vec2::new(
            -world.input.mouse_delta().y * self.sway_rotation.x,
            -world.input.mouse_delta().x * self.sway_rotation.y,
        );
        let sway_t = smooth_factor(self.sway_smoothing, dt);
        self.sway_rot = self.sway_rot.lerp(sway_target, sway_t);

        let sway_pos_target = Vec3::new(
            -world.input.mouse_delta().x * self.sway_position.x,
            -world.input.mouse_delta().y * self.sway_position.y,
            0.0,
        );
        self.sway_pos = self.sway_pos.lerp(sway_pos_target, sway_t);

        let bob_scale = (speed * self.bob_speed_scale).clamp(0.0, 2.0);
        self.bob_phase.x =
            (self.bob_phase.x + dt * self.bob_frequency.x * bob_scale) % std::f32::consts::TAU;
        self.bob_phase.y =
            (self.bob_phase.y + dt * self.bob_frequency.y * bob_scale) % std::f32::consts::TAU;
        let bob_target = Vec3::new(
            self.bob_phase.x.sin() * self.bob_amplitude.x,
            self.bob_phase.y.sin() * self.bob_amplitude.y,
            0.0,
        );
        let bob_t = smooth_factor(self.bob_smoothing, dt);
        self.bob_offset = self.bob_offset.lerp(bob_target, bob_t);

        let lag_offset = Vec3::new(
            -local_vel.x * self.movement_lag.x,
            -local_vel.y * self.movement_lag.y,
            -local_vel.z * self.movement_lag.z,
        );

        let target_pos = self.base_offset + self.sway_pos + self.bob_offset + lag_offset;
        let pos_t = smooth_factor(self.lag_smoothing, dt);
        self.current_pos = self.current_pos.lerp(target_pos, pos_t);

        let roll = (-local_vel.x * self.roll_from_strafe)
            .clamp(-self.max_roll, self.max_roll)
            .to_radians();
        let sway_rot_q = Quat::from_euler(
            EulerRot::XYZ,
            self.sway_rot.x.to_radians(),
            self.sway_rot.y.to_radians(),
            roll,
        );

        let base_rotation = if self.use_initial_transform {
            self.base_rotation
        } else {
            Quat::from_euler(
                EulerRot::XYZ,
                self.base_rotation_deg.x.to_radians(),
                self.base_rotation_deg.y.to_radians(),
                self.base_rotation_deg.z.to_radians(),
            )
        };
        let target_rot = base_rotation * sway_rot_q;
        let rot_t = smooth_factor(self.lag_smoothing, dt);
        self.current_rot = self.current_rot.slerp(target_rot, rot_t);

        object.transform.set_local_position_vec(self.current_pos);
        object.transform.set_local_rotation(self.current_rot);
    }
}

fn smooth_factor(speed: f32, dt: f32) -> f32 {
    1.0 - (-speed * dt).exp()
}
