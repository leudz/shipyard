mod iter_ref;

pub use iter_ref::{IntoIterRef, IterRef};

use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::borrow::Borrow;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::{AbstractMut, Iter, Mixed, Tight};
use crate::r#mut::Mut;
use crate::sparse_set::SparseSet;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::Tracking;
use crate::tracking::TrackingTimestamp;
use crate::views::{View, ViewMut};
use crate::{error, track};
use alloc::vec::Vec;
use core::any::{type_name, TypeId};
use core::ptr;

const ACCESS_FACTOR: usize = 3;

/// Trait used as bound for [`World::iter`] and [`AllStorages::iter`].
///
/// [`World::get`]: crate::World::get
/// [`World::iter`]: crate::World::iter
pub trait IterComponent {
    #[allow(missing_docs)]
    type Storage<'a>;
    #[allow(missing_docs)]
    type Borrow<'a>;

    #[allow(missing_docs)]
    #[allow(clippy::type_complexity)]
    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    >;

    #[allow(missing_docs)]
    fn type_id() -> TypeId;

    #[allow(missing_docs)]
    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId;

    #[allow(missing_docs)]
    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage>
    where
        Self: Sized;
}

impl<T: Component + Send + Sync> IterComponent for &'_ T {
    type Storage<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = View::<'a, T>::borrow(all_storages, all_borrow, None, current)?;

        Ok(FullRawWindow::from_owned_view(view))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.len();

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> IterComponent for NonSend<&'_ T> {
    type Storage<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        Ok(FullRawWindow::from_owned_view(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.len();

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> IterComponent for NonSync<&'_ T> {
    type Storage<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        Ok(FullRawWindow::from_owned_view(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.len();

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IterComponent for NonSendSync<&'_ T> {
    type Storage<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        Ok(FullRawWindow::from_owned_view(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.len();

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

impl<T: Component + Send + Sync> IterComponent for &'_ mut T {
    type Storage<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    #[track_caller]
    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = ViewMut::<'a, T>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        Ok(FullRawWindowMut::new_owned(view))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    #[track_caller]
    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> IterComponent for NonSend<&'_ mut T> {
    type Storage<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    #[track_caller]
    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<ViewMut<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        Ok(FullRawWindowMut::new_owned(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    #[track_caller]
    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> IterComponent for NonSync<&'_ mut T> {
    type Storage<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    #[track_caller]
    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<ViewMut<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        Ok(FullRawWindowMut::new_owned(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    #[track_caller]
    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IterComponent for NonSendSync<&'_ mut T> {
    type Storage<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    #[track_caller]
    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<ViewMut<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        Ok(FullRawWindowMut::new_owned(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    #[track_caller]
    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

impl<T: Component + Send + Sync> IterComponent for Mut<'_, T> {
    type Storage<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view =
            ViewMut::<'a, T, track::Modification>::borrow(all_storages, all_borrow, None, current)?;

        Ok(FullRawWindowMut::new_owned(view))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> IterComponent for NonSend<Mut<'_, T>> {
    type Storage<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;

        Ok(FullRawWindowMut::new_owned(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> IterComponent for NonSync<Mut<'_, T>> {
    type Storage<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;

        Ok(FullRawWindowMut::new_owned(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IterComponent for NonSendSync<Mut<'_, T>> {
    type Storage<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_abtract_mut<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Storage<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;

        Ok(FullRawWindowMut::new_owned(view.0))
    }

    fn type_id() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn dense(raw_window: &Self::Storage<'_>) -> *const EntityId {
        raw_window.dense
    }

    fn into_iter<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<IterRef<'a, Self>, error::GetStorage> {
        let (raw_window, all_borrow, borrow) =
            Self::into_abtract_mut(all_storages, all_borrow, current)?;

        let len = raw_window.dense_len;

        let iter = Iter::Tight(Tight {
            current: 0,
            end: len,
            storage: raw_window,
        });

        Ok(IterRef {
            iter,
            _all_borrow: all_borrow,
            _borrow: borrow,
        })
    }
}

macro_rules! impl_iter_component {
    ($(($type: ident, $raw_window: ident, $borrow: ident, $index: tt))+) => {
        impl<$($type: IterComponent),+> IterComponent for ($($type,)+)
        where
            $(for<'a> $type::Storage<'a>: AbstractMut),+,
            $(for<'a> <$type::Storage<'a> as AbstractMut>::Index: From<usize>),+
        {
            type Storage<'a> = ($($type::Storage<'a>,)+);
            type Borrow<'a> = ($($type::Borrow<'a>,)+);

            fn into_abtract_mut<'a>(
                all_storages: &'a AllStorages,
                all_borrow: Option<SharedBorrow<'a>>,
                current: TrackingTimestamp,
            ) -> Result<
                (
                    Self::Storage<'a>,
                    Option<SharedBorrow<'a>>,
                    Self::Borrow<'a>,
                ),
                error::GetStorage,
            > {
                $(
                    let ($raw_window, _, $borrow) = $type::into_abtract_mut(all_storages, all_borrow.clone(), current)?;
                )+

                Ok((($($raw_window,)+), all_borrow, ($($borrow,)+)))
            }


            fn type_id() -> TypeId {
                TypeId::of::<()>()
            }

            fn dense(_raw_window: &Self::Storage<'_>) -> *const EntityId {
                ptr::null()
            }

            fn into_iter<'a>(
                all_storages: &'a AllStorages,
                all_borrow: Option<SharedBorrow<'a>>,
                current: TrackingTimestamp,
            ) -> Result<IterRef<'a, Self>, error::GetStorage> {
                let (raw_window, all_borrow, borrow) =
                    Self::into_abtract_mut(all_storages, all_borrow, current)?;

                    let type_ids = [$($type::type_id()),+];
                    let mut smallest = usize::MAX;
                    let mut smallest_dense = ptr::null();
                    let mut mask: u16 = 0;
                    let mut factored_len = usize::MAX;

                    $(
                        let len = raw_window.$index.len();
                        let factor = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;

                        if factor < factored_len {
                            smallest = len;
                            smallest_dense = $type::dense(&raw_window.$index);
                            mask = 1 << $index;
                            factored_len = factor;
                        }
                    )+

                    let _ = factored_len;

                    let iter = if smallest == usize::MAX {
                        Iter::Mixed(Mixed {
                            count: 0,
                            mask,
                            indices: [].iter(),
                            last_id: EntityId::dead(),
                            storage: raw_window,
                            rev_next_storage: Vec::new(),
                        })
                    } else {
                        let slice = unsafe { core::slice::from_raw_parts(smallest_dense, smallest) };

                        Iter::Mixed(Mixed {
                            count: 0,
                            mask,
                            indices: slice.into_iter(),
                            last_id: EntityId::dead(),
                            storage: raw_window,
                            rev_next_storage: Vec::new(),
                        })
                    };

                    Ok(IterRef {
                        iter,
                        _all_borrow: all_borrow,
                        _borrow: borrow,
                    })
            }
        }
    }
}

macro_rules! iter_component {
    ($(($type: ident, $raw_window: ident, $borrow: ident, $index: tt))+; ($type1: ident, $raw_window1: ident, $borrow1: ident, $index1: tt) $(($queue_type: ident, $queue_raw_window: ident, $queue_borrow: ident, $queue_index: tt))*) => {
        impl_iter_component![$(($type, $raw_window, $borrow, $index))*];
        iter_component![$(($type, $raw_window, $borrow, $index))* ($type1, $raw_window1, $borrow1, $index1); $(($queue_type, $queue_raw_window, $queue_borrow, $queue_index))*];
    };
    ($(($type: ident, $raw_window: ident, $borrow: ident, $index: tt))+;) => {
        impl_iter_component![$(($type, $raw_window, $borrow, $index))*];
    }
}

iter_component![(A, raw_window0, borrow0, 0); (B, raw_window1, borrow1, 1) (C, raw_window2, borrow2, 2) (D, raw_window3, borrow3, 3) (E, raw_window4, borrow4, 4) (F, raw_window5, borrow5, 5) (G, raw_window6, borrow6, 6) (H, raw_window7, borrow7, 7) (I, raw_window8, borrow8, 8) (J, raw_window9, borrow9, 9)];
