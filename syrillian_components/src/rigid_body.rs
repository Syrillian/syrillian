use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;
use syrillian::math::{Affine3A, Pose};
use syrillian::physics::rapier3d::dynamics::{
    RigidBody, RigidBodyBuilder, RigidBodyHandle, RigidBodyType,
};
use syrillian::tracing::trace;
use syrillian::utils::QuaternionEuler;
use syrillian_utils::debug_panic;

#[derive(Debug, Default, Reflect)]
pub struct RigidBodyComponent {
    pub body_handle: Option<RigidBodyHandle>,
    #[reflect]
    kinematic: bool,
    prev_iso: Pose,
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
        self.sync_interpolation_to_current_pose();
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
            rb.set_translation(translation, true);
            rb.set_rotation(rotation, true);
        } else if rb.is_kinematic() && parent.transform.is_dirty() {
            rb.set_next_kinematic_translation(translation);
            rb.set_next_kinematic_rotation(rotation);
        }
    }

    fn post_fixed_update(&mut self, _world: &mut World) {
        let mut parent = self.parent();

        let Some(rb) = self.body_mut() else {
            debug_panic!("de-synced - remake_rigid_body();");
            return;
        };

        let pose = *rb.position();

        parent.transform.set_position_vec(pose.translation);
        if rb.is_rotation_locked().iter().all(|l| !l) {
            parent.transform.set_rotation(pose.rotation);
        }

        self.prev_iso = self.curr_iso;
        self.curr_iso = pose;
    }

    fn post_update(&mut self, _world: &mut World) {
        let Some(rb) = self.body() else {
            debug_panic!("de-synced - remake_rigid_body();");
            return;
        };

        let mut parent = self.parent();
        let pose = self.world_render_isometry();

        let locked_rotations = rb.is_rotation_locked();

        let scale = parent.transform.scale();
        let rotation = if locked_rotations.iter().all(|r| *r) {
            parent.transform.rotation()
        } else {
            pose.rotation
        };

        let render_affine =
            Affine3A::from_scale_rotation_translation(scale, rotation, pose.translation);

        parent.transform.set_render_affine(Some(render_affine));
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

        self.parent().transform.set_render_affine(None);
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
        if self.kinematic == kinematic {
            return;
        }

        let prev_state = if self.kinematic {
            "kinematic"
        } else {
            "dynamic"
        };

        let pose = {
            let rb = self.body_mut().expect("Rigid body de-synced");
            if kinematic {
                rb.set_body_type(RigidBodyType::KinematicPositionBased, false);
            } else {
                rb.set_body_type(RigidBodyType::Dynamic, false);
            }
            *rb.position()
        };

        self.kinematic = kinematic;
        self.prev_iso = pose;
        self.curr_iso = pose;

        let next_state = if self.kinematic {
            "kinematic"
        } else {
            "dynamic"
        };
        let object_id = self.parent().as_ffi();
        trace!(
            "[RigidBody] Object #{object_id} physics state changed: {prev_state} -> {next_state}"
        );
    }

    pub fn is_kinematic(&self) -> bool {
        self.kinematic
    }

    pub fn world_render_isometry(&self) -> Pose {
        self.render_isometry(self.world().physics.alpha)
    }

    pub fn render_isometry(&self, alpha: f32) -> Pose {
        self.prev_iso.lerp(&self.curr_iso, alpha)
    }

    fn sync_interpolation_to_current_pose(&mut self) {
        let Some(rb) = self.body() else {
            return;
        };
        let pose = *rb.position();
        self.prev_iso = pose;
        self.curr_iso = pose;
    }
}
