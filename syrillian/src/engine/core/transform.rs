use crate::core::GameObjectId;
use crate::math::{Affine3A, EulerRot, Quat, Vec3};
use crate::utils::QuaternionEuler;
use num_traits::AsPrimitive;
use syrillian::math::Pose;
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

    pos: Vec3,
    rot: Quat,
    scale: Vec3,
    compound_mat: Affine3A,
    invert_position: bool,

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

            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            scale: Vec3::ONE,
            compound_mat: Affine3A::IDENTITY,
            invert_position: false,

            is_dirty: true,
        }
    }

    pub(crate) fn clone(&self, owner: GameObjectId) -> Self {
        Transform {
            owner,

            pos: self.pos,
            rot: self.rot,
            scale: self.scale,
            compound_mat: self.compound_mat,
            invert_position: self.invert_position,

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
        self.set_position_vec(Vec3::new(x.as_(), y.as_(), z.as_()))
    }

    /// Sets the global position using a vector.
    pub fn set_position_vec(&mut self, pos: Vec3) {
        let mat = self.affine_ext(false);
        self.set_local_position_vec(mat.inverse().transform_vector3(pos)); // FIXME: transform point?
    }

    fn owner(&self) -> GameObjectId {
        self.owner
    }

    pub fn affine_ext(&self, include_self: bool) -> Affine3A {
        let mut mat = Affine3A::IDENTITY;
        let mut parents = self.owner().parents();

        if !include_self {
            parents.pop();
        }

        for parent in parents {
            mat *= parent.transform.compound_mat;
        }
        mat
    }

    pub fn affine(&self) -> Affine3A {
        self.affine_ext(true)
    }

    /// Global rigid transform (rotation+translation only), ignoring scale.
    pub fn rigid_global_isometry(&self) -> Pose {
        let (_, r, t) = self.affine().to_scale_rotation_translation();
        Pose::from_parts(t, r)
    }

    /// View matrix for cameras/lights: inverse of the rigid global isometry.
    pub fn view_matrix_rigid(&self) -> Pose {
        self.rigid_global_isometry().inverse()
    }

    /// Returns the global model matrix for this transform.
    pub fn position(&self) -> Vec3 {
        self.affine().transform_point3(Vec3::ZERO)
    }

    /// Calculates the global rotation, optionally excluding this transform.
    pub fn global_rotation_ext(&self, include_self: bool) -> Quat {
        let mut global_rotation = Quat::IDENTITY;
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
    pub fn global_scale_ext(&self, include_self: bool) -> Vec3 {
        let mut scale = Vec3::ONE;
        let mut parents = self.owner().parents();

        if !include_self {
            parents.pop();
        }

        for parent in parents {
            scale *= parent.transform.scale;
        }
        scale
    }

    /// Sets the local position of the transform.
    #[inline]
    pub fn set_local_position(&mut self, x: f32, y: f32, z: f32) {
        self.set_local_position_vec((x, y, z).into());
    }

    /// Sets the local position using a vector.
    pub fn set_local_position_vec(&mut self, position: Vec3) {
        self.pos = position;
        self.refresh_compound();
    }

    /// Returns a reference to the local position vector.
    pub fn local_position(&self) -> &Vec3 {
        &self.pos
    }

    /// Inverts the sign of the position when true.
    pub fn set_invert_position(&mut self, invert: bool) {
        self.invert_position = invert;
        self.refresh_compound()
    }

    /// Adds the given offset to the local position.
    pub fn translate(&mut self, other: Vec3) {
        self.pos += other;
        self.refresh_compound();
    }

    /// Sets the local model-space rotation of this transform
    pub fn set_local_rotation(&mut self, rotation: Quat) {
        self.rot = rotation;
        self.refresh_compound()
    }

    /// Returns a reference to the local rotation quaternion.
    pub fn local_rotation(&self) -> &Quat {
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
        let target = Quat::from_euler(EulerRot::XYZ, roll.as_(), pitch.as_(), yaw.as_());
        self.set_rotation(target);
    }

    pub fn set_euler_rotation_deg_vec(&mut self, euler_rot: Vec3) {
        self.set_euler_rotation_deg(euler_rot[0], euler_rot[1], euler_rot[2]);
    }

    pub fn set_euler_rotation_rad_vec(&mut self, euler_rot_rad: Vec3) {
        self.set_euler_rotation_rad(euler_rot_rad[0], euler_rot_rad[1], euler_rot_rad[2]);
    }

    /// Sets the global rotation of the transform.
    pub fn set_rotation(&mut self, target: Quat) {
        let parent_global_rotation = self.global_rotation_ext(false);
        let local_rotation_change = target * parent_global_rotation.inverse();

        self.set_local_rotation(local_rotation_change);
    }

    /// Returns the global rotation quaternion.
    pub fn rotation(&self) -> Quat {
        self.global_rotation_ext(true)
    }

    /// Returns the global rotation euler angles
    pub fn euler_rotation(&self) -> Vec3 {
        self.global_rotation_ext(true)
            .to_euler(EulerRot::XYZ)
            .into()
    }

    pub fn local_euler_rotation(&self) -> Vec3 {
        self.local_rotation().euler_vector()
    }

    /// Applies a relative rotation to the transform.
    pub fn rotate(&mut self, rot: Quat) {
        self.rot *= rot;
        self.refresh_compound();
    }

    /// Sets the local scale using three independent factors.
    pub fn set_nonuniform_local_scale(&mut self, scale: Vec3) {
        self.scale.x = scale.x.abs().max(f32::EPSILON);
        self.scale.y = scale.y.abs().max(f32::EPSILON);
        self.scale.z = scale.z.abs().max(f32::EPSILON);
        self.refresh_compound();
    }

    /// Sets the local scale uniformly.
    pub fn set_uniform_local_scale(&mut self, factor: f32) {
        self.set_nonuniform_local_scale(Vec3::splat(factor));
    }

    /// Returns a reference to the local scale vector.
    pub fn local_scale(&self) -> &Vec3 {
        &self.scale
    }

    /// Sets the global scale, preserving the current global orientation.
    pub fn set_nonuniform_scale(&mut self, x: f32, y: f32, z: f32) {
        self.set_nonuniform_scale_vec((x, y, z).into());
    }

    /// Sets the global scale, preserving the current global orientation.
    pub fn set_nonuniform_scale_vec(&mut self, scale: Vec3) {
        let global_scale = self.scale();
        let scale_delta = scale / global_scale;
        let new_local_scale = self.scale * scale_delta;

        self.set_nonuniform_local_scale(new_local_scale);
    }

    /// Sets the global scale uniformly.
    pub fn set_scale(&mut self, factor: f32) {
        self.set_nonuniform_scale_vec(Vec3::splat(factor));
    }

    /// Returns the global scale factors.
    #[inline]
    pub fn scale(&self) -> Vec3 {
        self.global_scale_ext(true)
    }

    /// Recalculates all cached matrices.
    pub fn refresh_compound(&mut self) {
        let pos = if self.invert_position {
            -self.pos
        } else {
            self.pos
        };
        self.compound_mat = Affine3A::from_scale_rotation_translation(self.scale, self.rot, pos);

        self.set_dirty();
    }

    pub fn local_translation(&self) -> Vec3 {
        self.pos
    }

    /// Returns a reference to the combined transformation matrix.
    pub fn full_matrix(&self) -> &Affine3A {
        &self.compound_mat
    }

    /// Returns the forward direction in world space.
    pub fn forward(&self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }

    /// Returns the right direction in world space.
    pub fn right(&self) -> Vec3 {
        self.rotation() * Vec3::X
    }

    /// Returns the up direction in world space.
    pub fn up(&self) -> Vec3 {
        self.rotation() * Vec3::Y
    }

    /// Returns the forward direction relative to the parent.
    pub fn local_forward(&self) -> Vec3 {
        self.local_rotation() * Vec3::NEG_Z
    }

    /// Returns the right direction relative to the parent.
    pub fn local_right(&self) -> Vec3 {
        self.local_rotation() * Vec3::X
    }

    /// Returns the up direction relative to the parent.
    pub fn local_up(&self) -> Vec3 {
        self.local_rotation() * Vec3::Y
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
