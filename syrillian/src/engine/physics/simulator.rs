use crate::World;
use crate::core::GameObjectId;
use nalgebra::Vector3;
use rapier3d::parry::query::{DefaultQueryDispatcher, ShapeCastOptions};
use rapier3d::prelude::*;
use syrillian_macros::Reflect;
use web_time::{Duration, Instant};

#[derive(Reflect)]
pub struct PhysicsManager {
    #[reflect]
    pub gravity: Vector3<f32>,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhaseBvh,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub physics_hooks: (),
    pub event_handler: (),
    pub last_update: Instant,
    #[reflect]
    pub timestep: Duration,
    pub alpha: f32,
}

const EARTH_GRAVITY: f32 = 9.81;

impl Default for PhysicsManager {
    fn default() -> Self {
        PhysicsManager {
            gravity: Vector3::new(0.0, -EARTH_GRAVITY, 0.0),
            rigid_body_set: RigidBodySet::default(),
            collider_set: ColliderSet::default(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::default(),
            island_manager: IslandManager::default(),
            broad_phase: DefaultBroadPhase::default(),
            narrow_phase: NarrowPhase::default(),
            impulse_joint_set: ImpulseJointSet::default(),
            multibody_joint_set: MultibodyJointSet::default(),
            ccd_solver: CCDSolver,
            physics_hooks: (),
            event_handler: (),
            last_update: Instant::now(),
            timestep: Duration::from_secs_f64(1.0 / 60.0),
            alpha: 0.0,
        }
    }
}

impl PhysicsManager {
    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(), // no hooks yet
            &(), // no events yet
        );
    }

    pub fn cast_ray(
        &self,
        ray: &Ray,
        max_toi: f32,
        solid: bool,
        filter: QueryFilter,
    ) -> Option<(f32, GameObjectId)> {
        let qp = self.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.rigid_body_set,
            &self.collider_set,
            filter,
        );
        let (collider, distance) = qp.cast_ray(ray, max_toi, solid)?;

        let object_id = self.collider_set.get(collider)?.user_data as u64;
        let object = GameObjectId::from_ffi(object_id);

        object.exists().then_some((distance, object))
    }

    pub fn cast_sphere(
        &self,
        radius: f32,
        max_toi: f32,
        shape_pos: &Isometry<f32>,
        dir: &Vector<f32>,
        filter: QueryFilter,
    ) -> Option<(ShapeCastHit, GameObjectId)> {
        let qp = self.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.rigid_body_set,
            &self.collider_set,
            filter,
        );
        let shape = Ball::new(radius);
        let options = ShapeCastOptions::with_max_time_of_impact(max_toi);
        let (collider, hit) = qp.cast_shape(shape_pos, dir, &shape, options)?;

        let object_id = self.collider_set.get(collider)?.user_data as u64;
        let object = GameObjectId::from_ffi(object_id);

        object.exists().then_some((hit, object))
    }

    pub fn cursor_ray(&self, world: &World) -> Option<Ray> {
        let cursor_pos = world.input.mouse_position();
        world
            .active_camera()
            .upgrade(world)
            .map(|cam| cam.click_ray(cursor_pos.x, cursor_pos.y))
    }

    pub fn cast_cursor_ray(
        &self,
        world: &World,
        max_toi: f32,
        solid: bool,
        filter: QueryFilter,
    ) -> Option<(f32, GameObjectId)> {
        let ray = self.cursor_ray(world)?;
        self.cast_ray(&ray, max_toi, solid, filter)
    }
}
