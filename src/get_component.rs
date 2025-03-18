use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::atomic_refcell::{ARef, ARefMut};
use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::error;
use crate::sparse_set::SparseSet;
use crate::tracking::TrackingTimestamp;
use core::any::type_name;
use core::ops::{Deref, DerefMut};

/// Shared reference to a component.
pub struct Ref<'a, T> {
    inner: T,
    all_borrow: Option<SharedBorrow<'a>>,
    borrow: SharedBorrow<'a>,
}

impl<'a, T> Ref<'a, T> {
    /// Makes a new [`Ref`].
    ///
    /// This is an associated function that needs to be used as `Ref::map(...)`. A method would interfere with methods of the same name used through Deref.
    #[inline]
    pub fn map<U, F: FnOnce(T) -> U>(orig: Self, f: F) -> Ref<'a, U> {
        Ref {
            inner: f(orig.inner),
            all_borrow: orig.all_borrow,
            borrow: orig.borrow,
        }
    }
}

impl<'a, T> Deref for Ref<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T> AsRef<T> for Ref<'a, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

/// Exclusive reference to a component.
pub struct RefMut<'a, T> {
    inner: T,
    flag: Option<&'a mut TrackingTimestamp>,
    current: TrackingTimestamp,
    all_borrow: Option<SharedBorrow<'a>>,
    borrow: ExclusiveBorrow<'a>,
}

impl<'a, T> RefMut<'a, T> {
    /// Makes a new [`RefMut`], the component will not be flagged if its modified inside `f`.
    ///
    /// This is an associated function that needs to be used as `RefMut::map(...)`. A method would interfere with methods of the same name used through Deref.
    #[inline]
    pub fn map<U, F: FnOnce(T) -> U>(orig: Self, f: F) -> RefMut<'a, U> {
        RefMut {
            inner: f(orig.inner),
            flag: orig.flag,
            current: orig.current,
            all_borrow: orig.all_borrow,
            borrow: orig.borrow,
        }
    }
}

impl<'a, T> Deref for RefMut<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T> AsRef<T> for RefMut<'a, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<'a, T> DerefMut for RefMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let Some(flag) = &mut self.flag {
            **flag = self.current;
        }

        &mut self.inner
    }
}

impl<'a, T> AsMut<T> for RefMut<'a, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        if let Some(flag) = &mut self.flag {
            **flag = self.current;
        }

        &mut self.inner
    }
}

/// Trait used as bound for [`World::get`] and [`AllStorages::get`].
///
/// [`World::get`]: crate::World::get
pub trait GetComponent {
    #[allow(missing_docs)]
    type Out<'a>;

    #[allow(missing_docs)]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent>;
}

impl<T: Component + Send + Sync> GetComponent for &'_ T {
    type Out<'a> = Ref<'a, &'a T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        _current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view = all_storages.custom_storage_or_insert(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        Ok(Ref {
            inner: sparse_set
                .private_get(entity)
                .ok_or_else(|| error::MissingComponent {
                    id: entity,
                    name: type_name::<T>(),
                })?,
            all_borrow,
            borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> GetComponent for NonSend<&'_ T> {
    type Out<'a> = Ref<'a, &'a T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        _current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view = all_storages.custom_storage_or_insert_non_send(|| NonSend(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        Ok(Ref {
            inner: sparse_set
                .private_get(entity)
                .ok_or_else(|| error::MissingComponent {
                    id: entity,
                    name: type_name::<T>(),
                })?,
            all_borrow,
            borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> GetComponent for NonSync<&'_ T> {
    type Out<'a> = Ref<'a, &'a T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        _current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view = all_storages.custom_storage_or_insert_non_sync(|| NonSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        Ok(Ref {
            inner: sparse_set
                .private_get(entity)
                .ok_or_else(|| error::MissingComponent {
                    id: entity,
                    name: type_name::<T>(),
                })?,
            all_borrow,
            borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> GetComponent for NonSendSync<&'_ T> {
    type Out<'a> = Ref<'a, &'a T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        _current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view = all_storages
            .custom_storage_or_insert_non_send_sync(|| NonSendSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        Ok(Ref {
            inner: sparse_set
                .private_get(entity)
                .ok_or_else(|| error::MissingComponent {
                    id: entity,
                    name: type_name::<T>(),
                })?,
            all_borrow,
            borrow,
        })
    }
}

impl<T: Component + Send + Sync> GetComponent for &'_ mut T {
    type Out<'a> = RefMut<'a, &'a mut T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view = all_storages.custom_storage_or_insert_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        let index = sparse_set
            .index_of(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })?;

        let SparseSet {
            data,
            modification_data,
            is_tracking_modification,
            ..
        } = sparse_set;

        Ok(RefMut {
            inner: unsafe { data.get_unchecked_mut(index) },
            flag: is_tracking_modification
                .then(|| unsafe { modification_data.get_unchecked_mut(index) }),
            current,
            all_borrow,
            borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> GetComponent for NonSend<&'_ mut T> {
    type Out<'a> = RefMut<'a, &'a mut T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view =
            all_storages.custom_storage_or_insert_non_send_mut(|| NonSend(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        let index = sparse_set
            .index_of(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })?;

        let NonSend(SparseSet {
            data,
            modification_data,
            is_tracking_modification,
            ..
        }) = sparse_set;

        Ok(RefMut {
            inner: unsafe { data.get_unchecked_mut(index) },
            flag: is_tracking_modification
                .then(|| unsafe { modification_data.get_unchecked_mut(index) }),
            current,
            all_borrow,
            borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> GetComponent for NonSync<&'_ mut T> {
    type Out<'a> = RefMut<'a, &'a mut T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view =
            all_storages.custom_storage_or_insert_non_sync_mut(|| NonSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        let index = sparse_set
            .index_of(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })?;

        let NonSync(SparseSet {
            data,
            modification_data,
            is_tracking_modification,
            ..
        }) = sparse_set;

        Ok(RefMut {
            inner: unsafe { data.get_unchecked_mut(index) },
            flag: is_tracking_modification
                .then(|| unsafe { modification_data.get_unchecked_mut(index) }),
            current,
            all_borrow,
            borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> GetComponent for NonSendSync<&'_ mut T> {
    type Out<'a> = RefMut<'a, &'a mut T>;

    #[inline]
    fn get<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        entity: EntityId,
    ) -> Result<Self::Out<'a>, error::GetComponent> {
        let view = all_storages
            .custom_storage_or_insert_non_send_sync_mut(|| NonSendSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        let index = sparse_set
            .index_of(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })?;

        let NonSendSync(SparseSet {
            data,
            modification_data,
            is_tracking_modification,
            ..
        }) = sparse_set;

        Ok(RefMut {
            inner: unsafe { data.get_unchecked_mut(index) },
            flag: is_tracking_modification
                .then(|| unsafe { modification_data.get_unchecked_mut(index) }),
            current,
            all_borrow,
            borrow,
        })
    }
}

macro_rules! impl_get_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: GetComponent),+> GetComponent for ($($type,)+) {
            type Out<'a> = ($($type::Out<'a>,)+);
            #[inline]
            fn get<'a>(
                all_storages: &'a AllStorages,
                all_borrow: Option<SharedBorrow<'a>>,
                current: TrackingTimestamp,
                entity: EntityId,
            ) -> Result<Self::Out<'a>, error::GetComponent> {
                Ok(($($type::get(all_storages, all_borrow.clone(), current, entity)?,)+))
            }
        }
    }
}

macro_rules! get_component {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_get_component![$(($type, $index))*];
        get_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_get_component![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
get_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
#[cfg(feature = "extended_tuple")]
get_component![
    (ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)
    (ViewK, 10) (ViewL, 11) (ViewM, 12) (ViewN, 13) (ViewO, 14) (ViewP, 15) (ViewQ, 16) (ViewR, 17) (ViewS, 18) (ViewT, 19)
    (ViewU, 20) (ViewV, 21) (ViewW, 22) (ViewX, 23) (ViewY, 24) (ViewZ, 25) (ViewAA, 26) (ViewBB, 27) (ViewCC, 28) (ViewDD, 29)
    (ViewEE, 30) (ViewFF, 31)
];
