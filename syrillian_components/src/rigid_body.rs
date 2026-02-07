use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;
use syrillian::math::Pose;
use syrillian::physics::rapier3d::dynamics::{
    RigidBody, RigidBodyBuilder, RigidBodyHandle, RigidBodyType,
};
use syrillian::utils::QuaternionEuler;
use syrillian_utils::debug_panic;

#[derive(Debug, Default, Reflect)]
pub struct RigidBodyComponent {
    pub body_handle: Option<RigidBodyHandle>,
    #[reflect]
    kinematic: bool,
    #[reflect]
    prev_iso: Pose,
    #[reflect]
    curr_iso: Pose,
}

impl Component for RigidBodyComponent {
    fn init(&mut self, _world: &mut World) {
        let parent = self.parent();
        let initial_translation = parent.transform.position();
        let initial_rotation = parent.transform.rotation();
        let rigid_body = RigidBodyBuilder::dynamic()
            .user_data(parent.as_ffi() as u128)
            .translation(initial_translation)
            .rotation(initial_rotation.euler_vector())
            .build();

        let body_handle = self.world().physics.rigid_body_set.insert(rigid_body);
        self.body_handle = Some(body_handle);
    }

    fn fixed_update(&mut self, _world: &mut World) {
        let parent = self.parent();

        let Some(rb) = self.body_mut() else {
            debug_panic!("de-synced - remake_rigid_body();");
            return;
        };

        let translation = parent.transform.position();
        let rotation = parent.transform.rotation();
        if rb.is_dynamic() && parent.transform.is_dirty() {
            rb.set_translation(translation, false);
            rb.set_rotation(rotation, false);
        } else if rb.is_kinematic() && parent.transform.is_dirty() {
            rb.set_next_kinematic_translation(translation);
            rb.set_next_kinematic_rotation(rotation);
        }
    }

    fn post_fixed_update(&mut self, _world: &mut World) {
        let Some(rb) = self.body_mut() else {
            debug_panic!("de-synced - remake_rigid_body();");
            return;
        };

        let pos = *rb.position();
        self.prev_iso = self.curr_iso;
        self.curr_iso = pos;
    }

    fn post_update(&mut self, _world: &mut World) {
        let mut parent = self.parent();
        let pose = self.world_render_isometry();

        let Some(rb) = self.body() else {
            debug_panic!("de-synced - remake_rigid_body();");
            return;
        };

        parent.transform.set_position_vec(pose.translation);
        if rb.is_rotation_locked().iter().all(|l| !l) {
            parent.transform.set_rotation(pose.rotation);
        }
    }

    fn delete(&mut self, world: &mut World) {
        if let Some(handle) = self.body_handle.take() {
            world.physics.rigid_body_set.remove(
                handle,
                &mut world.physics.island_manager,
                &mut world.physics.collider_set,
                &mut world.physics.impulse_joint_set,
                &mut world.physics.multibody_joint_set,
                false,
            );
        }
    }
}

impl RigidBodyComponent {
    pub(crate) fn handle(&self) -> RigidBodyHandle {
        self.body_handle
            .expect("Handle should be initialized in init")
    }

    #[allow(unused)]
    pub(crate) fn handle_opt(&self) -> Option<RigidBodyHandle> {
        self.body_handle
    }

    pub fn body(&self) -> Option<&RigidBody> {
        self.world().physics.rigid_body_set.get(self.body_handle?)
    }

    pub fn body_mut(&mut self) -> Option<&mut RigidBody> {
        self.world()
            .physics
            .rigid_body_set
            .get_mut(self.body_handle?)
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

    pub fn world_render_isometry(&self) -> Pose {
        self.render_isometry(self.world().physics.alpha)
    }

    pub fn render_isometry(&self, alpha: f32) -> Pose {
        let p0 = self.prev_iso.translation;
        let p1 = self.curr_iso.translation;
        let p = p0 + (p1 - p0) * alpha;
        let r = self.prev_iso.rotation.slerp(self.curr_iso.rotation, alpha);
        Pose::from_parts(p, r)
    }
}
