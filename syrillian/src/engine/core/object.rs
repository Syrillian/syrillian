use crate::components::{CRef, Component, TypedComponentId};
use crate::core::Transform;
use crate::ensure_aligned;
use crate::world::World;
use itertools::Itertools;
use nalgebra::{Matrix4, Translation3, Vector3};
use slotmap::{Key, KeyData, new_key_type};
use std::borrow::Borrow;
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::ptr::null_mut;
use syrillian_utils::debug_panic;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct EventType(u32);

impl EventType {
    pub const CLICK: EventType = EventType(0b1);

    pub const fn empty() -> Self {
        EventType(0)
    }

    pub fn contains(self, other: EventType) -> bool {
        self.0 & other.0 != 0
    }

    pub fn toggle(self, other: EventType) -> EventType {
        EventType(self.0 ^ other.0)
    }

    pub fn insert(self, other: EventType) -> EventType {
        EventType(self.0 | other.0)
    }

    pub fn remove(self, other: EventType) -> EventType {
        EventType(self.0 & !other.0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

pub type ObjectHash = u32;

new_key_type! {
    /// Uniquely identifies a game object within the world.
    pub struct GameObjectId;
}

impl AsRef<GameObjectId> for GameObjectId {
    fn as_ref(&self) -> &GameObjectId {
        self
    }
}

/// Strong reference to a game object that keeps it alive until all references are dropped.
/// Prefer this over storing raw [`GameObjectId`] when you need to hold onto an object across frames.
///
/// ```
/// # use syrillian::World;
/// # use syrillian::core::GameObjectRef;
/// # fn demo(world: &mut World) {
/// let obj = world.new_object("Handle Demo");
/// let mut handle = world.get_object_ref(obj).unwrap();
/// handle.transform.translate([1.0, 0.0, 0.0].into());
/// let weak = handle.downgrade();
/// drop(handle);
/// assert!(weak.upgrade().is_none() || weak.upgrade().unwrap().is_alive());
/// # }
/// ```
#[derive(Debug)]
pub struct GameObjectRef {
    id: GameObjectId,
    ptr: *mut GameObject,
}

impl AsRef<GameObjectId> for GameObjectRef {
    fn as_ref(&self) -> &GameObjectId {
        &self.id
    }
}

unsafe impl Send for GameObjectRef {}

/// Weak reference to a game object that can be upgraded to a [`GameObjectRef`].
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, Hash)]
pub struct GameObjectWeak(GameObjectId);

impl AsRef<GameObjectId> for GameObjectWeak {
    fn as_ref(&self) -> &GameObjectId {
        &self.0
    }
}

#[allow(dead_code)]
impl GameObjectId {
    const INVALID_VALUE: u64 = 0x0000_0001_ffff_ffff;
    /// Returns `true` if `self` is non-null and is contained within the [`World`] instance.
    pub fn exists(&self) -> bool {
        if self.is_null() {
            return false;
        }

        let world = World::instance();
        let Some(obj) = world.objects.get(*self) else {
            return false;
        };

        obj.is_alive()
    }

    /// Chaining method that applies the function `f` to `self`.
    pub fn tap<F: Fn(&mut GameObject)>(mut self, f: F) -> Self {
        if self.exists() {
            f(self.deref_mut())
        }
        self
    }

    /// Creates a strong reference to this object, keeping it alive until the reference is dropped.
    pub fn upgrade(&self) -> Option<GameObjectRef> {
        GameObjectRef::new(*self)
    }

    /// Creates a weak reference to this object that can later be upgraded to a strong reference.
    pub fn downgrade(&self) -> GameObjectWeak {
        GameObjectWeak(*self)
    }

    pub(crate) fn as_ffi(&self) -> u64 {
        self.0.as_ffi()
    }

    pub(crate) fn from_ffi(id: u64) -> GameObjectId {
        GameObjectId(KeyData::from_ffi(id))
    }
}

// USING and STORING a GameObjectId is like a contract. It defines that you will recheck the
//  existence of this game object every time you re-use it. Otherwise, you will crash. Prefer
//  using GameObjectRef / GameObjectWeak when you need to hold onto an object across frames.
impl Deref for GameObjectId {
    type Target = GameObject;

    fn deref(&self) -> &GameObject {
        World::instance().objects.get(*self).unwrap()
    }
}

impl DerefMut for GameObjectId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        World::instance().objects.get_mut(*self).unwrap()
    }
}

impl GameObjectRef {
    pub(crate) fn new(id: GameObjectId) -> Option<Self> {
        let world = World::instance();
        Self::new_in_world(world, id)
    }

    pub(crate) fn new_in_world(world: &mut World, id: GameObjectId) -> Option<Self> {
        let ptr = {
            let entry = world.objects.get(id)?;
            if !entry.is_alive() && !world.has_object_refs(id) {
                return None;
            }
            entry.as_ref() as *const _ as *mut _
        };
        if !world.retain_object(id) {
            return None;
        }

        Some(GameObjectRef { id, ptr })
    }

    pub fn id(&self) -> GameObjectId {
        self.id
    }

    pub fn downgrade(&self) -> GameObjectWeak {
        GameObjectWeak(self.id)
    }

    pub fn is_alive(&self) -> bool {
        unsafe { self.ptr.as_ref().is_some_and(|g| g.is_alive()) }
    }

    /// # Safety
    ///
    /// This is uninitialized territory. If you use this, you'll need to make sure to
    /// overwrite it before using it. Accessing this in any way is UB.
    ///
    /// The only reason this exists is that you can avoid Option for References where objects
    /// are initialized right away but not on struct creation. It's not recommended to use this.
    pub unsafe fn null() -> GameObjectRef {
        GameObjectRef {
            id: GameObjectId::null(),
            ptr: null_mut(),
        }
    }
}

impl Clone for GameObjectRef {
    fn clone(&self) -> Self {
        let world = World::instance();
        if !world.retain_object(self.id) {
            panic!("Tried to clone GameObjectRef for deleted object");
        }

        GameObjectRef {
            id: self.id,
            ptr: self.ptr,
        }
    }
}

impl Drop for GameObjectRef {
    fn drop(&mut self) {
        if World::is_thread_loaded() {
            let world = World::instance();
            world.release_object(self.id);
        }
    }
}

impl Deref for GameObjectRef {
    type Target = GameObject;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref().unwrap() }
    }
}

impl DerefMut for GameObjectRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut().unwrap() }
    }
}

impl PartialEq for GameObjectRef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for GameObjectRef {}

impl Hash for GameObjectRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl GameObjectWeak {
    pub fn upgrade(&self) -> Option<GameObjectRef> {
        GameObjectRef::new(self.0)
    }

    pub fn id(&self) -> GameObjectId {
        self.0
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn exists(&self) -> bool {
        self.0.exists()
    }

    pub fn null() -> GameObjectWeak {
        GameObjectWeak(GameObjectId::null())
    }
}

/// Structure representing an object tree within the world.
///
/// A game object has a unique identifier and a non-unique name.
/// It keeps track of its parent-child relationships, applied
/// transformation, and attached components. If a game object has
/// no parent, it is a root-level game object within the world.
pub struct GameObject {
    /// A unique identifier for this object within the world.
    pub id: GameObjectId,
    /// The name of the object (not required to be unique).
    pub name: String,
    /// Whether the object is still alive inside the world.
    pub(crate) alive: Cell<bool>,
    /// Whether the object components will be called or the object is inactive
    pub(crate) enabled: Cell<bool>,
    /// Game objects that are direct children of this object.
    pub(crate) children: Vec<GameObjectId>,
    /// Parent game object.
    /// If `None`, this object is a root-level game object.
    pub(crate) parent: Option<GameObjectId>,
    /// The world this object belongs to
    pub(crate) owning_world: *mut World,
    /// The transformation applied to the object.
    pub transform: Transform,
    /// Components attached to this object.
    pub(crate) components: Vec<CRef<dyn Component>>,
    /// Custom Property Data (Keys & Values)
    pub(crate) custom_properties: HashMap<String, serde_json::Value>,
    /// Events this object is registered for.
    pub(crate) event_mask: Cell<EventType>,
    /// Unique hash used for picking and lookup.
    pub(crate) hash: ObjectHash,
}

impl GameObject {
    pub fn enable(&self) {
        self.enabled.set(true);
    }

    pub fn disable(&self) {
        self.enabled.set(false);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }
    
    /// Returns whether this object is still alive inside the world.
    pub fn is_alive(&self) -> bool {
        self.alive.get()
    }

    pub fn object_hash(&self) -> ObjectHash {
        self.hash
    }

    pub fn event_mask(&self) -> EventType {
        self.event_mask.get()
    }

    pub fn is_notified_for(&self, event: EventType) -> bool {
        self.event_mask().contains(event)
    }

    pub fn notify_for(&self, world: &mut World, event: EventType) {
        let current = self.event_mask();
        let new_mask = current.insert(event);
        self.event_mask.set(new_mask);
        world.update_event_registration(self.id, current, new_mask);
    }

    pub fn stop_notify_for(&self, world: &mut World, event: EventType) {
        let current = self.event_mask();
        let new_mask = current.remove(event);
        self.event_mask.set(new_mask);
        world.update_event_registration(self.id, current, new_mask);
    }

    pub(crate) fn mark_dead(&self) {
        self.alive.set(false);
    }

    /// Returns the parent as a strong reference if it is still alive.
    pub fn parent_ref(&self) -> Option<GameObjectRef> {
        self.parent.and_then(|p| p.upgrade())
    }

    /// Returns all children as strong references, filtering out already deleted objects.
    pub fn child_refs(&self) -> Vec<GameObjectRef> {
        self.children
            .iter()
            .filter_map(GameObjectId::upgrade)
            .collect()
    }

    /// Unlinks this game object from its parent or the world (root level).
    pub fn unlink(&mut self) {
        if let Some(mut parent) = self.parent.take() {
            let pos_opt = parent
                .children
                .iter()
                .find_position(|other| self.id == **other)
                .map(|(id, _)| id);
            if let Some(pos) = pos_opt {
                parent.children.remove(pos);
            }
        } else {
            let world = self.world();
            if let Some(pos) = world
                .children
                .iter()
                .find_position(|other| self.id == other.id)
            {
                world.children.remove(pos.0);
            }
        }
    }

    /// Adds another game object as a child of this one, replacing the child's previous parent relationship.
    pub fn add_child(&mut self, mut child: GameObjectId) {
        if !self.is_alive() || !child.exists() {
            return;
        }
        // unlink from previous parent or world
        child.unlink();

        self.children.push(child);
        child.parent = Some(self.id);
    }

    /// Adds a new [`Component`] of type `C` to this game object, initializing the component within the world,
    /// and returns the component ID.
    pub fn add_component<C>(&mut self) -> CRef<C>
    where
        C: Component + Default + 'static,
    {
        assert!(
            self.is_alive(),
            "cannot add a component to an object that has been deleted"
        );
        let world = self.world();
        let comp: C = C::default();
        let mut new_comp = world.components.add(comp, self.id);

        if self
            .components
            .iter()
            .any(|c| c.ctx.tid == new_comp.ctx.tid)
        {
            debug_panic!("Tried to add the same component again to the same object");
        }

        let new_comp2 = new_comp.clone();
        self.components.push(new_comp.as_dyn());

        new_comp.init(world);
        new_comp2
    }

    /// Adds a new [`Component`] of type `C` to all children of this game object.
    pub fn add_child_components<C>(&mut self)
    where
        C: Component + Default + 'static,
    {
        if !self.is_alive() {
            return;
        }
        for child in &mut self.children {
            child.add_component::<C>();
        }
    }

    /// Adds a new [`Component`] of type `C` to all children of this game object, and applies the provided
    /// function `f` to each newly added component.
    pub fn add_child_components_then<C>(&mut self, f: impl Fn(&mut C))
    where
        C: Component + Default + 'static,
    {
        if !self.is_alive() {
            return;
        }
        for child in &mut self.children {
            let mut comp = child.add_component::<C>();
            f(&mut comp);
        }
    }

    /// Add a custom property to this object
    pub fn add_property(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.custom_properties.insert(key.into(), value);
    }

    /// Add a collection of custom properties to this object
    pub fn add_properties<T: IntoIterator<Item = (String, serde_json::Value)>>(
        &mut self,
        properties: T,
    ) {
        self.custom_properties.extend(properties);
    }

    /// Retrieve a custom property in this object by the given key
    pub fn property(&self, key: &str) -> Option<&serde_json::Value> {
        self.custom_properties.get(key)
    }

    /// Checks if the object has a property with the given key
    pub fn has_property(&self, key: &str) -> bool {
        self.custom_properties.contains_key(key)
    }

    /// Retrieve all custom properties of this object
    pub fn properties(&self) -> &HashMap<String, serde_json::Value> {
        &self.custom_properties
    }

    /// Remove property from this object by the given key
    pub fn remove_property(&mut self, key: &str) -> Option<serde_json::Value> {
        self.custom_properties.remove(key)
    }

    /// Remove property from this object by the given key
    pub fn clear_properties(&mut self) {
        self.custom_properties.clear();
    }

    /// Retrieves the first found [`Component`] of type `C` attached to this game object.
    pub fn get_component<C: Component + 'static>(&self) -> Option<CRef<C>> {
        self.components.iter().find_map(|c| c.as_a::<C>())
    }

    /// Returns an iterator over all [`Component`] of type `C` attached to this game object.
    pub fn iter_dyn_components(&self) -> impl Iterator<Item = &CRef<dyn Component>> {
        self.components.iter()
    }

    /// Returns an iterator over all [`Component`] of type `C` attached to this game object.
    pub fn iter_components<C: Component + 'static>(&self) -> impl Iterator<Item = CRef<C>> {
        self.components.iter().filter_map(|c| c.clone().as_a())
    }

    /// Retrieves the first found [`Component`] of type `C` attached to a child of this game object.
    pub fn get_child_component<C>(&mut self) -> Option<CRef<C>>
    where
        C: Component + 'static,
    {
        for child in &mut self.children {
            if let Some(comp) = child.get_component::<C>() {
                return Some(comp);
            }
        }

        None
    }

    /// Removes a [`Component`] by id from this game object and the world.
    pub fn remove_component(&mut self, comp: impl Borrow<TypedComponentId>, world: &mut World) {
        let comp = *comp.borrow();
        let removed = self
            .components
            .extract_if(.., |c| c.ctx.tid == comp)
            .count();
        if removed > 1 {
            debug_panic!("Removed more than one component by TID (which should be unique)");
        }
        world.components.remove(comp);
    }

    /// Returns an immutable reference to this game object's parent ID.
    pub fn parent(&self) -> &Option<GameObjectId> {
        &self.parent
    }

    /// Collects the list of parents up to the root.
    pub fn parents(&self) -> Vec<GameObjectId> {
        let mut parents = vec![];
        let mut parent_opt = Some(self.id);

        while let Some(parent) = parent_opt {
            parents.push(parent);
            parent_opt = *parent.parent();
        }
        parents.reverse();

        parents
    }

    /// Returns an immutable slice of this game object's child IDs.
    pub fn children(&self) -> &[GameObjectId] {
        &self.children
    }

    /// Destroys this game object tree, cleaning up any component-specific data,
    /// then unlinks and removes the object from the world.
    pub fn delete(&mut self) {
        if !self.alive.replace(false) {
            return;
        }

        for mut child in self.children.iter().copied() {
            child.delete();
        }

        let world = self.world();
        for mut comp in self.components.drain(..) {
            comp.delete(world);
            world.components.remove(&comp);
        }

        self.children.clear();
        self.unlink();
        world.schedule_object_removal(self.id);
    }

    pub fn world(&self) -> &'static mut World {
        unsafe { &mut *self.owning_world }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelUniform {
    pub model_mat: Matrix4<f32>,
}

ensure_aligned!(ModelUniform { model_mat }, align <= 16 * 4 => size);

impl ModelUniform {
    pub fn empty() -> Self {
        ModelUniform {
            model_mat: Matrix4::identity(),
        }
    }

    pub fn new_at(x: f32, y: f32, z: f32) -> Self {
        ModelUniform {
            model_mat: Translation3::new(x, y, z).to_homogeneous(),
        }
    }

    pub fn new_at_vec(pos: Vector3<f32>) -> Self {
        ModelUniform {
            model_mat: Translation3::from(pos).to_homogeneous(),
        }
    }

    pub fn from_matrix(translation: &Matrix4<f32>) -> Self {
        ModelUniform {
            model_mat: *translation,
        }
    }

    pub fn update(&mut self, transform: &Matrix4<f32>) {
        self.model_mat = *transform;
    }
}
