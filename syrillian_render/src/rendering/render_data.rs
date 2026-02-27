use crate::lighting::proxy::LightProxy;
use crate::rendering::uniform::ShaderUniform;
use glamx::{Mat4, UVec2, Vec3};
use std::f32::consts::FRAC_PI_2;
use syrillian_asset::ensure_aligned;
use syrillian_macros::UniformIndex;
use syrillian_utils::Frustum;
use wgpu::{BindGroupLayout, Device, Queue, Sampler, TextureView};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

// TODO: Use proper matrix types (Affine3, Perspective3)
#[repr(C)]
#[derive(Debug, Copy, Clone, Immutable, FromBytes, IntoBytes, KnownLayout)]
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
#[derive(Default, Copy, Clone, Debug, Immutable, FromBytes, IntoBytes, KnownLayout)]
pub struct SystemUniform {
    pub screen_size: UVec2,
    pub time: f32,
    pub delta_time: f32,
}

ensure_aligned!(SystemUniform { screen_size }, align <= 8 * 2 => size);

#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum SkyboxMode {
    #[default]
    Cubemap = 0,
    Procedural = 1,
}

impl SkyboxMode {
    pub const fn as_raw(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SkyAtmosphereSettings {
    pub sun_intensity: f32,
    pub sun_strength: f32,
    pub sun_elevation: f32,
    pub sun_rotation: f32,
    pub altitude: f32,
    pub air_density: f32,
    pub aerosols: f32,
}

impl Default for SkyAtmosphereSettings {
    fn default() -> Self {
        Self {
            sun_intensity: 1.0,
            sun_strength: 1.0,
            sun_elevation: 35.0_f32.to_radians(),
            sun_rotation: 0.0,
            altitude: 0.0,
            air_density: 1.0,
            aerosols: 0.15,
        }
    }
}

impl SkyAtmosphereSettings {
    pub fn clamped(self) -> Self {
        Self {
            sun_intensity: self.sun_intensity.max(0.0),
            sun_strength: self.sun_strength.max(0.0),
            sun_elevation: self.sun_elevation.clamp(-1.5533, 1.5533),
            sun_rotation: self.sun_rotation,
            altitude: self.altitude.max(-5000.0),
            air_density: self.air_density.max(0.0),
            aerosols: self.aerosols.max(0.0),
        }
    }

    pub fn sun_direction(self) -> Vec3 {
        let ce = self.sun_elevation.cos();
        let dir = Vec3::new(
            ce * self.sun_rotation.sin(),
            self.sun_elevation.sin(),
            -ce * self.sun_rotation.cos(),
        );
        if dir.length_squared() > 1e-8 {
            dir.normalize()
        } else {
            Vec3::Y
        }
    }

    pub fn from_sun_direction(direction_to_sun: Vec3) -> Self {
        let mut out = Self::default();
        let dir = if direction_to_sun.length_squared() > 1e-8 {
            direction_to_sun.normalize()
        } else {
            Vec3::Y
        };
        out.sun_elevation = dir.y.clamp(-1.0, 1.0).asin();
        out.sun_rotation = dir.x.atan2(-dir.z);
        out
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Immutable, FromBytes, IntoBytes, KnownLayout)]
pub struct SkyUniform {
    pub sun_direction: Vec3,
    pub mode: u32,
    pub sun_intensity: f32,
    pub sun_strength: f32,
    pub sun_elevation: f32,
    pub sun_rotation: f32,
    pub altitude: f32,
    pub air_density: f32,
    pub aerosols: f32,
    pub _pad0: f32,
}

ensure_aligned!(SkyUniform { sun_direction }, align <= 16 * 3 => size);

impl Default for SkyUniform {
    fn default() -> Self {
        Self::from_mode_and_settings(SkyboxMode::Cubemap, SkyAtmosphereSettings::default())
    }
}

impl SkyUniform {
    pub fn from_mode_and_settings(mode: SkyboxMode, settings: SkyAtmosphereSettings) -> Self {
        let settings = settings.clamped();
        Self {
            sun_direction: settings.sun_direction(),
            mode: mode.as_raw(),
            sun_intensity: settings.sun_intensity,
            sun_strength: settings.sun_strength,
            sun_elevation: settings.sun_elevation,
            sun_rotation: settings.sun_rotation,
            altitude: settings.altitude,
            air_density: settings.air_density,
            aerosols: settings.aerosols,
            _pad0: 0.0,
        }
    }

    pub fn set_mode(&mut self, mode: SkyboxMode) {
        self.mode = mode.as_raw();
    }

    pub fn apply_settings(&mut self, settings: SkyAtmosphereSettings) {
        let settings = settings.clamped();
        self.sun_direction = settings.sun_direction();
        self.sun_intensity = settings.sun_intensity;
        self.sun_strength = settings.sun_strength;
        self.sun_elevation = settings.sun_elevation;
        self.sun_rotation = settings.sun_rotation;
        self.altitude = settings.altitude;
        self.air_density = settings.air_density;
        self.aerosols = settings.aerosols;
    }

    pub fn sync_from_sun_light(&mut self, light: &LightProxy) {
        let to_sun = -light.direction;
        let dir = if to_sun.length_squared() > 1e-8 {
            to_sun.normalize()
        } else {
            Vec3::Y
        };
        self.sun_direction = dir;
        self.sun_elevation = dir.y.clamp(-1.0, 1.0).asin();
        self.sun_rotation = dir.x.atan2(-dir.z);
        let intensity = light.intensity.max(0.0);
        self.sun_intensity = intensity;
        self.sun_strength = intensity;
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum RenderUniformIndex {
    Camera = 0,
    System = 1,
    Skybox = 2,
    SkyboxSampler = 3,
    Sky = 4,
}

pub struct RenderUniformData {
    pub camera_data: CameraUniform,
    pub system_data: SystemUniform,
    pub sky_data: SkyUniform,
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
    pub fn empty(
        device: &Device,
        render_bgl: &BindGroupLayout,
        skybox_view: TextureView,
        skybox_sampler: Sampler,
    ) -> Self {
        let camera_data = CameraUniform::empty();
        let system_data = SystemUniform::empty();
        let sky_data = SkyUniform::default();
        let uniform = ShaderUniform::<RenderUniformIndex>::builder((*render_bgl).clone())
            .with_buffer_data(&camera_data)
            .with_buffer_data(&system_data)
            .with_texture(skybox_view)
            .with_sampler(skybox_sampler)
            .with_buffer_data(&sky_data)
            .build(device);

        RenderUniformData {
            camera_data,
            system_data,
            sky_data,
            uniform,
        }
    }

    pub fn update_shadow_camera_for_spot(&mut self, light: &LightProxy, queue: &Queue) {
        let fovy = (2.0 * light.outer_angle).clamp(0.0175, 3.12);
        let near = 0.05_f32;
        let far = light.range.max(near + 0.01);
        let proj = Mat4::perspective_rh(fovy, 1.0, near, far);
        let view = Mat4::look_at_rh(light.position, light.position + light.direction, light.up);

        self.camera_data.update(&proj, &light.position, &view);
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
            self.camera_data.as_bytes(),
        );
    }

    pub fn upload_system_data(&self, queue: &Queue) {
        queue.write_buffer(
            self.uniform.buffer(RenderUniformIndex::System),
            0,
            self.system_data.as_bytes(),
        );
    }

    pub fn upload_sky_data(&self, queue: &Queue) {
        queue.write_buffer(
            self.uniform.buffer(RenderUniformIndex::Sky),
            0,
            self.sky_data.as_bytes(),
        );
    }

    pub fn set_sky_mode(&mut self, mode: SkyboxMode) {
        self.sky_data.set_mode(mode);
    }

    pub fn set_sky_settings(&mut self, settings: SkyAtmosphereSettings) {
        self.sky_data.apply_settings(settings);
    }

    pub fn sync_sun_light(&mut self, light: &LightProxy) {
        self.sky_data.sync_from_sun_light(light);
    }

    pub fn rebuild_bind_group(
        &mut self,
        device: &Device,
        render_bgl: &BindGroupLayout,
        skybox_view: TextureView,
        skybox_sampler: Sampler,
    ) {
        self.uniform = ShaderUniform::<RenderUniformIndex>::builder((*render_bgl).clone())
            .with_buffer_data(&self.camera_data)
            .with_buffer_data(&self.system_data)
            .with_texture(skybox_view)
            .with_sampler(skybox_sampler)
            .with_buffer_data(&self.sky_data)
            .build(device);
    }
}
