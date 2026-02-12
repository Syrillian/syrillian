use super::{H, key::AssetKey};
use dashmap::DashMap;
use dashmap::iter::{Iter, IterMut};
use dashmap::mapref::one::Ref as MapRef;
use dashmap::mapref::one::RefMut as MapRefMut;
use parking_lot::RwLock;
#[cfg(debug_assertions)]
use std::backtrace::Backtrace;
use std::fmt::{Debug, Display, Formatter};
use std::mem;
use std::ops::{Deref, DerefMut};
#[cfg(debug_assertions)]
use std::time::Duration;
use tracing::{trace, warn};
#[cfg(debug_assertions)]
use web_time::Instant;

#[cfg(not(debug_assertions))]
pub struct Ref<'a, T>(MapRef<'a, AssetKey, T>);
#[cfg(not(debug_assertions))]
pub struct RefMut<'a, T>(MapRefMut<'a, AssetKey, T>);

#[cfg(not(debug_assertions))]
impl<'a, T> From<MapRef<'a, AssetKey, T>> for Ref<'a, T> {
    fn from(other: MapRef<'a, AssetKey, T>) -> Ref<'a, T> {
        Ref(other)
    }
}

#[cfg(not(debug_assertions))]
impl<'a, T> From<MapRefMut<'a, AssetKey, T>> for RefMut<'a, T> {
    fn from(other: MapRefMut<'a, AssetKey, T>) -> RefMut<'a, T> {
        RefMut(other)
    }
}

#[cfg(debug_assertions)]
pub struct Ref<'a, T: StoreType>(MapRef<'a, AssetKey, T>, Instant);
#[cfg(debug_assertions)]
pub struct RefMut<'a, T: StoreType>(MapRefMut<'a, AssetKey, T>, Instant);

#[cfg(debug_assertions)]
impl<'a, T: StoreType> From<MapRef<'a, AssetKey, T>> for Ref<'a, T> {
    fn from(other: MapRef<'a, AssetKey, T>) -> Ref<'a, T> {
        Ref(other, Instant::now())
    }
}

#[cfg(debug_assertions)]
impl<'a, T: StoreType> From<MapRefMut<'a, AssetKey, T>> for RefMut<'a, T> {
    fn from(other: MapRefMut<'a, AssetKey, T>) -> RefMut<'a, T> {
        RefMut(other, Instant::now())
    }
}

#[cfg(debug_assertions)]
impl<'a, T: StoreType> Drop for Ref<'a, T> {
    fn drop(&mut self) {
        if self.1.elapsed() > Duration::from_secs_f32(1.0 / 60.0) {
            warn!(
                "Access to a {:?} Asset Store Object took {}s\n{}",
                T::NAME,
                self.1.elapsed().as_secs_f32(),
                Backtrace::capture()
            );
        }
    }
}

#[cfg(debug_assertions)]
impl<'a, T: StoreType> Drop for RefMut<'a, T> {
    fn drop(&mut self) {
        if self.1.elapsed() > Duration::from_secs(1) {
            warn!(
                "Mutable Access to a {:?} Asset Store Object took {}s\n{}",
                T::NAME,
                self.1.elapsed().as_secs_f32(),
                Backtrace::capture()
            );
        }
    }
}

impl<'a, T: StoreType> Deref for Ref<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, T: StoreType> Deref for RefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, T: StoreType> DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

pub struct Store<T: StoreType> {
    data: DashMap<AssetKey, T>,
    next_id: RwLock<u32>,
    dirty: RwLock<Vec<AssetKey>>,
}

pub trait StoreDefaults: StoreType {
    fn populate(store: &mut Store<Self>);
}

pub trait StoreType: Sized + Debug + Clone {
    const NAME: &str;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self>;
    fn ident(handle: H<Self>) -> String {
        match Self::ident_fmt(handle) {
            HandleName::Static(name) => name.to_string(),
            HandleName::Id(id) => format!("{} #{id}", Self::NAME),
        }
    }

    fn store<S: AsRef<Store<Self>>>(self, store: &S) -> H<Self> {
        store.as_ref().add(self)
    }
    fn is_builtin(handle: H<Self>) -> bool;
}

pub trait StoreTypeFallback: StoreType {
    fn fallback() -> H<Self>;
}

pub enum HandleName<T: StoreType> {
    Static(&'static str),
    Id(H<T>),
}

impl<T: StoreType> Store<T> {
    pub fn empty() -> Self {
        Self {
            data: DashMap::new(),
            next_id: RwLock::new(0),
            dirty: RwLock::default(),
        }
    }
}

impl<T: StoreDefaults> Store<T> {
    pub fn populated() -> Self {
        let mut store = Self::empty();
        store.populate();
        store
    }

    pub fn populate(&mut self) {
        T::populate(self);
    }
}

impl<T: StoreType> Display for HandleName<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleName::Static(s) => write!(f, "\"{s}\"",),
            HandleName::Id(id) => write!(f, "#{id}"),
        }
    }
}

impl<T: StoreType> Store<T> {
    fn next_id(&self) -> H<T> {
        let mut id_lock = self.next_id.write();
        let id = H::new(*id_lock);
        *id_lock += 1;
        id
    }

    pub fn add<T2: Into<T>>(&self, elem: T2) -> H<T> {
        let id = self.next_id();
        self.data.insert(id.into(), elem.into());

        trace!("[{} Store] Added element: {}", T::NAME, T::ident_fmt(id));

        id
    }

    pub fn try_get(&self, h: H<T>) -> Option<Ref<'_, T>> {
        self.data
            .get(&h.into())
            .or_else(|| {
                warn!(
                    "[{} Store] Invalid Reference: h={} not found",
                    T::NAME,
                    T::ident_fmt(h)
                );
                None
            })
            .map(|i| i.into())
    }

    pub fn try_get_mut(&self, h: H<T>) -> Option<RefMut<'_, T>> {
        let reference = self.data.get_mut(&h.into()).or_else(|| {
            warn!(
                "[{} Store] Invalid Reference: h={} not found",
                T::NAME,
                T::ident_fmt(h)
            );
            None
        });

        if reference.is_some() {
            self.set_dirty(h.into());
        }

        reference.map(|i| i.into())
    }

    fn set_dirty(&self, h: AssetKey) {
        let mut dirty_store = self.dirty.write();
        if !dirty_store.contains(&h) {
            trace!("Set {} {} dirty", T::NAME, T::ident(h.into()));
            dirty_store.push(h);
        }
    }

    pub fn pop_dirty(&self) -> Vec<AssetKey> {
        let mut dirty_store = self.dirty.write();
        let mut swap_store = Vec::new();
        mem::swap::<Vec<AssetKey>>(dirty_store.as_mut(), swap_store.as_mut());

        swap_store
    }

    pub fn remove(&self, h: H<T>) -> Option<T> {
        if h.is_builtin() {
            return None;
        }
        let key = h.into();
        let item = self.data.remove(&key);
        self.set_dirty(key);
        Some(item?.1)
    }

    pub fn contains(&self, handle: H<T>) -> bool {
        self.data.contains_key(&handle.into())
    }

    pub fn items(&self) -> Iter<'_, AssetKey, T> {
        self.data.iter()
    }

    pub fn items_mut(&self) -> IterMut<'_, AssetKey, T> {
        self.data.iter_mut()
    }
}

impl<T: StoreTypeFallback> Store<T> {
    pub fn get(&self, h: H<T>) -> Ref<'_, T> {
        if !self.data.contains_key(&h.into()) {
            let fallback = self.try_get(T::fallback());
            match fallback {
                Some(elem) => elem,
                None => unreachable!("Fallback items should always be populated"),
            }
        } else {
            let data = self.data.get(&h.into());
            match data {
                Some(elem) => elem.into(),
                None => unreachable!("Item was checked previously"),
            }
        }
    }

    pub fn get_mut(&self, h: H<T>) -> RefMut<'_, T> {
        if !self.data.contains_key(&h.into()) {
            let fallback = self.try_get_mut(T::fallback());
            match fallback {
                Some(elem) => elem,
                None => unreachable!("Fallback items should always be populated"),
            }
        } else {
            let data = self.data.get_mut(&h.into());
            self.set_dirty(h.into());
            match data {
                Some(elem) => elem.into(),
                None => unreachable!("Item was checked previously"),
            }
        }
    }
}

impl<T: StoreType> AsRef<Store<T>> for Store<T> {
    fn as_ref(&self) -> &Store<T> {
        self
    }
}

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! store_add_checked {
    ($store:ident, $expected_id:path, $elem:expr) => {
        let id = $store.add($elem);
        assert_eq!(id.id(), $expected_id);
    };
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! store_add_checked {
    ($store:ident, $expected_id:path, $elem:expr) => {
        $store.add($elem);
    };
}

#[macro_export]
macro_rules! store_add_checked_many {
    ($store:ident, $( $expected_id:path => $elem:expr ),+ $(,)?) => {
        $( store_add_checked!($store, $expected_id, $elem); )*
    }
}
