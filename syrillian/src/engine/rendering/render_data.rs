use crate::core::Transform;
use crate::ensure_aligned;
use crate::math::{Mat4, UVec2, Vec3};
use crate::rendering::Frustum;
use crate::rendering::lights::LightProxy;
use crate::rendering::uniform::ShaderUniform;
use std::f32::consts::FRAC_PI_2;
use syrillian_macros::UniformIndex;
use wgpu::{BindGroupLayout, Device, Queue};

// TODO: Use proper matrix types (Affine3, Perspective3)
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub pos: Vec3,
    pub fov: f32,
    pub view_mat: Mat4,
    pub projection_mat: Mat4,
    pub proj_view_mat: Mat4,
    pub inv_proj_view_mat: Mat4,
    pub near: f32,
    pub far: f32,
    pub fov_target: f32,
    pub zoom_speed: f32,
}

ensure_aligned!(
    CameraUniform {
        pos,
        view_mat,
        projection_mat,
        proj_view_mat,
        inv_proj_view_mat
    },
    align <= 16 * 18 => size
);

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SystemUniform {
    pub(crate) screen_size: UVec2,
    pub(crate) time: f32,
    pub(crate) delta_time: f32,
}

ensure_aligned!(SystemUniform { screen_size }, align <= 8 * 2 => size);

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum RenderUniformIndex {
    Camera = 0,
    System = 1,
}

pub struct RenderUniformData {
    pub camera_data: CameraUniform,
    pub system_data: SystemUniform,
    pub uniform: ShaderUniform<RenderUniformIndex>,
}

impl Default for CameraUniform {
    fn default() -> Self {
        let projection_mat = Mat4::perspective_rh(60.0, 1.0, 0.1, 1000.0);
        let proj_view_mat = projection_mat; // identity matrix for view_mat so it's the same
        CameraUniform {
            pos: Vec3::ZERO,
            fov: 60.0,
            view_mat: Mat4::IDENTITY,
            projection_mat,
            proj_view_mat,
            inv_proj_view_mat: Mat4::IDENTITY,
            near: 0.1,
            far: 1000.0,
            fov_target: 60.0,
            zoom_speed: 1.0,
        }
    }
}

impl CameraUniform {
    pub const fn empty() -> Self {
        CameraUniform {
            pos: Vec3::ZERO,
            fov: 60.0,
            view_mat: Mat4::IDENTITY,
            projection_mat: Mat4::IDENTITY,
            proj_view_mat: Mat4::IDENTITY,
            inv_proj_view_mat: Mat4::IDENTITY,
            near: 0.1,
            far: 1000.0,
            fov_target: 60.0,
            zoom_speed: 1.0,
        }
    }

    pub fn update_with_transform(&mut self, proj_matrix: &Mat4, cam_transform: &Transform) {
        let pos = cam_transform.position();
        let view_mat: Mat4 = cam_transform.affine().inverse().into();

        self.update(proj_matrix, &pos, &view_mat);
    }

    pub fn update(&mut self, proj_matrix: &Mat4, pos: &Vec3, view_matrix: &Mat4) {
        self.pos = *pos;
        self.view_mat = *view_matrix;
        self.projection_mat = *proj_matrix;

        self.proj_view_mat = proj_matrix * self.view_mat;
        self.inv_proj_view_mat = self.proj_view_mat.inverse();
    }

    pub fn frustum(&self) -> Frustum {
        Frustum::from_matrix(&self.proj_view_mat)
    }
}

impl SystemUniform {
    pub const fn empty() -> Self {
        SystemUniform {
            screen_size: UVec2::ZERO,
            time: 0.0,
            delta_time: 0.0,
        }
    }
}

impl RenderUniformData {
    pub fn empty(device: &Device, render_bgl: &BindGroupLayout) -> Self {
        let camera_data = CameraUniform::empty();
        let system_data = SystemUniform::empty();
        let uniform = ShaderUniform::<RenderUniformIndex>::builder((*render_bgl).clone())
            .with_buffer_data(&camera_data)
            .with_buffer_data(&system_data)
            .build(device);

        RenderUniformData {
            camera_data,
            system_data,
            uniform,
        }
    }

    pub fn update_shadow_camera_for_spot(&mut self, light: &LightProxy, queue: &Queue) {
        let fovy = (2.0 * light.outer_angle).clamp(0.0175, 3.12);
        let near = 0.05_f32;
        let far = light.range.max(near + 0.01);
        let proj = Mat4::perspective_rh(fovy, 1.0, near, far);

        self.camera_data
            .update(&proj, &light.position, &light.view_mat);
        self.upload_camera_data(queue);
    }

    pub fn update_shadow_camera_for_point(&mut self, light: &LightProxy, face: u8, queue: &Queue) {
        const DIRECTIONS: [Vec3; 6] = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ];
        const UPS: [Vec3; 6] = [
            Vec3::NEG_Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
            Vec3::NEG_Y,
            Vec3::NEG_Y,
        ];

        let idx = face.min(5) as usize;
        let dir = DIRECTIONS[idx];
        let up = UPS[idx];

        let eye = light.position;
        let target = light.position + dir;
        let view = Mat4::look_at_rh(eye, target, up);

        let near = 0.05_f32;
        let far = light.range.max(near + 0.01);
        let proj = Mat4::perspective_rh(FRAC_PI_2, 1.0, near, far);

        self.camera_data.update(&proj, &light.position, &view);
        self.upload_camera_data(queue);
    }

    pub fn upload_camera_data(&self, queue: &Queue) {
        queue.write_buffer(
            self.uniform.buffer(RenderUniformIndex::Camera),
            0,
            bytemuck::bytes_of(&self.camera_data),
        );
    }

    pub fn upload_system_data(&self, queue: &Queue) {
        queue.write_buffer(
            self.uniform.buffer(RenderUniformIndex::System),
            0,
            bytemuck::bytes_of(&self.system_data),
        );
    }
}
