use self::ColliderError::{DesyncedCollider, InvalidMesh, InvalidMeshRef, NoMeshRenderer};
use crate::World;
use crate::components::{Component, MeshRenderer, RigidBodyComponent};
use crate::core::GameObjectId;
use crate::engine::assets::{HMesh, Mesh};
use nalgebra::{Point3, Vector3};
use rapier3d::prelude::*;
use snafu::Snafu;
use tracing::{trace, warn};

#[cfg(debug_assertions)]
use crate::assets::StoreType;
#[cfg(debug_assertions)]
use crate::core::Vertex3D;
#[cfg(debug_assertions)]
use crate::proxy_data_mut;
#[cfg(debug_assertions)]
use crate::rendering::proxies::DebugSceneProxy;
#[cfg(debug_assertions)]
use crate::rendering::proxies::SceneProxy;
#[cfg(debug_assertions)]
use crate::rendering::{CPUDrawCtx, DebugRenderer};
#[cfg(debug_assertions)]
use nalgebra::{Matrix4, Vector4};
use syrillian_macros::Reflect;
use syrillian_utils::debug_panic;

#[derive(Debug, Reflect)]
pub struct Collider3D {
    pub phys_handle: Option<ColliderHandle>,
    linked_to_body: Option<RigidBodyHandle>,
    shape_kind: ColliderShapeKind,
    last_scale: Vector3<f32>,

    #[cfg(debug_assertions)]
    enable_debug_render: bool, // TODO: Sync with GPU
    #[cfg(debug_assertions)]
    debug_collider_mesh: Option<HMesh>,
    #[cfg(debug_assertions)]
    was_debug_enabled: bool,
}

#[derive(Debug, Clone)]
enum ColliderShapeKind {
    Cuboid,
    Mesh(HMesh),
}

#[derive(Debug, Snafu)]
#[snafu(context(suffix(Err)))]
pub enum ColliderError {
    #[snafu(display(
        "Cannot use Mesh as Collider since no MeshRenderer is attached to the Object"
    ))]
    NoMeshRenderer,

    #[snafu(display("A mesh renderer was storing an invalid mesh reference"))]
    InvalidMeshRef,

    #[snafu(display("No collider was attached to the object"))]
    DesyncedCollider,

    #[snafu(display("The collider mesh was invalid"))]
    InvalidMesh,
}

impl Default for Collider3D {
    fn default() -> Self {
        Collider3D {
            phys_handle: None,
            linked_to_body: None,
            shape_kind: ColliderShapeKind::Cuboid,
            last_scale: Vector3::new(1.0, 1.0, 1.0),

            #[cfg(debug_assertions)]
            enable_debug_render: true,
            #[cfg(debug_assertions)]
            debug_collider_mesh: None,
            #[cfg(debug_assertions)]
            was_debug_enabled: true,
        }
    }
}

impl Component for Collider3D {
    fn init(&mut self, world: &mut World) {
        let parent = self.parent();
        let scale = Collider3D::sanitize_scale(parent.transform.scale());
        let shape = Self::build_cuboid_shape(scale);
        let collider = Self::default_collider(parent, shape);
        let phys_handle = world.physics.collider_set.insert(collider.clone());

        self.phys_handle = Some(phys_handle);
        self.sync_with_transform_world(world, true);
    }
    #[cfg(debug_assertions)]
    fn update(&mut self, world: &mut World) {
        if self.debug_collider_mesh.is_none() {
            trace!("[Collider] Regenerating debug mesh");
            let mesh = self.generate_collider_mesh(world);
            self.debug_collider_mesh = Some(mesh);
        }
    }

    fn fixed_update(&mut self, world: &mut World) {
        if let Some(body_comp) = (*self.parent()).get_component::<RigidBodyComponent>()
            && self.linked_to_body.is_none()
            && let Some(body_handle) = body_comp.body_handle
        {
            self.link_to_rigid_body(world, Some(body_handle));

            if let Some(collider) = world.physics.collider_set.get_mut(self.handle()) {
                collider.set_translation(Vector3::identity());
                collider.set_rotation(Rotation::identity());
            }
        }

        self.sync_with_transform_world(world, false);
    }

    #[cfg(debug_assertions)]
    fn create_render_proxy(&mut self, _world: &World) -> Option<Box<dyn SceneProxy>> {
        let Some(mesh) = self.debug_collider_mesh else {
            debug_panic!("Debug mode is enabled but no collider mesh was made in update");
            return None;
        };

        const COLOR: Vector4<f32> = Vector4::new(0.0, 1.0, 0.2, 1.0);

        let transform = self.collider_debug_transform();
        let mut proxy = Box::new(DebugSceneProxy::single_mesh(mesh));
        proxy.color = COLOR;
        proxy.set_override_transform(transform);
        Some(proxy)
    }

    #[cfg(debug_assertions)]
    fn update_proxy(&mut self, _world: &World, mut ctx: CPUDrawCtx) {
        if !DebugRenderer::collider_mesh() && self.was_debug_enabled {
            ctx.disable_proxy();
            self.was_debug_enabled = false;
        } else if DebugRenderer::collider_mesh() && !self.was_debug_enabled {
            ctx.enable_proxy();
            self.was_debug_enabled = true;
        }

        if DebugRenderer::collider_mesh() {
            let transform = self.collider_debug_transform();
            ctx.send_proxy_update(move |proxy| {
                let proxy: &mut DebugSceneProxy = proxy_data_mut!(proxy);
                proxy.set_override_transform(transform);
            });
        }
    }

    fn delete(&mut self, world: &mut World) {
        world.physics.collider_set.remove(
            self.handle(),
            &mut world.physics.island_manager,
            &mut world.physics.rigid_body_set,
            false,
        );
    }
}

impl Collider3D {
    fn sanitize_scale(scale: Vector3<f32>) -> Vector3<f32> {
        Vector3::new(
            scale.x.abs().max(f32::EPSILON),
            scale.y.abs().max(f32::EPSILON),
            scale.z.abs().max(f32::EPSILON),
        )
    }

    fn build_cuboid_shape(scale: Vector3<f32>) -> SharedShape {
        SharedShape::cuboid(scale.x * 0.5, scale.y * 0.5, scale.z * 0.5)
    }

    fn build_shape_for_scale_world(
        &self,
        world: &World,
        scale: Vector3<f32>,
    ) -> Option<SharedShape> {
        match &self.shape_kind {
            ColliderShapeKind::Cuboid => Some(Self::build_cuboid_shape(scale)),
            ColliderShapeKind::Mesh(handle) => {
                let mesh = world.assets.meshes.try_get(*handle)?;
                SharedShape::mesh_with_scale(&mesh, scale)
            }
        }
    }

    fn sync_with_transform_world(&mut self, world: &mut World, force_pose: bool) {
        let scale = self.parent().transform.scale();
        let new_shape = ((scale - self.last_scale).norm() > f32::EPSILON)
            .then(|| self.build_shape_for_scale_world(world, scale));

        let Some(collider) = self.collider_mut() else {
            debug_panic!("[Collider] No collider found when trying to sync with world transform");
            return;
        };
        if let Some(new_shape) = new_shape {
            let Some(shape) = new_shape else {
                debug_panic!("[Collider] Couldn't make shape");
                return;
            };

            collider.set_shape(shape);
        }

        if force_pose || self.linked_to_body.is_none() {
            collider.set_translation(self.parent().transform.position());
            collider.set_rotation(self.parent().transform.rotation());
        }

        self.last_scale = scale;
    }

    pub fn collider(&self) -> Option<&Collider> {
        World::instance().physics.collider_set.get(self.handle())
    }

    pub fn collider_mut(&self) -> Option<&mut Collider> {
        World::instance()
            .physics
            .collider_set
            .get_mut(self.handle())
    }

    fn default_collider(parent: GameObjectId, shape: SharedShape) -> Collider {
        ColliderBuilder::new(shape)
            .density(1.0)
            .friction(0.999)
            .user_data(parent.as_ffi() as u128)
            .build()
    }

    pub fn link_to_rigid_body(&mut self, world: &mut World, h_body: Option<RigidBodyHandle>) {
        world.physics.collider_set.set_parent(
            self.handle(),
            h_body,
            &mut world.physics.rigid_body_set,
        );

        self.linked_to_body = h_body;

        let force_pose = self.linked_to_body.is_none();
        self.sync_with_transform_world(world, force_pose);
    }

    pub fn use_mesh(&mut self) {
        if let Err(e) = self.try_use_mesh() {
            warn!("{e}");
        }
    }

    /// Same as Collider3D::use_mesh but without a warning. This is nice for guarantee-less iteration
    pub fn please_use_mesh(&mut self) {
        _ = self.try_use_mesh();
    }

    pub fn try_use_mesh(&mut self) -> Result<(), ColliderError> {
        let parent = self.parent();
        let world = self.world();

        let mesh_renderer = parent
            .get_component::<MeshRenderer>()
            .ok_or(NoMeshRenderer)?;

        let handle = mesh_renderer.mesh();
        let scale = Self::sanitize_scale(self.parent().transform.scale());
        let shape = {
            let mesh = world.assets.meshes.try_get(handle).ok_or(InvalidMeshRef)?;
            SharedShape::mesh_with_scale(&mesh, scale).ok_or(InvalidMesh)?
        };

        world
            .physics
            .collider_set
            .get_mut(self.handle())
            .ok_or(DesyncedCollider)?
            .set_shape(shape);

        self.shape_kind = ColliderShapeKind::Mesh(handle);
        self.last_scale = scale;

        #[cfg(debug_assertions)]
        {
            self.debug_collider_mesh = None;
        }

        self.sync_with_transform_world(world, self.linked_to_body.is_none());

        Ok(())
    }

    #[cfg(debug_assertions)]
    pub fn set_local_debug_render_enabled(&mut self, enabled: bool) {
        self.enable_debug_render = enabled;
    }

    #[cfg(debug_assertions)]
    pub fn is_local_debug_render_enabled(&self) -> bool {
        self.enable_debug_render
    }

    #[cfg(debug_assertions)]
    fn generate_collider_mesh(&mut self, world: &mut World) -> HMesh {
        let Some(collider) = self.collider() else {
            debug_panic!("No collider attached to Collider 3D component");
            return HMesh::UNIT_CUBE;
        };

        let (vertices, indices) = collider.shared_shape().to_trimesh();
        let vertices: Vec<_> = vertices
            .iter()
            .map(|v| Vertex3D::position_only(v.coords))
            .collect();

        Mesh::builder(vertices)
            .with_indices(indices.into_flattened())
            .build()
            .store(world)
    }

    #[cfg(debug_assertions)]
    fn collider_debug_transform(&self) -> Matrix4<f32> {
        if let Some(collider) = self.collider() {
            let iso = collider.position();
            let mut mat = Matrix4::identity();
            let rot = iso.rotation.to_rotation_matrix().into_inner();
            mat.fixed_view_mut::<3, 3>(0, 0).copy_from(&rot);
            mat[(0, 3)] = iso.translation.vector.x;
            mat[(1, 3)] = iso.translation.vector.y;
            mat[(2, 3)] = iso.translation.vector.z;
            mat
        } else {
            self.parent()
                .transform
                .global_transform_matrix()
                .to_homogeneous()
        }
    }

    fn handle(&self) -> ColliderHandle {
        self.phys_handle
            .expect("Handle should be initialized in init")
    }
}

pub trait MeshShapeExtra<T> {
    fn mesh(mesh: &Mesh) -> Option<T>;
    fn mesh_with_scale(mesh: &Mesh, scale: Vector3<f32>) -> Option<T>;
    fn mesh_convex_hull(mesh: &Mesh) -> Option<SharedShape>;
    fn local_aabb_mesh(&self) -> (Vec<Point3<f32>>, Vec<[u32; 3]>);
    fn to_trimesh(&self) -> (Vec<Point3<f32>>, Vec<[u32; 3]>);
}

impl MeshShapeExtra<SharedShape> for SharedShape {
    fn mesh(mesh: &Mesh) -> Option<SharedShape> {
        trace!(
            "Loading collider mesh with {} vertices",
            mesh.data.vertices.len()
        );

        if mesh.triangle_count() == 0 {
            return None;
        }

        let vertices = mesh.data.make_point_cloud();
        let indices = mesh.data.make_triangle_indices();
        match SharedShape::trimesh(vertices, indices) {
            Ok(shape) => Some(shape),
            Err(e) => {
                warn!("Mesh could not be processed as a trimesh: {e}");
                None
            }
        }
    }

    fn mesh_with_scale(mesh: &Mesh, scale: Vector3<f32>) -> Option<SharedShape> {
        trace!(
            "Loading scaled collider mesh with {} vertices",
            mesh.data.vertices.len()
        );

        if mesh.triangle_count() == 0 {
            return None;
        }

        let mut vertices = mesh.data.make_point_cloud();
        for v in &mut vertices {
            v.coords.x *= scale.x;
            v.coords.y *= scale.y;
            v.coords.z *= scale.z;
        }
        let indices = mesh.data.make_triangle_indices();
        match SharedShape::trimesh(vertices, indices) {
            Ok(shape) => Some(shape),
            Err(e) => {
                warn!("Scaled mesh could not be processed as a trimesh: {e}");
                None
            }
        }
    }

    fn mesh_convex_hull(mesh: &Mesh) -> Option<SharedShape> {
        let vertices = mesh.data.make_point_cloud();
        SharedShape::convex_hull(&vertices)
    }

    fn local_aabb_mesh(&self) -> (Vec<Point3<f32>>, Vec<[u32; 3]>) {
        let aabb = self.compute_local_aabb();
        aabb.to_trimesh()
    }

    fn to_trimesh(&self) -> (Vec<Point3<f32>>, Vec<[u32; 3]>) {
        trace!("[Collider] Type: {:?}", self.as_typed_shape());
        match self.as_typed_shape() {
            TypedShape::Ball(s) => s.to_trimesh(10, 10),
            TypedShape::Cuboid(s) => s.to_trimesh(),
            TypedShape::Capsule(s) => s.to_trimesh(10, 10),
            TypedShape::Segment(_) => self.local_aabb_mesh(),
            TypedShape::Triangle(s) => (s.vertices().to_vec(), vec![[0, 1, 2]]),
            TypedShape::Voxels(s) => s.to_trimesh(),
            TypedShape::TriMesh(s) => (s.vertices().to_vec(), s.indices().to_vec()),
            TypedShape::Polyline(_) => self.local_aabb_mesh(),
            TypedShape::HalfSpace(_) => self.local_aabb_mesh(),
            TypedShape::HeightField(s) => s.to_trimesh(),
            TypedShape::Compound(_) => self.local_aabb_mesh(),
            TypedShape::ConvexPolyhedron(s) => s.to_trimesh(),
            TypedShape::Cylinder(s) => s.to_trimesh(10),
            TypedShape::Cone(s) => s.to_trimesh(10),
            TypedShape::RoundCuboid(_) => self.local_aabb_mesh(),
            TypedShape::RoundTriangle(_) => self.local_aabb_mesh(),
            TypedShape::RoundCylinder(_) => self.local_aabb_mesh(),
            TypedShape::RoundCone(_) => self.local_aabb_mesh(),
            TypedShape::RoundConvexPolyhedron(_) => self.local_aabb_mesh(),
            TypedShape::Custom(_) => self.local_aabb_mesh(),
        }
    }
}
