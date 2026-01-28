use crate::MeshRenderer;
use itertools::izip;
use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;
use syrillian::core::Bones;
use syrillian::math::Quat;
use syrillian::math::{Mat4, Vec3};
use syrillian::tracing::warn;

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct SkeletalComponent {
    bones_static: Bones,
    skin_transform: Vec<Mat4>,
    skin_rotation: Vec<Mat4>,
    skin_scale: Vec<Mat4>,
    skin_local: Vec<Mat4>,
    globals: Vec<Mat4>,
    palette: Vec<Mat4>,
    #[dont_reflect]
    dirty: bool,
}

impl Default for SkeletalComponent {
    fn default() -> Self {
        Self {
            bones_static: Bones::none(),
            skin_transform: Vec::new(),
            skin_rotation: Vec::new(),
            skin_scale: Vec::new(),
            skin_local: Vec::new(),
            globals: Vec::new(),
            palette: Vec::new(),
            dirty: true,
        }
    }
}

impl Component for SkeletalComponent {
    fn init(&mut self, world: &mut World) {
        let Some(renderer) = self.parent().get_component::<MeshRenderer>() else {
            warn!("No Mesh Renderer found on Skeletal Object");
            return;
        };
        let Some(mesh) = world.assets.meshes.try_get(renderer.mesh()) else {
            warn!("No Mesh found for the Mesh linked in a Mesh Renderer");
            return;
        };

        let n = mesh.bones.len();
        self.bones_static.clone_from(&mesh.bones);

        let (t, r, s, sl): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) = self
            .bones_static
            .bind_local
            .iter()
            .map(|l| {
                let (s, r, t) = l.to_scale_rotation_translation();
                let t: Mat4 = Mat4::from_translation(t);
                let r: Mat4 = Mat4::from_quat(r);
                let s: Mat4 = Mat4::from_scale(s);
                let sl = t * r * s;
                (t, r, s, sl)
            })
            .collect();

        self.skin_transform = t;
        self.skin_rotation = r;
        self.skin_scale = s;
        self.skin_local = sl;

        self.globals = vec![Mat4::IDENTITY; n];
        self.palette = vec![Mat4::IDENTITY; n];

        self.dirty = true;
    }
}

impl SkeletalComponent {
    pub fn bone_count(&self) -> usize {
        self.bones_static.len()
    }

    /// Access bones metadata (names/parents/inv_bind)
    pub fn bones(&self) -> &Bones {
        &self.bones_static
    }

    /// Set local TRS for (some/all) bones.
    pub fn set_local_pose_trs(&mut self, locals: &[(Vec3, Quat, Vec3)]) {
        let n = self.bones_static.len();
        self.skin_transform.resize(n, Mat4::IDENTITY);
        self.skin_rotation.resize(n, Mat4::IDENTITY);
        self.skin_scale.resize(n, Mat4::IDENTITY);
        self.skin_local.resize(n, Mat4::IDENTITY);

        for (i, (pos, rot, scale)) in locals.iter().enumerate().take(n) {
            self.set_local_transform(i, *pos);
            self.set_local_rotation(i, *rot);
            self.set_local_scale(i, *scale);
        }
        self.dirty = true;
    }

    pub fn set_local_transform(&mut self, index: usize, pos: Vec3) {
        self.skin_transform[index] = Mat4::from_translation(pos);
        self.dirty = true;
    }

    pub fn set_local_rotation(&mut self, index: usize, q: Quat) {
        self.skin_rotation[index] = Mat4::from_quat(q);
        self.dirty = true;
    }

    pub fn set_local_scale(&mut self, index: usize, scale: Vec3) {
        self.skin_scale[index] = Mat4::from_scale(scale);
        self.dirty = true;
    }

    pub fn palette(&self) -> &[Mat4] {
        &self.palette
    }

    fn recalculate_skin_locals(&mut self) {
        for (i, (t, r, s)) in
            izip!(&self.skin_transform, &self.skin_rotation, &self.skin_scale).enumerate()
        {
            self.skin_local[i] = t * r * s;
        }
    }

    pub fn update_palette(&mut self) -> bool {
        if !self.dirty {
            return false;
        }

        self.recalculate_skin_locals();

        fn visit(
            i: usize,
            bones: &Bones,
            globals: &mut [Mat4],
            skin_locals: &[Mat4],
            palette: &mut [Mat4],
            parent_global: Mat4,
        ) {
            let g = parent_global * skin_locals[i];
            globals[i] = g;
            palette[i] = g * bones.inverse_bind[i];
            for &c in &bones.children[i] {
                visit(c, bones, globals, skin_locals, palette, g);
            }
        }

        for &root in &self.bones_static.roots {
            visit(
                root,
                &self.bones_static,
                &mut self.globals,
                &self.skin_local,
                &mut self.palette,
                Mat4::IDENTITY,
            );
        }

        self.dirty = false;
        true
    }
}
