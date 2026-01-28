use crate::components::Component;
use crate::core::GameObject;
use crate::math::Vec3;
use std::ops::{Deref, DerefMut};

pub trait GameObjectExt {
    fn at(&mut self, x: f32, y: f32, z: f32) -> &mut Self;
    fn at_vec(&mut self, pos: Vec3) -> &mut Self;
    fn scale(&mut self, scale: f32) -> &mut Self;
    fn non_uniform_scale(&mut self, x: f32, y: f32, z: f32) -> &mut Self;
}

pub trait GOComponentExt<'a>: Component + Default {
    type Outer: Deref<Target = GameObject> + DerefMut;

    fn build_component(&'a mut self, obj: &'a mut GameObject) -> Self::Outer;
    fn finish(outer: &'a mut Self::Outer) -> &'a mut GameObject {
        outer.deref_mut()
    }
}

impl GameObjectExt for GameObject {
    #[inline]
    fn at(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.transform.set_position(x, y, z);
        self
    }

    #[inline]
    fn at_vec(&mut self, pos: Vec3) -> &mut Self {
        self.transform.set_position_vec(pos);
        self
    }

    #[inline]
    fn scale(&mut self, scale: f32) -> &mut Self {
        self.transform.set_scale(scale);
        self
    }

    #[inline]
    fn non_uniform_scale(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.transform.set_nonuniform_scale(x, y, z);
        self
    }
}

impl GameObject {
    #[inline]
    pub fn build_component<'a, C: GOComponentExt<'a>>(&'a mut self) -> C::Outer {
        let component = self.add_component::<C>();
        C::build_component(component.forget_lifetime(), self)
    }
}
