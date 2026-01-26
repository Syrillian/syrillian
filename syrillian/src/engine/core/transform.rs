use crate::core::GameObjectId;
use nalgebra::{Affine3, Isometry3, Point, Scale3, Translation3, UnitQuaternion, Vector3};
use num_traits::AsPrimitive;
use syrillian_macros::Reflect;

/// Stores the translation, rotation and scale of a [`GameObject`](crate::core::GameObject).
///
/// The transform keeps precomputed matrices for each component so that
/// operations such as retrieving the final model matrix are fast.
#[repr(C)]
#[derive(Reflect)]
#[reflect_all]
pub struct Transform {
    #[dont_reflect]
    pub(crate) owner: GameObjectId,

    pos: Vector3<f32>,
    rot: UnitQuaternion<f32>,
    scale: Vector3<f32>,
    pos_mat: Translation3<f32>,
    scale_mat: Scale3<f32>,
    compound_mat: Affine3<f32>,
    invert_position: bool,
    compound_pos_first: bool,

    #[dont_reflect]
    is_dirty: bool,
}

#[allow(dead_code)]
impl Transform {
    /// Creates a new [`Transform`] owned by the given [`GameObjectId`].
    ///
    /// The transform starts at the origin with no rotation and a uniform scale
    /// of `1.0`.
    pub fn new(owner: GameObjectId) -> Self {
        Transform {
            owner,

            pos: Vector3::zeros(),
            rot: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
            pos_mat: Translation3::identity(),
            scale_mat: Scale3::identity(),
            compound_mat: Affine3::identity(),
            invert_position: false,
            compound_pos_first: true,

            is_dirty: true,
        }
    }

    pub(crate) fn clone(&self, owner: GameObjectId) -> Self {
        Transform {
            owner,

            pos: self.pos,
            rot: self.rot,
            scale: self.scale,
            pos_mat: self.pos_mat,
            scale_mat: self.scale_mat,
            compound_mat: self.compound_mat,
            invert_position: self.invert_position,
            compound_pos_first: self.compound_pos_first,

            is_dirty: self.is_dirty,
        }
    }

    /// Sets the global position of the transform.
    #[inline(always)]
    pub fn set_position(
        &mut self,
        x: impl AsPrimitive<f32>,
        y: impl AsPrimitive<f32>,
        z: impl AsPrimitive<f32>,
    ) {
        self.set_position_vec(Vector3::new(x.as_(), y.as_(), z.as_()))
    }

    /// Sets the global position using a vector.
    pub fn set_position_vec(&mut self, pos: Vector3<f32>) {
        let mat = self.global_transform_matrix_ext(false);
        self.set_local_position_vec(mat.inverse_transform_vector(&pos)); // FIXME: transform point?
    }

    /// Returns the global position of the transform.
    pub fn position(&self) -> Vector3<f32> {
        self.global_transform_matrix()
            .transform_point(&Point::default())
            .coords
    }

    fn owner(&self) -> GameObjectId {
        self.owner
    }

    pub fn global_transform_matrix_ext(&self, include_self: bool) -> Affine3<f32> {
        let mut mat = Affine3::identity();
        let mut parents = self.owner().parents();

        if !include_self {
            parents.pop();
        }

        for parent in parents {
            mat *= parent.transform.compound_mat;
        }
        mat
    }

    /// Global rigid transform (rotation+translation only), ignoring scale.
    pub fn rigid_global_isometry(&self) -> Isometry3<f32> {
        let p = self.position();
        let r = self.rotation();
        Isometry3::from_parts(Translation3::from(p), r)
    }

    /// View matrix for cameras/lights: inverse of the rigid global isometry.
    pub fn view_matrix_rigid(&self) -> Isometry3<f32> {
        self.rigid_global_isometry().inverse()
    }

    /// Returns the global model matrix for this transform.
    pub fn global_transform_matrix(&self) -> Affine3<f32> {
        self.global_transform_matrix_ext(true)
    }

    /// Calculates the global rotation, optionally excluding this transform.
    pub fn global_rotation_ext(&self, include_self: bool) -> UnitQuaternion<f32> {
        let mut global_rotation = UnitQuaternion::identity();
        let mut parents = self.owner().parents();

        if !include_self {
            parents.pop();
        }

        for parent in parents {
            global_rotation *= parent.transform.rot;
        }
        global_rotation
    }

    /// Calculates the global scale matrix, optionally excluding this transform.
    pub fn global_scale_matrix_ext(&self, include_self: bool) -> Scale3<f32> {
        let mut mat = Scale3::identity();
        let mut parents = self.owner().parents();

        if !include_self {
            parents.pop();
        }

        for parent in parents {
            mat *= parent.transform.scale_mat;
        }
        mat
    }

    /// Returns the global scale matrix for this transform.
    pub fn global_scale_matrix(&self) -> Scale3<f32> {
        self.global_scale_matrix_ext(true)
    }

    /// Sets the local position of the transform.
    #[inline]
    pub fn set_local_position(&mut self, x: f32, y: f32, z: f32) {
        let position = Vector3::new(x, y, z);
        self.set_local_position_vec(position);
    }

    /// Sets the local position using a vector.
    pub fn set_local_position_vec(&mut self, position: Vector3<f32>) {
        self.pos = position;
        self.recalculate_pos_matrix();
    }

    /// Returns a reference to the local position vector.
    pub fn local_position(&self) -> &Vector3<f32> {
        &self.pos
    }

    /// Inverts the sign of the position when true.
    pub fn set_invert_position(&mut self, invert: bool) {
        self.invert_position = invert;
    }

    /// Adds the given offset to the local position.
    pub fn translate(&mut self, other: Vector3<f32>) {
        self.pos += other;
        self.recalculate_pos_matrix();
    }

    /// Sets the local model-space rotation of this transform
    pub fn set_local_rotation(&mut self, rotation: UnitQuaternion<f32>) {
        self.rot = rotation;
        self.recalculate_combined_matrix()
    }

    /// Returns a reference to the local rotation quaternion.
    pub fn local_rotation(&self) -> &UnitQuaternion<f32> {
        &self.rot
    }

    /// Sets the global rotation of the transform in euler angles.
    /// This will do the transformation to quaternions for you, but it's recommended to use quaternions.
    pub fn set_euler_rotation_deg(
        &mut self,
        roll: impl AsPrimitive<f32>,
        pitch: impl AsPrimitive<f32>,
        yaw: impl AsPrimitive<f32>,
    ) {
        self.set_euler_rotation_rad(
            roll.as_().to_radians(),
            pitch.as_().to_radians(),
            yaw.as_().to_radians(),
        );
    }

    /// Sets the global rotation of the transform in euler angles.
    /// This will do the transformation to quaternions for you, but it's recommended to use quaternions.
    pub fn set_euler_rotation_rad(
        &mut self,
        roll: impl AsPrimitive<f32>,
        pitch: impl AsPrimitive<f32>,
        yaw: impl AsPrimitive<f32>,
    ) {
        let parent_global_rotation = self.global_rotation_ext(false);
        let target = UnitQuaternion::from_euler_angles(roll.as_(), pitch.as_(), yaw.as_());

        let local_rotation_change = parent_global_rotation.rotation_to(&target);
        self.set_local_rotation(local_rotation_change);
    }

    pub fn set_euler_rotation_deg_vec(&mut self, euler_rot: Vector3<impl AsPrimitive<f32>>) {
        self.set_euler_rotation_deg(euler_rot[0], euler_rot[1], euler_rot[2]);
    }

    pub fn set_euler_rotation_rad_vec(&mut self, euler_rot_rad: Vector3<impl AsPrimitive<f32>>) {
        self.set_euler_rotation_rad(euler_rot_rad[0], euler_rot_rad[1], euler_rot_rad[2]);
    }

    /// Sets the global rotation of the transform.
    pub fn set_rotation(&mut self, rotation: UnitQuaternion<f32>) {
        let parent_global_rotation = self.global_rotation_ext(false);
        let local_rotation_change = parent_global_rotation.rotation_to(&rotation);

        self.set_local_rotation(local_rotation_change);
    }

    /// Returns the global rotation quaternion.
    pub fn rotation(&self) -> UnitQuaternion<f32> {
        self.global_rotation_ext(true)
    }

    /// Returns the global rotation euler angles
    pub fn euler_rotation(&self) -> Vector3<f32> {
        let (x, y, z) = self.global_rotation_ext(true).euler_angles();
        Vector3::new(x, y, z)
    }

    pub fn local_euler_rotation(&self) -> Vector3<f32> {
        let (x, y, z) = self.local_rotation().euler_angles();
        Vector3::new(x, y, z)
    }

    /// Applies a relative rotation to the transform.
    pub fn rotate(&mut self, rot: UnitQuaternion<f32>) {
        self.rot *= rot;
        self.recalculate_combined_matrix();
    }

    /// Sets the local scale using three independent factors.
    pub fn set_nonuniform_local_scale(&mut self, scale: Vector3<f32>) {
        self.scale.x = scale.x.abs().max(f32::EPSILON);
        self.scale.y = scale.y.abs().max(f32::EPSILON);
        self.scale.z = scale.z.abs().max(f32::EPSILON);
        self.recalculate_scale_matrix();
    }

    /// Sets the local scale uniformly.
    pub fn set_uniform_local_scale(&mut self, factor: f32) {
        self.set_nonuniform_local_scale(Vector3::new(factor, factor, factor));
    }

    /// Returns a reference to the local scale vector.
    pub fn local_scale(&self) -> &Vector3<f32> {
        &self.scale
    }

    /// Sets the global scale, preserving the current global orientation.
    pub fn set_nonuniform_scale(&mut self, x: f32, y: f32, z: f32) {
        self.set_nonuniform_scale_vec(Vector3::new(x, y, z));
    }

    /// Sets the global scale, preserving the current global orientation.
    pub fn set_nonuniform_scale_vec(&mut self, scale: Vector3<f32>) {
        let global_scale = self.scale();
        let scale_delta = scale.component_div(&global_scale);
        let new_local_scale = self.scale.component_mul(&scale_delta);

        self.set_nonuniform_local_scale(new_local_scale);
    }

    /// Sets the global scale uniformly.
    pub fn set_scale(&mut self, factor: f32) {
        self.set_nonuniform_scale_vec(Vector3::new(factor, factor, factor));
    }

    /// Returns the global scale factors.
    pub fn scale(&self) -> Vector3<f32> {
        let global_scale = self.global_scale_matrix();
        global_scale.vector
    }

    /// Recalculates all cached matrices.
    pub fn regenerate_matrices(&mut self) {
        self.recalculate_pos_matrix();
        self.recalculate_scale_matrix();
        self.recalculate_combined_matrix();
    }

    fn recalculate_pos_matrix(&mut self) {
        let pos = if self.invert_position {
            -self.pos
        } else {
            self.pos
        };
        self.pos_mat = Translation3::from(pos);
        self.recalculate_combined_matrix()
    }

    fn recalculate_scale_matrix(&mut self) {
        self.scale_mat = Scale3::from(self.scale);
        self.recalculate_combined_matrix()
    }

    fn recalculate_combined_matrix(&mut self) {
        self.compound_mat = Affine3::from_matrix_unchecked(
            self.pos_mat.to_homogeneous()
                * self.rot.to_homogeneous()
                * self.scale_mat.to_homogeneous(),
        );

        self.set_dirty();

        debug_assert_ne!(0.0, self.compound_mat.matrix().determinant());
    }

    pub fn translation(&self) -> &Translation3<f32> {
        &self.pos_mat
    }

    /// Returns a reference to the combined transformation matrix.
    pub fn full_matrix(&self) -> &Affine3<f32> {
        &self.compound_mat
    }

    /// Returns the forward direction in world space.
    pub fn forward(&self) -> Vector3<f32> {
        self.rotation() * Vector3::new(0.0, 0.0, -1.0)
    }

    /// Returns the right direction in world space.
    pub fn right(&self) -> Vector3<f32> {
        self.rotation() * Vector3::new(1.0, 0.0, 0.0)
    }

    /// Returns the up direction in world space.
    pub fn up(&self) -> Vector3<f32> {
        self.rotation() * Vector3::new(0.0, 1.0, 0.0)
    }

    /// Returns the forward direction relative to the parent.
    pub fn local_forward(&self) -> Vector3<f32> {
        self.local_rotation() * Vector3::new(0.0, 0.0, -1.0)
    }

    /// Returns the right direction relative to the parent.
    pub fn local_right(&self) -> Vector3<f32> {
        self.local_rotation() * Vector3::new(1.0, 0.0, 0.0)
    }

    /// Returns the up direction relative to the parent.
    pub fn local_up(&self) -> Vector3<f32> {
        self.local_rotation() * Vector3::new(0.0, 1.0, 0.0)
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn set_dirty(&mut self) {
        self.is_dirty = true;

        if !self.owner().exists() {
            return;
        }

        for mut child in self.owner().children().iter().copied() {
            child.transform.set_dirty();
        }
    }

    pub fn clear_dirty(&mut self) {
        self.is_dirty = false;
    }
}
