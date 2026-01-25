use crate::core::reflection::{PartialReflect, Reflect, ReflectedField, ReflectedTypeInfo};
use crate::{
    World,
    components::{Component, RigidBodyComponent},
    core::GameObjectId,
};
use nalgebra::{Point3, Unit, Vector3};
use rapier3d::{
    math::{Isometry, Vector},
    prelude::{
        FixedJointBuilder, GenericJoint, ImpulseJointHandle, JointAxis, PrismaticJointBuilder,
        RevoluteJointBuilder, RigidBody, RopeJointBuilder, SphericalJointBuilder,
        SpringJointBuilder,
    },
};
use snafu::{Snafu, ensure};
use std::any::TypeId;
use std::mem::offset_of;
use std::{f32, marker::PhantomData};
use syrillian_macros::Reflect;
use tracing::warn;

#[derive(Debug, Snafu)]
#[snafu(context(suffix(Err)))]
pub enum JointError {
    #[snafu(display("JointComponent: Connector doesnt exist"))]
    InvalidConnector,
    #[snafu(display("JointComponent: Parent doesnt have a rigid body"))]
    NoParentRigidBody,
    #[snafu(display("JointComponent: Connector doesnt have a rigid body"))]
    NoConnectorRigidBody,
}

pub trait JointTypeTrait: Send + Sync + 'static {
    type Config: Reflect + Default + Clone + Send + Sync;

    const NAME: &'static str;
    const FULL_NAME: &'static str;

    fn build(config: &Self::Config, anchor1: Point3<f32>, anchor2: Point3<f32>) -> GenericJoint;
}

pub struct Fixed;
pub struct Revolute;
pub struct Prismatic;
pub struct Spherical;
pub struct Rope;
pub struct Spring;

#[derive(Clone, Default, Reflect)]
#[reflect_all]
pub struct FixedConfig {
    pub frame1: Isometry<f32>,
    pub frame2: Isometry<f32>,
}

#[derive(Clone, Reflect)]
#[reflect_all]
pub struct RevoluteConfig {
    pub axis: Unit<Vector3<f32>>,
    pub limits: Option<[f32; 2]>,
}

#[derive(Clone, Reflect)]
#[reflect_all]
pub struct PrismaticConfig {
    pub axis: Unit<Vector3<f32>>,
    pub limits: Option<[f32; 2]>,
}

#[derive(Clone, Default, Reflect)]
#[reflect_all]
pub struct SphericalConfig;

#[derive(Clone, Reflect)]
#[reflect_all]
pub struct RopeConfig {
    pub max_distance: f32,
}

#[derive(Clone, Reflect)]
#[reflect_all]
pub struct SpringConfig {
    pub rest_length: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Default for RevoluteConfig {
    fn default() -> Self {
        Self {
            axis: Vector3::y_axis(),
            limits: None,
        }
    }
}

impl Default for PrismaticConfig {
    fn default() -> Self {
        Self {
            axis: Vector3::y_axis(),
            limits: None,
        }
    }
}

impl Default for RopeConfig {
    fn default() -> Self {
        Self { max_distance: 10.0 }
    }
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            rest_length: 1.0,
            stiffness: 100.0,
            damping: 10.0,
        }
    }
}

impl JointTypeTrait for Fixed {
    type Config = FixedConfig;

    const NAME: &str = "FixedJoint";
    const FULL_NAME: &str = concat!(module_path!(), "::", "FixedJoint");

    fn build(config: &Self::Config, anchor1: Point3<f32>, anchor2: Point3<f32>) -> GenericJoint {
        FixedJointBuilder::new()
            .local_anchor1(anchor1)
            .local_anchor2(anchor2)
            .local_frame1(config.frame1)
            .local_frame2(config.frame2)
            .build()
            .into()
    }
}

impl JointTypeTrait for Revolute {
    type Config = RevoluteConfig;

    const NAME: &str = "RevoluteJoint";
    const FULL_NAME: &str = concat!(module_path!(), "::", "RevoluteJoint");

    fn build(config: &Self::Config, anchor1: Point3<f32>, anchor2: Point3<f32>) -> GenericJoint {
        let mut b = RevoluteJointBuilder::new(config.axis)
            .local_anchor1(anchor1)
            .local_anchor2(anchor2);

        if let Some(lim) = config.limits {
            b = b.limits(lim);
        }

        b.build().into()
    }
}

impl JointTypeTrait for Prismatic {
    type Config = PrismaticConfig;

    const NAME: &str = "PrismaticJoint";
    const FULL_NAME: &str = concat!(module_path!(), "::", "PrismaticJoint");

    fn build(config: &Self::Config, anchor1: Point3<f32>, anchor2: Point3<f32>) -> GenericJoint {
        let mut b = PrismaticJointBuilder::new(config.axis)
            .local_anchor1(anchor1)
            .local_anchor2(anchor2);

        if let Some(lim) = config.limits {
            b = b.limits(lim);
        }

        b.build().into()
    }
}

impl JointTypeTrait for Spherical {
    type Config = SphericalConfig;

    const NAME: &str = "SphericalJoint";
    const FULL_NAME: &str = concat!(module_path!(), "::", "SphericalJoint");

    fn build(_: &Self::Config, anchor1: Point3<f32>, anchor2: Point3<f32>) -> GenericJoint {
        SphericalJointBuilder::new()
            .local_anchor1(anchor1)
            .local_anchor2(anchor2)
            .build()
            .into()
    }
}

impl JointTypeTrait for Rope {
    type Config = RopeConfig;

    const NAME: &str = "RopeJoint";
    const FULL_NAME: &str = concat!(module_path!(), "::", "RopeJoint");

    fn build(config: &Self::Config, anchor1: Point3<f32>, anchor2: Point3<f32>) -> GenericJoint {
        RopeJointBuilder::new(config.max_distance)
            .local_anchor1(anchor1)
            .local_anchor2(anchor2)
            .build()
            .into()
    }
}

impl JointTypeTrait for Spring {
    type Config = SpringConfig;

    const NAME: &str = "SpringJoint";
    const FULL_NAME: &str = concat!(module_path!(), "::", "SpringJoint");

    fn build(config: &Self::Config, anchor1: Point3<f32>, anchor2: Point3<f32>) -> GenericJoint {
        SpringJointBuilder::new(config.rest_length, config.stiffness, config.damping)
            .local_anchor1(anchor1)
            .local_anchor2(anchor2)
            .build()
            .into()
    }
}

pub struct JointComponent<T: JointTypeTrait> {
    pub connected: Option<GameObjectId>,
    handle: Option<ImpulseJointHandle>,
    anchor1: Point3<f32>,
    anchor2: Point3<f32>,
    pub break_force: Option<f32>,
    pub break_torque: Option<f32>,
    pub broken: bool,
    pub config: T::Config,
    _marker: PhantomData<T>,
}

pub type FixedJoint = JointComponent<Fixed>;
pub type RevoluteJoint = JointComponent<Revolute>;
pub type PrismaticJoint = JointComponent<Prismatic>;
pub type SphericalJoint = JointComponent<Spherical>;
pub type RopeJoint = JointComponent<Rope>;
pub type SpringJoint = JointComponent<Spring>;

impl<T: JointTypeTrait> PartialReflect for JointComponent<T> {
    const DATA: ReflectedTypeInfo = ReflectedTypeInfo {
        type_id: TypeId::of::<Self>(),
        type_name: T::FULL_NAME,
        short_name: T::NAME,
        fields: &[
            ReflectedField {
                name: "broken",
                offset: offset_of!(Self, broken),
                type_id: TypeId::of::<bool>(),
            },
            ReflectedField {
                name: "break_force",
                offset: offset_of!(Self, break_force),
                type_id: TypeId::of::<Option<bool>>(),
            },
            ReflectedField {
                name: "break_torque",
                offset: offset_of!(Self, break_torque),
                type_id: TypeId::of::<Option<bool>>(),
            },
            ReflectedField {
                name: "config",
                offset: offset_of!(Self, config),
                type_id: TypeId::of::<T::Config>(),
            },
        ],
    };
}

inventory::submit! { FixedJoint::DATA }
inventory::submit! { RevoluteJoint::DATA }
inventory::submit! { PrismaticJoint::DATA }
inventory::submit! { SphericalJoint::DATA }
inventory::submit! { RopeJoint::DATA }
inventory::submit! { SpringJoint::DATA }

impl<T: JointTypeTrait> Default for JointComponent<T> {
    fn default() -> Self {
        Self {
            connected: None,
            handle: None,
            anchor1: Point3::origin(),
            anchor2: Point3::origin(),
            break_force: None,
            break_torque: None,
            broken: false,
            config: T::Config::default(),
            _marker: PhantomData,
        }
    }
}

impl<T: JointTypeTrait> Component for JointComponent<T> {
    fn fixed_update(&mut self, world: &mut crate::World) {
        if self.handle.is_some() && !self.broken {
            self.check_break(world);
        }
    }

    fn delete(&mut self, world: &mut crate::World) {
        self.disconnect(world);
    }
}

impl<T: JointTypeTrait> JointComponent<T> {
    // connection
    pub fn connect_to(&mut self, body: GameObjectId) {
        if let Err(e) = self.try_connect_to(body) {
            warn!("{e}");
        }
    }

    pub fn try_connect_to(&mut self, body: GameObjectId) -> Result<(), JointError> {
        ensure!(body.exists(), InvalidConnectorErr);

        let parent = self.parent();

        let self_rb = parent
            .get_component::<RigidBodyComponent>()
            .ok_or(JointError::NoParentRigidBody)?
            .body_handle
            .ok_or(JointError::NoParentRigidBody)?;

        let other_rb = body
            .get_component::<RigidBodyComponent>()
            .ok_or(JointError::NoConnectorRigidBody)?
            .body_handle
            .ok_or(JointError::NoConnectorRigidBody)?;

        let joint = T::build(&self.config, self.anchor1, self.anchor2);

        self.handle = Some(
            self.world()
                .physics
                .impulse_joint_set
                .insert(self_rb, other_rb, joint, true),
        );
        self.connected = Some(body);
        self.broken = false;

        Ok(())
    }

    pub fn disconnect(&mut self, world: &mut World) {
        if let Some(h) = self.handle.take() {
            world.physics.impulse_joint_set.remove(h, false);
            self.connected = None;
        }
    }

    pub fn reconnect(&mut self, world: &mut World) {
        if let Some(body) = self.connected {
            self.disconnect(world);
            self.connect_to(body);
        }
    }

    // accessors

    pub fn is_connected(&self) -> bool {
        self.handle.is_some() && !self.broken
    }

    pub fn is_broken(&self) -> bool {
        self.broken
    }

    pub fn connected(&self) -> Option<GameObjectId> {
        self.connected
    }

    pub fn handle(&self) -> Option<ImpulseJointHandle> {
        self.handle
    }

    pub fn joint_data(&self) -> Option<&GenericJoint> {
        Some(
            &self
                .world()
                .physics
                .impulse_joint_set
                .get(self.handle?)?
                .data,
        )
    }

    pub fn joint_data_mut(&self) -> Option<&mut GenericJoint> {
        Some(
            &mut World::instance()
                .physics
                .impulse_joint_set
                .get_mut(self.handle?, false)?
                .data,
        )
    }

    // anchors

    pub fn set_anchor1(&mut self, anchor: Point3<f32>) {
        self.anchor1 = anchor;
        if let Some(j) = self.joint_data_mut() {
            j.set_local_anchor1(anchor);
        }
    }

    pub fn set_anchor2(&mut self, anchor: Point3<f32>) {
        self.anchor2 = anchor;
        if let Some(j) = self.joint_data_mut() {
            j.set_local_anchor2(anchor);
        }
    }

    pub fn anchor1(&self) -> Point3<f32> {
        self.anchor1
    }

    pub fn anchor2(&self) -> Point3<f32> {
        self.anchor2
    }

    // break behavior

    pub fn set_break_force(&mut self, force: Option<f32>) {
        self.break_force = force;
    }

    pub fn set_break_torque(&mut self, torque: Option<f32>) {
        self.break_torque = torque;
    }

    pub fn make_breakable(&mut self, force: f32, torque: f32) {
        self.break_force = Some(force);
        self.break_torque = Some(torque);
    }

    pub fn make_unbreakable(&mut self) {
        self.break_force = None;
        self.break_torque = None;
    }

    pub fn repair(&mut self) {
        if self.broken
            && let Some(body) = self.connected
        {
            self.broken = false;
            self.connect_to(body);
        }
    }

    pub fn check_break(&mut self, world: &mut World) {
        let dominated = self
            .break_force
            .is_some_and(|t| self.force_magnitude().is_some_and(|f| f > t))
            || self
                .break_torque
                .is_some_and(|t| self.torque_magnitude().is_some_and(|f| f > t));

        if dominated {
            self.disconnect(world);
            self.broken = true;
        }
    }

    // physics queires

    fn bodies(&self) -> Option<(&RigidBody, &RigidBody)> {
        let world = self.world();
        let jd = world.physics.impulse_joint_set.get(self.handle?)?;
        let rb1 = world.physics.rigid_body_set.get(jd.body1)?;
        let rb2 = world.physics.rigid_body_set.get(jd.body2)?;

        Some((rb1, rb2))
    }

    pub fn anchor_distance(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        let w1 = rb1.position() * self.anchor1;
        let w2 = rb2.position() * self.anchor2;

        Some((w2 - w1).magnitude())
    }

    pub fn anchor_direction(&self) -> Option<Unit<Vector3<f32>>> {
        let (rb1, rb2) = self.bodies()?;
        let w1 = rb1.position() * self.anchor1;
        let w2 = rb2.position() * self.anchor2;
        Unit::try_new(w2 - w1, f32::EPSILON)
    }

    pub fn world_anchor1(&self) -> Option<Point3<f32>> {
        let (rb1, _) = self.bodies()?;
        Some(rb1.position() * self.anchor1)
    }

    pub fn world_anchor2(&self) -> Option<Point3<f32>> {
        let (_, rb2) = self.bodies()?;
        Some(rb2.position() * self.anchor2)
    }

    pub fn force(&self) -> Option<Vector3<f32>> {
        let (rb1, rb2) = self.bodies()?;
        let reduced = (rb1.mass() * rb2.mass()) / (rb1.mass() + rb2.mass());
        Some((rb2.linvel() - rb1.linvel()) * reduced)
    }

    pub fn force_magnitude(&self) -> Option<f32> {
        self.force().map(|f| f.magnitude())
    }

    pub fn torque(&self) -> Option<Vector3<f32>> {
        let (rb1, rb2) = self.bodies()?;
        Some(rb2.angvel() - rb1.angvel())
    }

    pub fn torque_magnitude(&self) -> Option<f32> {
        self.torque().map(|t| t.magnitude())
    }
}

impl JointComponent<Fixed> {
    pub fn set_frame1(&mut self, q: Isometry<f32>) {
        self.config.frame1 = q;
        if let Some(j) = self.joint_data_mut()
            && let Some(f) = j.as_fixed_mut()
        {
            f.set_local_frame1(q);
        }
    }

    pub fn set_frame2(&mut self, q: Isometry<f32>) {
        self.config.frame2 = q;
        if let Some(j) = self.joint_data_mut()
            && let Some(f) = j.as_fixed_mut()
        {
            f.set_local_frame2(q);
        }
    }

    pub fn rotation_error(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        let expected = rb1.rotation() * self.config.frame1 * self.config.frame2.inverse();

        Some((expected.inverse() * rb2.rotation()).rotation.angle())
    }
}

impl JointComponent<Revolute> {
    pub fn set_axis(&mut self, axis: Unit<Vector3<f32>>) {
        self.config.axis = axis;
    }

    pub fn set_limits(&mut self, min: f32, max: f32) {
        self.config.limits = Some([min, max]);
        if let Some(j) = self.joint_data_mut()
            && let Some(r) = j.as_revolute_mut()
        {
            r.set_limits([min, max]);
        }
    }

    pub fn set_limits_deg(&mut self, min: f32, max: f32) {
        self.set_limits(min.to_radians(), max.to_radians());
    }

    pub fn angle(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        self.joint_data()?
            .as_revolute()?
            .angle(rb1.rotation(), rb2.rotation())
            .into()
    }

    pub fn angle_deg(&self) -> Option<f32> {
        self.angle().map(|a| a.to_degrees())
    }

    pub fn angular_velocity(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        Some((rb2.angvel() - rb1.angvel()).dot(&self.config.axis))
    }

    pub fn set_motor_velocity(&mut self, vel: f32, max_force: f32) {
        if let Some(r) = self.joint_data_mut().and_then(|j| j.as_revolute_mut()) {
            r.set_motor_velocity(vel, max_force);
        }
    }

    pub fn set_motor_position(&mut self, angle: f32, stiffness: f32, damping: f32) {
        if let Some(r) = self.joint_data_mut().and_then(|j| j.as_revolute_mut()) {
            r.set_motor_position(angle, stiffness, damping);
        }
    }

    pub fn at_min(&self) -> bool {
        matches!((self.angle(), self.config.limits), (Some(a), Some([m, _])) if (a - m).abs() < 0.01)
    }

    pub fn at_max(&self) -> bool {
        matches!((self.angle(), self.config.limits), (Some(a), Some([_, m])) if (a - m).abs() < 0.01)
    }

    pub fn limit_ratio(&self) -> Option<f32> {
        let a = self.angle()?;
        let [min, max] = self.config.limits?;
        Some(((a - min) / (max - min)).clamp(0.0, 1.0))
    }
}

impl JointComponent<Prismatic> {
    pub fn set_axis(&mut self, axis: Unit<Vector3<f32>>) {
        self.config.axis = axis;
    }

    pub fn set_limits(&mut self, min: f32, max: f32) {
        self.config.limits = Some([min, max]);
        if let Some(p) = self.joint_data_mut().and_then(|j| j.as_prismatic_mut()) {
            p.set_limits([min, max]);
        }
    }

    pub fn translation(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        let w1 = rb1.position() * self.anchor1;
        let w2 = rb2.position() * self.anchor2;
        let sub = w2 - w1;

        let vec = Vector::new(sub.x, sub.y, sub.z);

        Some(vec.dot(&self.config.axis))
    }

    pub fn velocity(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        Some((rb2.linvel() - rb1.linvel()).dot(&self.config.axis))
    }

    pub fn set_motor_velocity(&mut self, vel: f32, max_force: f32) {
        if let Some(p) = self.joint_data_mut().and_then(|j| j.as_prismatic_mut()) {
            p.set_motor_velocity(vel, max_force);
        }
    }

    pub fn set_motor_position(&mut self, pos: f32, stiffness: f32, damping: f32) {
        if let Some(p) = self.joint_data_mut().and_then(|j| j.as_prismatic_mut()) {
            p.set_motor_position(pos, stiffness, damping);
        }
    }

    pub fn extend(&mut self, vel: f32, max_force: f32) {
        self.set_motor_velocity(vel.abs(), max_force);
    }

    pub fn retract(&mut self, vel: f32, max_force: f32) {
        self.set_motor_velocity(-vel.abs(), max_force);
    }

    pub fn is_extended(&self) -> bool {
        matches!((self.translation(), self.config.limits), (Some(t), Some([_, m])) if (t - m).abs() < 0.01)
    }

    pub fn is_retracted(&self) -> bool {
        matches!((self.translation(), self.config.limits), (Some(t), Some([m, _])) if (t - m).abs() < 0.01)
    }

    pub fn position_ratio(&self) -> Option<f32> {
        let t = self.translation()?;
        let [min, max] = self.config.limits?;
        Some(((t - min) / (max - min)).clamp(0.0, 1.0))
    }
}

impl JointComponent<Rope> {
    pub fn set_max_distance(&mut self, d: f32) {
        self.config.max_distance = d;
        if let Some(r) = self.joint_data_mut().and_then(|j| j.as_rope_mut()) {
            r.set_max_distance(d);
        }
    }

    pub fn max_distance(&self) -> f32 {
        self.config.max_distance
    }

    pub fn current_distance(&self) -> Option<f32> {
        self.anchor_distance()
    }

    pub fn is_taut(&self) -> bool {
        self.current_distance()
            .is_some_and(|d| d >= self.config.max_distance - 0.01)
    }

    pub fn is_slack(&self) -> bool {
        !self.is_taut()
    }

    pub fn slack(&self) -> Option<f32> {
        self.current_distance()
            .map(|d| (self.config.max_distance - d).max(0.0))
    }

    pub fn tension_ratio(&self) -> Option<f32> {
        self.current_distance()
            .map(|d| (d / self.config.max_distance).min(1.0))
    }

    pub fn shorten(&mut self, amount: f32) {
        self.set_max_distance((self.config.max_distance - amount).max(0.1));
    }

    pub fn lengthen(&mut self, amount: f32) {
        self.set_max_distance(self.config.max_distance + amount);
    }

    pub fn tighten(&mut self) {
        if let Some(d) = self.current_distance() {
            self.set_max_distance(d);
        }
    }
}

impl JointComponent<Spring> {
    pub fn set_rest_length(&mut self, l: f32) {
        self.config.rest_length = l;
        self.refresh_motor();
    }

    pub fn set_stiffness(&mut self, k: f32) {
        self.config.stiffness = k;
        self.refresh_motor();
    }

    pub fn set_damping(&mut self, c: f32) {
        self.config.damping = c;
        self.refresh_motor();
    }

    pub fn configure(&mut self, rest: f32, stiffness: f32, damping: f32) {
        self.config.rest_length = rest;
        self.config.stiffness = stiffness;
        self.config.damping = damping;
        self.refresh_motor();
    }

    pub fn refresh_motor(&mut self) {
        if let Some(j) = self.joint_data_mut() {
            j.set_motor_position(
                JointAxis::LinX,
                self.config.rest_length,
                self.config.stiffness,
                self.config.damping,
            );
        }
    }

    pub fn rest_length(&self) -> f32 {
        self.config.rest_length
    }

    pub fn stiffness(&self) -> f32 {
        self.config.stiffness
    }

    pub fn damping(&self) -> f32 {
        self.config.damping
    }

    pub fn current_length(&self) -> Option<f32> {
        self.anchor_distance()
    }

    pub fn extension(&self) -> Option<f32> {
        self.current_length().map(|l| l - self.config.rest_length)
    }

    pub fn is_stretched(&self) -> bool {
        self.extension().is_some_and(|e| e > 0.01)
    }

    pub fn is_compressed(&self) -> bool {
        self.extension().is_some_and(|e| e < -0.01)
    }

    pub fn is_at_rest(&self) -> bool {
        self.extension().is_some_and(|e| e.abs() < 0.01)
    }

    pub fn spring_force(&self) -> Option<f32> {
        self.extension().map(|x| self.config.stiffness * x * x)
    }

    pub fn potential_energy(&self) -> Option<f32> {
        self.extension()
            .map(|x| 0.5 * self.config.stiffness * x * x)
    }

    pub fn natural_frequency(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        let reduced = (rb1.mass() * rb2.mass()) / (rb1.mass() + rb2.mass());

        Some((self.config.stiffness / reduced).sqrt())
    }

    pub fn period(&self) -> Option<f32> {
        self.natural_frequency().map(|w| std::f32::consts::TAU / w)
    }

    pub fn damping_ratio(&self) -> Option<f32> {
        let (rb1, rb2) = self.bodies()?;
        let reduced = (rb1.mass() * rb2.mass()) / (rb1.mass() + rb2.mass());
        let critical = 2.0 * (self.config.stiffness * reduced).sqrt();

        Some(self.config.damping / critical)
    }

    pub fn set_damping_ratio(&mut self, ratio: f32) {
        if let Some((rb1, rb2)) = self.bodies() {
            let reduced = (rb1.mass() * rb2.mass()) / (rb1.mass() + rb2.mass());
            let critical = 2.0 * (self.config.stiffness * reduced).sqrt();
            self.set_damping(critical * ratio.clamp(0.0, 2.0));
        }
    }
}
