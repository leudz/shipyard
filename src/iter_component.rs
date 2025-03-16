mod iter_ref;

pub use iter_ref::IntoIterRef;

use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::borrow::Borrow;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Component;
use crate::iter::{Mixed, ShiperatorCaptain};
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, RawEntityIdAccess};
use crate::storage::StorageId;
use crate::tracking::Tracking;
use crate::tracking::TrackingTimestamp;
use crate::views::{View, ViewMut};
use crate::{error, track, ShipHashSet};
use core::any::type_name;

/// Trait used as bound for [`World::iter`] and [`AllStorages::iter`].
///
/// [`World::get`]: crate::World::get
/// [`World::iter`]: crate::World::iter
pub trait IterComponent {
    #[allow(missing_docs)]
    type Shiperator<'a>;
    #[allow(missing_docs)]
    type Borrow<'a>;

    #[allow(missing_docs)]
    #[allow(clippy::type_complexity)]
    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    >;
}

pub(crate) fn into_iter<'a, T: IterComponent>(
    all_storages: &'a AllStorages,
    all_borrow: Option<SharedBorrow<'a>>,
    current: TrackingTimestamp,
) -> Result<IntoIterRef<'a, T>, error::GetStorage>
where
    <T as IterComponent>::Shiperator<'a>: ShiperatorCaptain,
{
    let mut storage_ids = ShipHashSet::new();
    let (shiperator, all_borrow, borrow, end, entities) =
        T::into_shiperator(all_storages, all_borrow, current, &mut storage_ids)?;

    let is_exact_sized = shiperator.is_exact_sized();

    Ok(IntoIterRef {
        shiperator,
        _all_borrow: all_borrow,
        _borrow: borrow,
        entities,
        is_exact_sized,
        end,
        phantom: core::marker::PhantomData,
    })
}

impl<T: Component + Send + Sync> IterComponent for &'_ T {
    type Shiperator<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = View::<'a, T>::borrow(all_storages, all_borrow, None, current)?;
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> IterComponent for NonSend<&'_ T> {
    type Shiperator<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> IterComponent for NonSync<&'_ T> {
    type Shiperator<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IterComponent for NonSendSync<&'_ T> {
    type Shiperator<'a> = FullRawWindow<'a, T>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

impl<T: Component + Send + Sync> IterComponent for &'_ mut T {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    #[track_caller]
    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
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

        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> IterComponent for NonSend<&'_ mut T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
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

        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> IterComponent for NonSync<&'_ mut T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
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

        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IterComponent for NonSendSync<&'_ mut T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<ViewMut<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

impl<T: Component + Send + Sync> IterComponent for Mut<'_, T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view =
            ViewMut::<'a, T, track::Modification>::borrow(all_storages, all_borrow, None, current)?;
        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> IterComponent for NonSend<Mut<'_, T>> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;
        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> IterComponent for NonSync<Mut<'_, T>> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;
        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IterComponent for NonSendSync<Mut<'_, T>> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
        _storage_ids: &mut ShipHashSet<StorageId>,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            usize,
            RawEntityIdAccess,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;
        let (window, all_borrow, borrow) = FullRawWindowMut::new_owned(view.0);

        let len = window.len();
        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, len, entities))
    }
}

macro_rules! impl_iter_component {
    ($(($type: ident, $raw_window: ident, $borrow: ident, $len: ident, $entity_iter: ident, $index: tt))+) => {
        impl<$($type: IterComponent),+> IterComponent for ($($type,)+) where $(for<'a> $type::Shiperator<'a>: ShiperatorCaptain,)+ {
            type Shiperator<'a> = Mixed<($($type::Shiperator<'a>,)+)>;
            type Borrow<'a> = ($($type::Borrow<'a>,)+);

            fn into_shiperator<'a>(
                all_storages: &'a AllStorages,
                all_borrow: Option<SharedBorrow<'a>>,
                current: TrackingTimestamp,
                storage_ids: &mut ShipHashSet<StorageId>,
            ) -> Result<
                (
                    Self::Shiperator<'a>,
                    Option<SharedBorrow<'a>>,
                    Self::Borrow<'a>,
                    usize,
                    RawEntityIdAccess,
                ),
                error::GetStorage,
            > {
                $(
                    let (mut $raw_window, _, $borrow, $len, $entity_iter) = $type::into_shiperator(all_storages, all_borrow.clone(), current, storage_ids)?;
                )+

                let mut mask = 0;
                let mut len = 0;
                let mut entity_iter = RawEntityIdAccess::dangling();
                let mut min_sail_time = usize::MAX;

                $(
                    let sail_time = $raw_window.sail_time();
                    if sail_time < min_sail_time {
                        mask = 1 << $index;
                        len = $len;
                        entity_iter = $entity_iter;
                        min_sail_time = sail_time;
                    }
                )+

                let _ = min_sail_time;

                $(
                    if mask & (1 << $index) == 0 {
                        $raw_window.unpick();
                    } else {
                        if !$raw_window.is_exact_sized() {
                            mask = 0;
                        }
                    }
                )+

                Ok((Mixed {shiperator: ($($raw_window,)+), mask }, all_borrow, ($($borrow,)+), len, entity_iter))
            }
        }
    }
}

macro_rules! iter_component {
    ($(($type: ident, $raw_window: ident, $borrow: ident, $len: ident, $entity_iter: ident, $index: tt))+; ($type1: ident, $raw_window1: ident, $borrow1: ident, $len1: ident, $entity_iter1: ident, $index1: tt) $(($queue_type: ident, $queue_raw_window: ident, $queue_borrow: ident, $queue_len: ident, $queue_entity_iter: ident, $queue_index: tt))*) => {
        impl_iter_component![$(($type, $raw_window, $borrow, $len, $entity_iter, $index))*];
        iter_component![$(($type, $raw_window, $borrow, $len, $entity_iter, $index))* ($type1, $raw_window1, $borrow1, $len1, $entity_iter1, $index1); $(($queue_type, $queue_raw_window, $queue_borrow, $queue_len, $queue_entity_iter, $queue_index))*];
    };
    ($(($type: ident, $raw_window: ident, $borrow: ident, $len: ident, $entity_iter: ident, $index: tt))+;) => {
        impl_iter_component![$(($type, $raw_window, $borrow, $len, $entity_iter, $index))*];
    }
}

iter_component![(A, raw_window0, borrow0, len0, entity_iter0, 0); (B, raw_window1, borrow1, len1, entity_iter1, 1) (C, raw_window2, borrow2, len2, entity_iter2, 2) (D, raw_window3, borrow3, len3, entity_iter3, 3) (E, raw_window4, borrow4, len4, entity_iter4, 4) (F, raw_window5, borrow5, len5, entity_iter5, 5) (G, raw_window6, borrow6, len6, entity_iter6, 6) (H, raw_window7, borrow7, len7, entity_iter7, 7) (I, raw_window8, borrow8, len8, entity_iter8, 8) (J, raw_window9, borrow9, len9, entity_iter9, 9)];
