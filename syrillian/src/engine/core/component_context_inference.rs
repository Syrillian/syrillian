use crate::components::ComponentContext;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

thread_local! {
    static CTX_INFER: Rc<RefCell<ComponentContextInference>> = Rc::default();
}

#[derive(Default)]
pub struct ComponentContextInference {
    mapper: HashMap<*mut (), Rc<ComponentContext>>,
}

impl ComponentContextInference {
    pub fn find<T>(&self, comp: *const T) -> Option<Rc<ComponentContext>> {
        self.mapper.get(&(comp as *mut ())).cloned()
    }

    pub fn tl_find<T>(comp: *const T) -> Option<Rc<ComponentContext>> {
        CTX_INFER.with(|i| i.borrow().find(comp))
    }

    pub fn insert<T>(
        &mut self,
        comp: *const T,
        ctx: Rc<ComponentContext>,
    ) -> Option<Rc<ComponentContext>> {
        self.mapper.insert(comp as *mut (), ctx)
    }

    pub fn tl_insert<T>(comp: *const T, ctx: Rc<ComponentContext>) -> Option<Rc<ComponentContext>> {
        CTX_INFER.with(|i| i.borrow_mut().insert(comp, ctx))
    }

    pub fn remove<T>(&mut self, comp: *const T) -> Option<Rc<ComponentContext>> {
        self.mapper.remove(&(comp as *mut ()))
    }

    pub fn tl_remove<T>(comp: *const T) -> Option<Rc<ComponentContext>> {
        CTX_INFER.with(|i| i.borrow_mut().remove(comp))
    }
}
