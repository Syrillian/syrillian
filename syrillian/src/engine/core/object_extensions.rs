use crate::components::joints::{
    Fixed, JointComponent, JointTypeTrait, Prismatic, Revolute, Rope, Spring,
};
use crate::components::light::{LightComponent, LightTypeTrait};
use crate::components::{Collider3D, Component, RigidBodyComponent, RotateComponent};
use crate::core::{GameObject, GameObjectId};
use crate::rendering::lights::Light;
use nalgebra::{Point3, Unit, Vector3};
use rapier3d::dynamics::RigidBody;
use rapier3d::math::Isometry;
use rapier3d::prelude::Collider;
use std::ops::{Deref, DerefMut};

pub trait GameObjectExt {
    fn at(&mut self, x: f32, y: f32, z: f32) -> &mut Self;
    fn at_vec(&mut self, pos: Vector3<f32>) -> &mut Self;
    fn scale(&mut self, scale: f32) -> &mut Self;
    fn non_uniform_scale(&mut self, x: f32, y: f32, z: f32) -> &mut Self;
}

pub trait GOComponentExt<'a>: Component + Default {
    type Outer: Deref<Target = GameObject> + DerefMut;

    fn build_component(&'a mut self, obj: &'a mut GameObject) -> Self::Outer;
    fn finish(outer: &'a mut Self::Outer) -> &'a mut GameObject {
        outer.deref_mut()
    }
}

pub struct GOColliderExt<'a>(&'a mut Collider, &'a mut GameObject);
pub struct GORigidBodyExt<'a>(&'a mut RigidBody, &'a mut GameObject);
pub struct GOLightExt<'a, L: LightTypeTrait + 'static>(
    &'a mut LightComponent<L>,
    &'a mut GameObject,
);
pub struct GORotateExt<'a>(&'a mut RotateComponent, &'a mut GameObject);
pub struct GOJointExt<'a, T: JointTypeTrait>(&'a mut JointComponent<T>, &'a mut GameObject);

impl GameObjectExt for GameObject {
    #[inline]
    fn at(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.transform.set_position(x, y, z);
        self
    }

    #[inline]
    fn at_vec(&mut self, pos: Vector3<f32>) -> &mut Self {
        self.transform.set_position_vec(pos);
        self
    }

    #[inline]
    fn scale(&mut self, scale: f32) -> &mut Self {
        self.transform.set_scale(scale);
        self
    }

    #[inline]
    fn non_uniform_scale(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.transform.set_nonuniform_scale(x, y, z);
        self
    }
}

impl GameObject {
    #[inline]
    pub fn build_component<'a, C: GOComponentExt<'a>>(&'a mut self) -> C::Outer {
        let component = self.add_component::<C>();
        C::build_component(component.forget_lifetime(), self)
    }
}

impl<'a> GOComponentExt<'a> for Collider3D {
    type Outer = GOColliderExt<'a>;

    #[inline]
    fn build_component(&'a mut self, obj: &'a mut GameObject) -> Self::Outer {
        let collider = self.collider_mut().expect("Collider should be created");
        GOColliderExt(collider, obj)
    }

    #[inline]
    fn finish(outer: &'a mut Self::Outer) -> &'a mut GameObject {
        outer.1
    }
}

impl GOColliderExt<'_> {
    #[inline]
    pub fn mass(self, mass: f32) -> Self {
        self.0.set_mass(mass);
        self
    }

    #[inline]
    pub fn restitution(self, restitution: f32) -> Self {
        self.0.set_restitution(restitution);
        self
    }
}

impl Deref for GOColliderExt<'_> {
    type Target = GameObject;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1
    }
}

impl DerefMut for GOColliderExt<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.1
    }
}

impl<'a> GOComponentExt<'a> for RigidBodyComponent {
    type Outer = GORigidBodyExt<'a>;

    #[inline]
    fn build_component(&'a mut self, obj: &'a mut GameObject) -> Self::Outer {
        let rb = self.body_mut().expect("Rigid Body should be created");
        GORigidBodyExt(rb, obj)
    }

    #[inline]
    fn finish(outer: &'a mut Self::Outer) -> &'a mut GameObject {
        outer.1
    }
}

impl GORigidBodyExt<'_> {
    /// Enables continuous collision detection on this rigid body.
    /// Use this if it's bugging through walls, expected to move at fast speeds or
    /// expected to collide with high mass or high speed bodies.
    ///
    /// This makes the physics simulation more stable at the cost of performance
    ///
    /// This is disabled by default, this builder only provides an enable method.
    /// Please use RigidBodyComponent::get_body_mut for more settings
    #[inline]
    pub fn enable_ccd(self) -> Self {
        self.0.enable_ccd(true);
        self
    }

    #[inline]
    pub fn gravity_scale(self, scale: f32) -> Self {
        self.0.set_gravity_scale(scale, true);
        self
    }

    #[inline]
    pub fn angular_damping(self, damping: f32) -> Self {
        self.0.set_angular_damping(damping);
        self
    }

    #[inline]
    pub fn linear_damping(self, damping: f32) -> Self {
        self.0.set_linear_damping(damping);
        self
    }
}

impl Deref for GORigidBodyExt<'_> {
    type Target = GameObject;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1
    }
}

impl DerefMut for GORigidBodyExt<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.1
    }
}

impl<'a, L: LightTypeTrait> GOComponentExt<'a> for LightComponent<L> {
    type Outer = GOLightExt<'a, L>;

    #[inline]
    fn build_component(&'a mut self, obj: &'a mut GameObject) -> Self::Outer {
        GOLightExt(self, obj)
    }
}

impl<L: LightTypeTrait + 'static> GOLightExt<'_, L> {
    #[inline]
    pub fn color(self, r: f32, g: f32, b: f32) -> Self {
        self.0.set_color(r, g, b);
        self
    }

    #[inline]
    pub fn brightness(self, amount: f32) -> Self {
        self.0.set_intensity(amount);
        self
    }
}

impl<L: LightTypeTrait + 'static> Deref for GOLightExt<'_, L> {
    type Target = GameObject;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1
    }
}

impl<L: LightTypeTrait + 'static> DerefMut for GOLightExt<'_, L> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.1
    }
}

impl<'a> GOComponentExt<'a> for RotateComponent {
    type Outer = GORotateExt<'a>;

    #[inline]
    fn build_component(&'a mut self, obj: &'a mut GameObject) -> Self::Outer {
        GORotateExt(self, obj)
    }
}

impl GORotateExt<'_> {
    #[inline]
    pub fn speed(&mut self, speed: f32) -> &mut Self {
        self.0.rotate_speed = speed;
        self
    }

    #[inline]
    pub fn scaling(&mut self, scaling: f32) -> &mut Self {
        self.0.scale_coefficient = scaling;
        self
    }
}

impl Deref for GORotateExt<'_> {
    type Target = GameObject;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1
    }
}

impl DerefMut for GORotateExt<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.1
    }
}

impl<'a, T: JointTypeTrait> GOComponentExt<'a> for JointComponent<T> {
    type Outer = GOJointExt<'a, T>;

    #[inline]
    fn build_component(&'a mut self, obj: &'a mut GameObject) -> Self::Outer {
        GOJointExt(self, obj)
    }
}

impl<T: JointTypeTrait> Deref for GOJointExt<'_, T> {
    type Target = GameObject;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1
    }
}

impl<T: JointTypeTrait> DerefMut for GOJointExt<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.1
    }
}

impl<T: JointTypeTrait> GOJointExt<'_, T> {
    #[inline]
    pub fn connect_to(self, other: GameObjectId) -> Self {
        self.0.connect_to(other);
        self
    }

    #[inline]
    pub fn anchor1(self, point: Point3<f32>) -> Self {
        self.0.set_anchor1(point);
        self
    }

    #[inline]
    pub fn anchor2(self, point: Point3<f32>) -> Self {
        self.0.set_anchor2(point);
        self
    }

    #[inline]
    pub fn break_force(self, force: f32) -> Self {
        self.0.set_break_force(Some(force));
        self
    }

    #[inline]
    pub fn break_torque(self, torque: f32) -> Self {
        self.0.set_break_torque(Some(torque));
        self
    }

    #[inline]
    pub fn breakable(self, force: f32, torque: f32) -> Self {
        self.0.make_breakable(force, torque);
        self
    }
}

impl GOJointExt<'_, Fixed> {
    #[inline]
    pub fn frame1(self, frame: Isometry<f32>) -> Self {
        self.0.set_frame1(frame);
        self
    }

    #[inline]
    pub fn frame2(self, frame: Isometry<f32>) -> Self {
        self.0.set_frame2(frame);
        self
    }
}

impl GOJointExt<'_, Revolute> {
    #[inline]
    pub fn axis(self, axis: Unit<Vector3<f32>>) -> Self {
        self.0.set_axis(axis);
        self
    }

    #[inline]
    pub fn limits(self, min: f32, max: f32) -> Self {
        self.0.set_limits(min, max);
        self
    }

    #[inline]
    pub fn limits_deg(self, min: f32, max: f32) -> Self {
        self.0.set_limits_deg(min, max);
        self
    }
}

impl GOJointExt<'_, Prismatic> {
    #[inline]
    pub fn axis(self, axis: Unit<Vector3<f32>>) -> Self {
        self.0.set_axis(axis);
        self
    }

    #[inline]
    pub fn limits(self, min: f32, max: f32) -> Self {
        self.0.set_limits(min, max);
        self
    }
}

impl GOJointExt<'_, Rope> {
    #[inline]
    pub fn max_distance(self, distance: f32) -> Self {
        self.0.set_max_distance(distance);
        self
    }

    #[inline]
    pub fn length(self, distance: f32) -> Self {
        self.0.set_max_distance(distance);
        self
    }
}

impl GOJointExt<'_, Spring> {
    #[inline]
    pub fn rest_length(self, length: f32) -> Self {
        self.0.set_rest_length(length);
        self
    }

    #[inline]
    pub fn stiffness(self, length: f32) -> Self {
        self.0.set_stiffness(length);
        self
    }

    #[inline]
    pub fn damping(self, damping: f32) -> Self {
        self.0.set_damping(damping);
        self
    }

    #[inline]
    pub fn configure(self, rest_length: f32, stiffness: f32, damping: f32) -> Self {
        self.0.configure(rest_length, stiffness, damping);
        self
    }
}
