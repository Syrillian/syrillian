use crate::core::Transform;
use crate::math::Mat4;
use syrillian_render::rendering::render_data::CameraUniform;

pub trait CameraUniformHelper {
    fn update_with_transform(&mut self, proj_matrix: &Mat4, cam_transform: &Transform);
}

impl CameraUniformHelper for CameraUniform {
    fn update_with_transform(&mut self, proj_matrix: &Mat4, cam_transform: &Transform) {
        let pos = cam_transform.position();
        let view_mat: Mat4 = cam_transform.affine().inverse().into();

        self.update(proj_matrix, &pos, &view_mat);
    }
}
