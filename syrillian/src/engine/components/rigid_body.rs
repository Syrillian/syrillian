use crate::Reflect;
use crate::World;
use crate::components::{Component, NewComponent};
use crate::core::GameObjectId;
use crate::utils::math::QuaternionEuler;
use nalgebra::{Isometry3, Translation3};
use rapier3d::prelude::*;
use syrillian_utils::debug_panic;

#[derive(Debug, Reflect)]
pub struct RigidBodyComponent {
    parent: GameObjectId,
    pub body_handle: RigidBodyHandle,
    kinematic: bool,
    prev_iso: Isometry3<f32>,
    curr_iso: Isometry3<f32>,
}

impl NewComponent for RigidBodyComponent {
    fn new(parent: GameObjectId) -> Self {
        let initial_translation = parent.transform.position();
        let initial_rotation = parent.transform.rotation().euler_vector();
        let rigid_body = RigidBodyBuilder::dynamic()
            .user_data(parent.as_ffi() as u128)
            .translation(initial_translation)
            .rotation(initial_rotation)
            .build();

        let body_handle = World::instance().physics.rigid_body_set.insert(rigid_body);

        RigidBodyComponent {
            parent,
            body_handle,
            kinematic: false,
            prev_iso: Isometry3::default(),
            curr_iso: Isometry3::default(),
        }
    }
}

impl Component for RigidBodyComponent {
    fn pre_fixed_update(&mut self, _world: &mut World) {
        let rb = World::instance()
            .physics
            .rigid_body_set
            .get_mut(self.body_handle);
        if let Some(rb) = rb {
            if rb.is_dynamic() && self.parent.transform.is_dirty() {
                rb.set_translation(self.parent.transform.position(), false);
                rb.set_rotation(self.parent.transform.rotation(), false);
            } else if rb.is_kinematic() {
                rb.set_next_kinematic_translation(self.parent.transform.position());
                rb.set_next_kinematic_rotation(self.parent.transform.rotation());
            }
        } else {
            debug_panic!("de-synced - remake_rigid_body();");
        }
    }

    fn fixed_update(&mut self, world: &mut World) {
        let rb = world.physics.rigid_body_set.get_mut(self.body_handle);
        if let Some(rb) = rb {
            self.prev_iso = self.curr_iso;
            self.curr_iso =
                Isometry3::from_parts(Translation3::from(*rb.translation()), *rb.rotation());
            if rb.is_dynamic() {
                self.parent.transform.set_position_vec(*rb.translation());
                if rb.is_rotation_locked().iter().all(|l| !l) {
                    self.parent.transform.set_rotation(*rb.rotation());
                }
            }
        }
    }

    fn delete(&mut self, world: &mut World) {
        world.physics.rigid_body_set.remove(
            self.body_handle,
            &mut world.physics.island_manager,
            &mut world.physics.collider_set,
            &mut world.physics.impulse_joint_set,
            &mut world.physics.multibody_joint_set,
            false,
        );
    }
}

impl RigidBodyComponent {
    pub fn body(&self) -> Option<&RigidBody> {
        World::instance()
            .physics
            .rigid_body_set
            .get(self.body_handle)
    }

    pub fn body_mut(&mut self) -> Option<&mut RigidBody> {
        World::instance()
            .physics
            .rigid_body_set
            .get_mut(self.body_handle)
    }

    pub fn set_kinematic(&mut self, kinematic: bool) {
        let rb = self.body_mut().expect("Rigid body de-synced");
        if kinematic {
            rb.set_body_type(RigidBodyType::KinematicPositionBased, false);
        } else {
            rb.set_body_type(RigidBodyType::Dynamic, false);
        }
        self.kinematic = kinematic;
    }

    pub fn is_kinematic(&self) -> bool {
        self.kinematic
    }

    pub fn render_isometry(&self, alpha: f32) -> Isometry3<f32> {
        let p0 = self.prev_iso.translation.vector;
        let p1 = self.curr_iso.translation.vector;
        let p = p0 + (p1 - p0) * alpha;
        let r = self.prev_iso.rotation.slerp(&self.curr_iso.rotation, alpha);
        Isometry3::from_parts(Translation3::from(p), r)
    }
}
