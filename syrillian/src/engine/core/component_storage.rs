use crate::components::{CRef, Component, ComponentId, TypedComponentId};
use crate::core::GameObjectId;
use crate::core::component_context_inference::ComponentContextInference;
use slotmap::SlotMap;
use slotmap::basic::Values;
use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;
use tracing::trace;

#[allow(unused)]
pub(crate) trait SlotMapUntyped<K>
where
    K: slotmap::Key + 'static,
{
    fn as_dyn(&self) -> &dyn Any;
    fn as_dyn_mut(&mut self) -> &mut dyn Any;
    fn iter_refs<'a>(&'a self) -> Box<(dyn Iterator<Item = CRef<dyn Component>> + 'a)>;
    fn iter_comps<'a>(&'a self) -> Box<dyn Iterator<Item = &'a dyn Component> + 'a>;
    fn iter_comps_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = &'a mut dyn Component> + 'a>;
    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (K, &'a dyn Component)> + 'a>;
    fn iter_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = (K, &'a mut dyn Component)> + 'a>;
    fn get(&self, key: K) -> Option<CRef<dyn Component>>;
    fn get_mut(&mut self, key: K) -> Option<CRef<dyn Component>>;
    fn remove(&mut self, key: K) -> Option<CRef<dyn Component>>;
}

impl<K, V> SlotMapUntyped<K> for SlotMap<K, CRef<V>>
where
    K: slotmap::Key + 'static,
    V: Component,
{
    fn as_dyn(&self) -> &dyn Any {
        self
    }

    fn as_dyn_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn iter_refs<'a>(&'a self) -> Box<dyn Iterator<Item = CRef<dyn Component>> + 'a> {
        Box::new(self.values().map(|v| v.as_dyn()))
    }

    fn iter_comps<'a>(&'a self) -> Box<dyn Iterator<Item = &'a dyn Component> + 'a> {
        Box::new(self.values().map(|v| &**v as &dyn Component))
    }

    fn iter_comps_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = &'a mut dyn Component> + 'a> {
        Box::new(self.values_mut().map(|v| &mut **v as &mut dyn Component))
    }

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (K, &'a dyn Component)> + 'a> {
        Box::new(self.iter().map(|(k, v)| (k, &**v as &dyn Component)))
    }

    fn iter_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = (K, &'a mut dyn Component)> + 'a> {
        Box::new(
            self.iter_mut()
                .map(|(k, v)| (k, &mut **v as &mut dyn Component)),
        )
    }

    fn get(&self, key: K) -> Option<CRef<dyn Component>> {
        self.get(key).map(|v| v.as_dyn())
    }

    fn get_mut(&mut self, key: K) -> Option<CRef<dyn Component>> {
        self.get_mut(key).map(|v| v.as_dyn())
    }

    fn remove(&mut self, key: K) -> Option<CRef<dyn Component>> {
        self.remove(key).map(|v| v.as_dyn())
    }
}

#[derive(Default)]
pub struct ComponentStorage {
    inner: HashMap<TypeId, Box<dyn SlotMapUntyped<ComponentId>>>,
    len: usize,
    pub(crate) fresh: Vec<TypedComponentId>,
    pub(crate) removed: Vec<TypedComponentId>,
}

impl ComponentStorage {
    pub(crate) fn _get_from_id(&self, tid: TypeId) -> Option<&dyn SlotMapUntyped<ComponentId>> {
        Some(self.inner.get(&tid)?.as_ref())
    }

    pub(crate) fn _get_mut_from_id(
        &mut self,
        tid: TypeId,
    ) -> Option<&mut dyn SlotMapUntyped<ComponentId>> {
        Some(self.inner.get_mut(&tid)?.as_mut())
    }

    pub(crate) fn _get<C: Component>(&self) -> Option<&SlotMap<ComponentId, CRef<C>>> {
        let tid = TypeId::of::<C>();

        let typed = self
            ._get_from_id(tid)?
            .as_dyn()
            .downcast_ref::<SlotMap<ComponentId, CRef<C>>>()
            .expect("Type ID was checked");

        Some(typed)
    }

    pub(crate) fn _get_mut<C: Component>(&mut self) -> Option<&mut SlotMap<ComponentId, CRef<C>>> {
        let tid = TypeId::of::<C>();

        let typed = self
            ._get_mut_from_id(tid)?
            .as_dyn_mut()
            .downcast_mut::<SlotMap<ComponentId, CRef<C>>>()
            .expect("Type ID was checked");

        Some(typed)
    }

    pub(crate) fn _get_or_insert_mut<C: Component>(
        &mut self,
    ) -> &mut SlotMap<ComponentId, CRef<C>> {
        let tid = TypeId::of::<C>();
        self.inner
            .entry(tid)
            .or_insert_with(|| Box::new(SlotMap::<ComponentId, CRef<C>>::with_key()))
            .as_dyn_mut()
            .downcast_mut()
            .expect("Type ID was checked")
    }

    pub fn get<C: Component>(&self, id: impl Into<ComponentId>) -> Option<&CRef<C>> {
        self._get()?.get(id.into())
    }

    pub fn get_mut<C: Component>(&mut self, id: TypedComponentId) -> Option<&mut CRef<C>> {
        self._get_mut()?.get_mut(id.1)
    }

    pub fn get_dyn(&self, id: TypedComponentId) -> Option<CRef<dyn Component>> {
        self._get_from_id(id.0)?.get(id.1)
    }

    pub fn values_of_type<C: Component>(&self) -> Option<Values<'_, ComponentId, CRef<C>>> {
        Some(self._get()?.values())
    }

    pub fn iter(&self) -> impl Iterator<Item = (TypedComponentId, &dyn Component)> {
        self.inner
            .iter()
            .flat_map(|(tid, store)| store.iter().map(|(k, v)| (TypedComponentId(*tid, k), v)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (TypedComponentId, &mut dyn Component)> {
        self.inner.iter_mut().flat_map(|(tid, store)| {
            store
                .iter_mut()
                .map(|(k, v)| (TypedComponentId(*tid, k), v))
        })
    }

    pub fn iter_refs(&self) -> impl Iterator<Item = CRef<dyn Component>> {
        self.inner.values().flat_map(|store| store.iter_refs())
    }

    pub fn values(&self) -> impl Iterator<Item = &dyn Component> {
        self.inner.values().flat_map(|store| store.iter_comps())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut dyn Component> {
        self.inner
            .values_mut()
            .flat_map(|store| store.iter_comps_mut())
    }

    pub(crate) fn add<C: Component>(&mut self, component: C, parent: GameObjectId) -> CRef<C> {
        let comp = Rc::new(component);

        let comp_ptr = Rc::as_ptr(&comp);

        let store = self._get_or_insert_mut();
        let id = store.insert_with_key(|id| {
            let tid = TypedComponentId::from_typed::<C>(id);
            CRef::new(comp, tid, parent)
        });

        let tid = TypedComponentId::from_typed::<C>(id);
        let cref = store.get(id).expect("Element was just inserted").clone();

        ComponentContextInference::tl_insert(comp_ptr, cref.ctx.clone());

        self.len += 1;
        self.fresh.push(tid);
        cref
    }

    pub(crate) fn remove(&mut self, ctid: impl Borrow<TypedComponentId>) {
        trace!("Removed component");

        let ctid = *ctid.borrow();

        let Some(map) = self._get_mut_from_id(ctid.0) else {
            // already empty
            return;
        };

        let comp = map.remove(ctid.1);
        debug_assert!(
            comp.is_some(),
            "Component wasn't found despite still being owned by a game object."
        );
        self.removed.push(ctid);

        debug_assert_ne!(self.len, 0);

        self.len = self.len.saturating_sub(1);

        if let Some(comp) = comp.and_then(|c| c.data) {
            ComponentContextInference::tl_remove(Rc::as_ptr(&comp) as *const ());
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}
