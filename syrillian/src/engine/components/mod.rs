//! Built-in components that can be attached to [`GameObject`](crate::core::GameObject).
//!
//! Components implement behavior ranging from camera control to physics. If it's dynamic,
//! it's probably a component.
//!
//! To make a component:
//! ```rust
//! use nalgebra::Vector3;
//! use syrillian::components::{Component};
//! use syrillian::core::GameObjectId;
//! use syrillian::World;
//!
//! pub struct Gravity {
//!     force: f32,
//! }
//!
//! impl Default for Gravity {
//!     fn default() -> Self {
//!         Gravity {
//!             force: 8.91,
//!         }
//!     }
//! }
//!
//! impl Component for Gravity {
//!     fn update(&mut self, world: &mut World) {
//!         let delta_time = world.delta_time().as_secs_f32();
//!
//!         let movement = Vector3::new(0.0, self.force * delta_time, 0.0);
//!
//!         let transform = &mut self.parent().transform;
//!         transform.translate(movement);
//!     }
//! }
//! ```

pub mod animation;
pub mod audio;
pub mod button;
pub mod camera;
pub mod collider;
pub mod fp_camera;
pub mod fp_movement;
pub mod freecam;
pub mod gravity;
pub mod image;
pub mod joints;
pub mod light;
pub mod mesh_renderer;
pub mod panel;
pub mod rigid_body;
pub mod rotate;
pub mod skeletal;
pub mod text;
pub mod ui_rect;

#[cfg(debug_assertions)]
pub mod camera_debug;

pub use animation::AnimationComponent;
pub use button::Button;
pub use camera::CameraComponent;
pub use collider::Collider3D;
pub use fp_camera::FirstPersonCameraController;
pub use fp_movement::FirstPersonMovementController;
pub use freecam::FreecamController;
pub use gravity::GravityComponent;
pub use image::Image;
pub use joints::{
    FixedJoint, PrismaticJoint, RevoluteJoint, RopeJoint, SphericalJoint, SpringJoint,
};
pub use light::{PointLightComponent, SpotLightComponent, Sun, SunLightComponent};
pub use mesh_renderer::MeshRenderer;
pub use panel::Panel;
pub use rigid_body::RigidBodyComponent;
pub use rotate::RotateComponent;
pub use skeletal::SkeletalComponent;
pub use text::{Text2D, Text3D};
pub use ui_rect::UiRect;

#[cfg(debug_assertions)]
pub use camera_debug::*;

use crate::World;
use crate::core::GameObjectId;
use crate::core::component_context_inference::ComponentContextInference;
use crate::core::reflection::{ReflectedTypeInfo, Value, type_info};
use crate::rendering::lights::LightProxy;
use crate::rendering::proxies::SceneProxy;
use crate::rendering::{CPUDrawCtx, UiContext};
use delegate::delegate;
use slotmap::{Key, new_key_type};
use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use syrillian::core::reflection::{Reflect, ReflectSerialize};

new_key_type! { pub struct ComponentId; }

pub struct ComponentContext {
    pub(crate) tid: TypedComponentId,
    pub(crate) parent: GameObjectId,
}

impl ComponentContext {
    pub(crate) fn new(tid: TypedComponentId, parent: GameObjectId) -> Self {
        Self { tid, parent }
    }

    pub(crate) unsafe fn null() -> Self {
        ComponentContext {
            tid: TypedComponentId::null::<dyn Component>(),
            parent: GameObjectId::null(),
        }
    }

    pub fn parent(&self) -> GameObjectId {
        self.parent
    }

    pub fn world(&self) -> &'static mut World {
        self.parent.world()
    }
}

pub struct CRef<C: Component + ?Sized> {
    pub(crate) data: Option<Rc<C>>,
    pub(crate) ctx: Rc<ComponentContext>,
}

impl<C: Component + Reflect> Reflect for CRef<C> {
    fn field_ref<'a, T: 'static>(this: &'a Self, name: &str) -> Option<&'a T>
    where
        Self: Sized + 'static,
    {
        Reflect::field_ref(this.get_mut(), name)
    }

    fn field_mut<'a, T: 'static>(this: &'a mut Self, name: &str) -> Option<&'a mut T>
    where
        Self: Sized + 'static,
    {
        Reflect::field_mut(this.get_mut(), name)
    }
}
//
// impl Reflect for CRef<dyn Component> {
//     fn field_ref<'a, T: 'static>(this: &'a Self, name: &str) -> Option<&'a T>
//     where
//         Self: Sized + 'static,
//     {
//         let type_info = type_info(this.ctx.tid.0)?;
//         let field = type_info.fields.iter().find(|f| f.name == name)?;
//         if field.type_id != TypeId::of::<T>() {
//             return None;
//         }
//         let base = unsafe { *(this.get_mut() as *mut _ as *mut *mut u8) };
//         Some(unsafe { &*(base.byte_add(field.offset) as *const T) })
//     }
//
//     fn field_mut<'a, T: 'static>(this: &'a mut Self, name: &str) -> Option<&'a mut T>
//     where
//         Self: Sized + 'static,
//     {
//         let type_info = type_info(this.ctx.tid.0)?;
//         let field = type_info.fields.iter().find(|f| f.name == name)?;
//         if field.type_id != TypeId::of::<T>() {
//             return None;
//         }
//         let base = unsafe { std::ptr::from_mut(this.get_mut()).cast::<u8>() };
//         Some(unsafe { &mut *(base.byte_add(field.offset) as *mut T) })
//     }
// }

impl ReflectSerialize for CRef<dyn Component> {
    fn serialize(this: &Self) -> Value
    where
        Self: Sized,
    {
        let Some(type_info) = type_info(this.ctx.tid.0) else {
            return Value::None;
        };
        let base = std::ptr::from_mut(this.get_mut()).cast::<u8>();
        (type_info.actions.serialize)(base)
    }
}

impl<C: Component + ?Sized> Clone for CRef<C> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            ctx: self.ctx.clone(),
        }
    }
}

impl<C: Component + ?Sized> Deref for CRef<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().unwrap_unchecked() }
    }
}

impl<C: Component + ?Sized> DerefMut for CRef<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(&raw const **self.data.as_ref().unwrap_unchecked() as *mut _) }
    }
}

impl<C: Component + ?Sized> Hash for CRef<C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.typed_id().hash(state)
    }
}

#[allow(unused)]
impl<C: Component> CRef<C> {
    pub(crate) fn new(comp: Rc<C>, tid: TypedComponentId, parent: GameObjectId) -> Self {
        CRef {
            data: Some(comp),
            ctx: Rc::new(ComponentContext::new(tid, parent)),
        }
    }

    pub(crate) fn forget_lifetime(mut self) -> &'static mut C {
        unsafe { mem::transmute(self.deref_mut()) }
    }

    pub fn downgrade(self) -> CWeak<C> {
        self.into()
    }

    pub fn as_dyn(&self) -> CRef<dyn Component> {
        unsafe {
            CRef {
                data: Some(self.data.as_ref().unwrap_unchecked().clone() as Rc<dyn Component>),
                ctx: self.ctx.clone(),
            }
        }
    }

    /// # Safety
    ///
    /// This is uninitialized territory. If you use this, you'll need to make sure to
    /// overwrite it before using it. Accessing this in any way is UB.
    ///
    /// The only reason this exists is that you can save References for components which
    /// are also managed by a component so you can avoid Option. It's not recommended to
    /// use this.
    pub unsafe fn null() -> CRef<C> {
        CRef {
            data: None,
            ctx: Rc::new(unsafe { ComponentContext::null() }),
        }
    }
}

impl<C: Component + ?Sized> CRef<C> {
    pub fn is_a<O: Component>(&self) -> bool {
        self.ctx.tid.0 == TypeId::of::<O>()
    }

    pub fn typed_id(&self) -> TypedComponentId {
        self.ctx.tid
    }

    pub fn parent(&self) -> GameObjectId {
        self.ctx.parent()
    }

    pub fn type_info(&self) -> Option<ReflectedTypeInfo> {
        type_info(self.ctx.tid.0)
    }

    #[allow(clippy::mut_from_ref)]
    pub fn get_mut(&self) -> &mut C {
        unsafe { &mut *(&raw const **self.data.as_ref().unwrap_unchecked() as *mut _) }
    }
}

impl CRef<dyn Component> {
    pub fn as_a<C: Component>(&self) -> Option<CRef<C>> {
        if !self.is_a::<C>() {
            return None;
        }
        let downcasted =
            Rc::downcast::<C>(unsafe { self.data.as_ref().unwrap_unchecked() }.clone()).ok()?;
        Some(CRef {
            data: Some(downcasted),
            ctx: self.ctx.clone(),
        })
    }
}

impl<C: Component + ?Sized> From<CRef<C>> for CWeak<C> {
    fn from(value: CRef<C>) -> Self {
        CWeak(value.ctx.tid.1, PhantomData)
    }
}

impl<C: ?Sized + Component> PartialEq<Self> for CRef<C> {
    fn eq(&self, other: &Self) -> bool {
        self.typed_id() == other.typed_id()
    }
}

impl<C: ?Sized + Component> Eq for CRef<C> {}

impl<C: Component> Debug for CRef<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Component").finish()
    }
}

impl<C: Component + ?Sized> Borrow<TypedComponentId> for CRef<C> {
    fn borrow(&self) -> &TypedComponentId {
        &self.ctx.tid
    }
}

impl<C: Component + ?Sized> Borrow<TypedComponentId> for &CRef<C> {
    fn borrow(&self) -> &TypedComponentId {
        &self.ctx.tid
    }
}

pub struct CWeak<C: Component + ?Sized>(pub(crate) ComponentId, pub(crate) PhantomData<C>);

impl<C: Component> Clone for CWeak<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: Component> Copy for CWeak<C> {}

impl<C: Component> PartialEq<Self> for CWeak<C> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<C: Component> Eq for CWeak<C> {}

impl<C: Component> Debug for CWeak<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Weak Component").finish()
    }
}

#[allow(unused)]
impl<C: Component> CWeak<C> {
    pub fn exists(&self, world: &World) -> bool {
        world
            .components
            ._get::<C>()
            .map(|c| c.contains_key(self.0))
            .unwrap_or(false)
    }

    pub fn upgrade(&self, world: &World) -> Option<CRef<C>> {
        world.components.get::<C>(self.0).cloned()
    }

    pub fn null() -> CWeak<C> {
        CWeak(ComponentId::null(), PhantomData)
    }

    delegate! {
        to self.0 {
            fn is_null(&self) -> bool;
        }
    }
}

impl<C: Component> Default for CWeak<C> {
    fn default() -> Self {
        CWeak::null()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct TypedComponentId(pub(crate) TypeId, pub(crate) ComponentId);

impl From<TypedComponentId> for ComponentId {
    fn from(value: TypedComponentId) -> Self {
        value.1
    }
}

impl TypedComponentId {
    pub fn is_a<C: Component>(&self) -> bool {
        self.0 == TypeId::of::<C>()
    }

    pub fn type_id(&self) -> TypeId {
        self.0
    }

    pub fn type_info(&self) -> Option<ReflectedTypeInfo> {
        type_info(self.0)
    }

    pub fn type_name(&self) -> Option<&'static str> {
        self.type_info().map(|info| info.full_path)
    }

    pub fn short_name(&self) -> Option<&'static str> {
        self.type_info().map(|info| info.name)
    }

    pub(crate) fn null<C: Component + ?Sized>() -> TypedComponentId {
        Self::from_typed::<C>(ComponentId::null())
    }

    pub(crate) fn from_typed<C: Component + ?Sized>(id: ComponentId) -> Self {
        TypedComponentId(TypeId::of::<C>(), id)
    }
}

/// A component attached to [`GameObject`](crate::core::GameObject).
///
/// Typical components include `Collider3D`, `MeshRenderer`, `AudioEmitter`, etc.
/// Can also be used to create custom game logic.
///
/// # Examples
///
/// ```rust
/// use nalgebra::Vector3;
/// use syrillian::World;
/// use syrillian::components::{Component, ComponentContext};
/// use syrillian::core::GameObjectId;
///
/// #[derive(Default)]
/// struct MyComponent;
///
/// impl Component for MyComponent {
///     fn init(&mut self, _world: &mut World) {
///         // Move the GameObject on attachment
///         self.parent().transform.translate(Vector3::new(1.0, 0.0, 0.0));
///     }
/// }
///```
#[allow(unused)]
pub trait Component: Any {
    // Gets called when the game object is created directly after new
    fn init(&mut self, world: &mut World) {}

    // Gets called when the component should update anything state-related
    fn update(&mut self, world: &mut World) {}

    // Gets called when the component should update any state that's necessary for physics
    fn late_update(&mut self, world: &mut World) {}

    // Gets called before physics are evolved
    fn pre_fixed_update(&mut self, world: &mut World) {}

    // Gets called after physics have evolved
    fn fixed_update(&mut self, world: &mut World) {}

    // Gets called after all other updates are done
    fn post_update(&mut self, world: &mut World) {}

    fn create_render_proxy(&mut self, world: &World) -> Option<Box<dyn SceneProxy>> {
        None
    }

    fn create_light_proxy(&mut self, world: &World) -> Option<Box<LightProxy>> {
        None
    }

    fn update_proxy(&mut self, world: &World, draw_ctx: CPUDrawCtx) {}

    fn on_click(&mut self, _world: &mut World) {}

    fn on_gui(&mut self, world: &mut World, ctx: UiContext) {}

    // Gets called when the component is about to be deleted
    fn delete(&mut self, world: &mut World) {}

    fn parent_opt(&self) -> Option<GameObjectId>
    where
        Self: Sized,
    {
        ComponentContextInference::tl_find(self as *const _ as *const ()).map(|ctx| ctx.parent)
    }

    fn world_opt(&self) -> Option<&'static mut World>
    where
        Self: Sized,
    {
        use syrillian::core::GOComponentExt;
        ComponentContextInference::tl_find(self as *const _).map(|ctx| ctx.world())
    }

    /// Will panic when Component is manually instantiated instead of adding it to a game object
    #[track_caller]
    fn parent(&self) -> GameObjectId
    where
        Self: Sized,
    {
        self.parent_opt()
            .expect("Tried to get component parent, but component wasn't managed by a world")
    }

    /// Will panic when Component is manually instantiated instead of adding it to a game object
    #[track_caller]
    fn world(&self) -> &'static mut World
    where
        Self: Sized,
    {
        self.world_opt()
            .expect("Tried to get component world, but component wasn't managed by a world")
    }
}
