use crate::World;
use crate::components::Component;
use crate::core::reflection::{PartialReflect, ReflectedTypeInfo};
use crate::rendering::CPUDrawCtx;
use crate::rendering::lights::{Light, LightProxy, LightType};
use crate::utils::FloatMathExt;
use std::any::TypeId;
use std::marker::PhantomData;
use std::mem::offset_of;
use syrillian::core::reflection::ReflectedField;

pub trait LightTypeTrait: Send + Sync {
    const NAME: &str;
    const FULL_NAME: &str;
    fn type_id() -> LightType;
}

pub struct Point;
pub struct Sun;
pub struct Spot;

pub struct LightComponent<L: LightTypeTrait + 'static> {
    target_inner_angle: f32,
    target_outer_angle: f32,
    pub inner_angle_t: f32,
    pub outer_angle_t: f32,
    pub tween_enabled: bool,
    dirty: bool,

    local_proxy: LightProxy,

    light_type: PhantomData<L>,
}

pub type PointLightComponent = LightComponent<Point>;
pub type SunLightComponent = LightComponent<Sun>;
pub type SpotLightComponent = LightComponent<Spot>;

impl<T: LightTypeTrait> PartialReflect for LightComponent<T> {
    const DATA: ReflectedTypeInfo = ReflectedTypeInfo {
        type_id: TypeId::of::<Self>(),
        type_name: T::FULL_NAME,
        short_name: T::NAME,
        fields: &[
            ReflectedField {
                name: "inner_angle_t",
                offset: offset_of!(Self, inner_angle_t),
                type_id: TypeId::of::<f32>(),
            },
            ReflectedField {
                name: "outer_angle_t",
                offset: offset_of!(Self, outer_angle_t),
                type_id: TypeId::of::<f32>(),
            },
            ReflectedField {
                name: "tween_enabled",
                offset: offset_of!(Self, tween_enabled),
                type_id: TypeId::of::<bool>(),
            },
        ],
    };
}

impl LightTypeTrait for Sun {
    const NAME: &str = "SunLightComponent";
    const FULL_NAME: &str = concat!(module_path!(), "::", "SunLightComponent");

    fn type_id() -> LightType {
        LightType::Sun
    }
}

impl LightTypeTrait for Point {
    const NAME: &str = "PointLightComponent";
    const FULL_NAME: &str = concat!(module_path!(), "::", "PointLightComponent");

    fn type_id() -> LightType {
        LightType::Point
    }
}

impl LightTypeTrait for Spot {
    const NAME: &str = "SpotLightComponent";
    const FULL_NAME: &str = concat!(module_path!(), "::", "SpotLightComponent");

    fn type_id() -> LightType {
        LightType::Spot
    }
}

impl<L: LightTypeTrait + 'static> Default for LightComponent<L> {
    fn default() -> Self {
        const DEFAULT_INNER_ANGLE: f32 = 5.0f32.to_radians();
        const DEFAULT_OUTER_ANGLE: f32 = 30.0f32.to_radians();

        let mut local_proxy = LightProxy::dummy();

        let type_id = L::type_id();
        local_proxy.type_id = type_id as u32;
        if type_id == LightType::Spot {
            local_proxy.inner_angle = DEFAULT_INNER_ANGLE;
            local_proxy.outer_angle = DEFAULT_OUTER_ANGLE;
            local_proxy.range = 100.0;
            local_proxy.intensity = 1000.0;
        }

        LightComponent {
            target_inner_angle: DEFAULT_INNER_ANGLE,
            target_outer_angle: DEFAULT_OUTER_ANGLE,
            inner_angle_t: 1.0,
            outer_angle_t: 1.0,
            tween_enabled: false,

            dirty: true,
            local_proxy,

            light_type: PhantomData,
        }
    }
}

impl<L: LightTypeTrait + 'static> Component for LightComponent<L> {
    fn init(&mut self, _world: &mut World) {
        let parent = self.parent();
        self.local_proxy.position = parent.transform.position();
        self.local_proxy.direction = parent.transform.forward();
        self.local_proxy.up = parent.transform.up();
        self.local_proxy.view_mat = parent.transform.view_matrix_rigid().to_matrix();
    }

    fn late_update(&mut self, world: &mut World) {
        let parent = self.parent();
        if parent.transform.is_dirty() {
            self.local_proxy.position = parent.transform.position();
            self.local_proxy.direction = parent.transform.forward();
            self.local_proxy.up = parent.transform.up();
            self.local_proxy.view_mat = parent.transform.view_matrix_rigid().to_matrix();
            self.dirty = true;
        }

        if self.tween_enabled {
            let delta = world.delta_time().as_secs_f32();

            self.local_proxy.outer_angle = self
                .local_proxy
                .outer_angle
                .lerp(self.target_outer_angle, self.outer_angle_t * delta);
            self.local_proxy.inner_angle = self
                .local_proxy
                .inner_angle
                .lerp(self.target_inner_angle, self.inner_angle_t * delta);
            self.dirty = true;
        }
    }

    fn create_light_proxy(&mut self, _world: &World) -> Option<Box<LightProxy>> {
        Some(Box::new(self.local_proxy))
    }

    fn update_proxy(&mut self, _world: &World, mut ctx: CPUDrawCtx) {
        if !self.dirty {
            return;
        }

        let new_proxy = self.local_proxy;
        ctx.send_light_proxy_update(move |proxy| {
            *proxy = new_proxy;
        });

        self.dirty = false;
    }
}

impl<L: LightTypeTrait + 'static> Light for LightComponent<L> {
    #[inline]
    fn light_type(&self) -> LightType {
        L::type_id()
    }

    fn data(&self) -> &LightProxy {
        &self.local_proxy
    }

    fn data_mut(&mut self, mark_dirty: bool) -> &mut LightProxy {
        if mark_dirty {
            self.dirty = true;
        }
        &mut self.local_proxy
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }
}

impl<L: LightTypeTrait + 'static> LightComponent<L> {
    pub fn set_outer_angle_tween_target(&mut self, angle: f32) {
        let rad = angle.clamp(f32::EPSILON, 45. - f32::EPSILON).to_radians();
        self.target_outer_angle = rad;
    }

    pub fn set_inner_angle_tween_target(&mut self, angle: f32) {
        let rad = angle.clamp(f32::EPSILON, 45. - f32::EPSILON).to_radians();
        self.target_inner_angle = rad;
    }
}
