use crate::assets::Store;
use crate::engine::assets::HandleName;
use crate::engine::assets::generic_store::StoreType;
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct H<T: StoreType>(u32, PhantomData<T>);

// Manually impl all the traits that should be derived, which has actual downsides like completely
// wasting the const-functionality of those traits, but has the very big upside of not relating the
// handle derives with the generic bounds, meaning the Store types don't need to implement them.
//
// Except Debug, because the Store Types should all impl Debug.

impl<T: StoreType> Clone for H<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: StoreType> Copy for H<T> {}

impl<T: StoreType> PartialEq for H<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: StoreType> Eq for H<T> {}

impl<T: StoreType> PartialOrd for H<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: StoreType> Ord for H<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T: StoreType> Hash for H<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: StoreType> H<T> {
    #[inline(always)]
    pub(crate) const fn new(id: u32) -> Self {
        H(id, PhantomData)
    }

    #[inline(always)]
    pub(crate) const fn id(&self) -> u32 {
        self.0
    }

    pub fn ident(&self) -> String {
        T::ident(*self)
    }

    pub fn ident_fmt(&self) -> HandleName<T> {
        T::ident_fmt(*self)
    }

    pub fn is_builtin(&self) -> bool {
        T::is_builtin(*self)
    }

    pub fn exists<R: AsRef<Store<T>>>(&self, store: R) -> bool {
        store.as_ref().contains(*self)
    }
}

impl<T: StoreType> Display for H<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}
