use crate::Reflect;
use crate::World;
use crate::components::{Component, MeshRenderer};
use crate::core::Bones;
use crate::utils::{ExtraMatrixMath, MATRIX4_ID};
use itertools::izip;
use nalgebra::{Matrix4, Rotation3, Scale3, Vector3};
use nalgebra::{Translation3, UnitQuaternion};
use tracing::warn;

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct SkeletalComponent {
    bones_static: Bones,
    skin_transform: Vec<Matrix4<f32>>,
    skin_rotation: Vec<Matrix4<f32>>,
    skin_scale: Vec<Matrix4<f32>>,
    skin_local: Vec<Matrix4<f32>>,
    globals: Vec<Matrix4<f32>>,
    palette: Vec<Matrix4<f32>>,
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
                let (t, r, s) = l.decompose();
                let (t, r, s) = (
                    Translation3::from(t).to_homogeneous(),
                    Rotation3::from(r).to_homogeneous(),
                    Scale3::from(s).to_homogeneous(),
                );
                let sl = t * r * s;
                (t, r, s, sl)
            })
            .collect();

        self.skin_transform = t;
        self.skin_rotation = r;
        self.skin_scale = s;
        self.skin_local = sl;

        self.globals = vec![Matrix4::identity(); n];
        self.palette = vec![Matrix4::identity(); n];

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
    pub fn set_local_pose_trs(
        &mut self,
        locals: &[(Vector3<f32>, UnitQuaternion<f32>, Vector3<f32>)],
    ) {
        let n = self.bones_static.len();
        self.skin_transform.resize(n, MATRIX4_ID);
        self.skin_rotation.resize(n, MATRIX4_ID);
        self.skin_scale.resize(n, MATRIX4_ID);
        self.skin_local.resize(n, MATRIX4_ID);

        for (i, (pos, rot, scale)) in locals.iter().enumerate().take(n) {
            self.set_local_transform(i, Translation3::from(*pos));
            self.set_local_rotation(i, rot.to_rotation_matrix());
            self.set_local_scale(i, Scale3::from(*scale));
        }
        self.dirty = true;
    }

    pub fn set_local_transform(&mut self, index: usize, pos: Translation3<f32>) {
        self.skin_transform[index] = pos.to_homogeneous();
        self.dirty = true;
    }

    pub fn set_local_rotation(&mut self, index: usize, q: Rotation3<f32>) {
        let mut rot = Matrix4::identity();
        rot.fixed_view_mut::<3, 3>(0, 0).copy_from(q.matrix());
        self.skin_rotation[index] = rot;
        self.dirty = true;
    }

    pub fn set_local_scale(&mut self, index: usize, scale: Scale3<f32>) {
        self.skin_scale[index] = scale.to_homogeneous();
        self.dirty = true;
    }

    pub fn palette(&self) -> &[Matrix4<f32>] {
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
            globals: &mut [Matrix4<f32>],
            skin_locals: &[Matrix4<f32>],
            palette: &mut [Matrix4<f32>],
            parent_global: Matrix4<f32>,
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
                MATRIX4_ID,
            );
        }

        self.dirty = false;
        true
    }
}
