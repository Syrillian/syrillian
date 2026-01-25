use crate::Reflect;
use crate::World;
use crate::components::Component;
use crate::core::GameObjectId;
use crate::utils::FloatMathExt;
use crate::windowing::RenderTargetId;
use nalgebra::{Matrix4, Perspective3, Point3, Vector2, Vector4};
use rapier3d::geometry::Ray;

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct CameraComponent {
    pub projection: Perspective3<f32>,
    pub projection_inverse: Matrix4<f32>,
    fov_active: f32,
    fov_target: f32,
    near: f32,
    far: f32,
    width: f32,
    height: f32,
    pub zoom_speed: f32,

    #[dont_reflect]
    projection_dirty: bool,
    #[dont_reflect]
    render_target: RenderTargetId,
}

impl CameraComponent {
    /// Returns the fov in degrees
    pub fn fov(&self) -> f32 {
        self.fov_active
    }

    /// Sets the fov in degrees. Only use this for camera switches/init etc.
    /// Prefer set_fov_target for a smooth zoom.
    pub fn set_fov_instant(&mut self, fov: f32) {
        self.fov_active = fov;
        self.regenerate();
    }

    /// Smoothly zoom to the fov using the speed specified with set_zoom_speed
    pub fn set_fov_target(&mut self, fov: f32) {
        self.fov_target = fov;
    }

    pub fn near(&self) -> f32 {
        self.near
    }

    pub fn set_near(&mut self, near: f32) {
        self.near = near;
        self.regenerate();
    }

    #[inline]
    pub fn far(&self) -> f32 {
        self.far
    }

    pub fn set_far(&mut self, far: f32) {
        self.far = far;
        self.regenerate();
    }

    #[inline]
    pub fn resolution(&self) -> (f32, f32) {
        (self.width, self.height)
    }

    #[inline]
    pub fn mouse_viewport_position(&self, x: f32, y: f32) -> Vector2<f32> {
        Vector2::new(x.max(0.), y.max(0.))
    }

    #[inline]
    pub fn mouse_viewport_ndc(&self, x: f32, y: f32) -> Vector2<f32> {
        let nx = (x / self.width).clamp(0.0, 1.0);
        let ny = 1.0 - (y / self.height).clamp(0.0, 1.0);
        Vector2::new(nx * 2.0 - 1.0, ny * 2.0 - 1.0)
    }

    #[inline]
    pub fn mouse_eye_dir(&self, x: f32, y: f32) -> Vector4<f32> {
        let ndc = self.mouse_viewport_ndc(x, y);
        let clip = Vector4::new(ndc.x, ndc.y, 0.0, 1.0);
        let mut eye = self.projection_inverse * clip;
        eye /= eye.w;
        eye.w = 0.0;
        eye
    }

    pub fn click_ray(&self, x: f32, y: f32) -> Ray {
        let eye = self.mouse_eye_dir(x, y);

        let cam_to_world = self.parent().transform.view_matrix_rigid().to_matrix();

        let dir_world = (cam_to_world * eye).xyz().normalize();
        let origin = cam_to_world.transform_point(&Point3::origin());

        Ray::new(origin, dir_world)
    }

    pub fn regenerate(&mut self) {
        self.projection = Perspective3::new(
            self.width / self.height,
            self.fov_active.to_radians(),
            self.near,
            self.far,
        );
        self.projection_inverse = self.projection.inverse();
        self.projection_dirty = true;
    }

    pub fn is_projection_dirty(&self) -> bool {
        self.projection_dirty
    }

    pub fn clear_projection_dirty(&mut self) {
        self.projection_dirty = false;
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        self.regenerate();
    }

    pub fn render_target(&self) -> RenderTargetId {
        self.render_target
    }

    pub fn set_render_target(&mut self, target: RenderTargetId) {
        self.render_target = target;
    }

    #[cfg(debug_assertions)]
    pub fn push_debug_ray(&mut self, ray: Ray, max_toi: f32) {
        use crate::components::CameraDebug;

        let Some(mut debug) = self.parent().get_component::<CameraDebug>() else {
            tracing::warn!("No camera debug drawable found!");
            return;
        };

        debug.push_ray(ray, max_toi);
    }
}

impl Default for CameraComponent {
    fn default() -> Self {
        let projection = Perspective3::new(800.0 / 600.0, 60f32.to_radians(), 0.01, 1000.0);
        let projection_inverse = projection.inverse();

        CameraComponent {
            projection,
            projection_inverse,
            fov_active: 60.0,
            fov_target: 0.0,
            zoom_speed: 10.0,
            near: 0.01,
            far: 1000.0,
            width: 800.0,
            height: 600.0,
            projection_dirty: true,
            render_target: RenderTargetId::PRIMARY,
        }
    }
}

impl Component for CameraComponent {
    #[cfg(debug_assertions)]
    fn init(&mut self, _world: &mut World) {
        add_debug_drawable(self.parent());
    }

    fn update(&mut self, world: &mut World) {
        let delta_time = world.delta_time().as_secs_f32();

        if self.fov_target != 0.0 && (self.fov_active - self.fov_target).abs() > f32::EPSILON {
            self.fov_active = self
                .fov_active
                .lerp(self.fov_target, self.zoom_speed * delta_time);
            self.regenerate();
        }
    }
}

#[cfg(debug_assertions)]
fn add_debug_drawable(mut parent: GameObjectId) {
    use crate::components::CameraDebug;

    parent.add_component::<CameraDebug>();
}
