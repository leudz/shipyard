mod all_storages;
mod borrow_info;
#[cfg(feature = "thread_local")]
mod non_send;
#[cfg(feature = "thread_local")]
mod non_send_sync;
#[cfg(feature = "thread_local")]
mod non_sync;

pub use all_storages::AllStoragesBorrow;
pub use borrow_info::BorrowInfo;
#[cfg(feature = "thread_local")]
pub use non_send::NonSend;
#[cfg(feature = "thread_local")]
pub use non_send_sync::NonSendSync;
#[cfg(feature = "thread_local")]
pub use non_sync::NonSync;

use crate::all_storages::CustomStorageAccess;
use crate::atomic_refcell::{Ref, RefMut};
use crate::component::Component;
use crate::error;
use crate::sparse_set::SparseSet;
use crate::view::{
    AllStoragesView, AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut,
    View, ViewMut,
};
use crate::world::World;
use core::marker::PhantomData;

/// Describes if a storage is borrowed exlusively or not.  
/// It is used to display workloads' borrowing information.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutability {
    #[allow(missing_docs)]
    Shared,
    #[allow(missing_docs)]
    Exclusive,
}

/// Transforms a view into a helper type. This allows workloads to have the current syntax.
pub trait IntoBorrow {
    /// Helper type almost allowing GAT on stable.
    type Borrow: for<'a> Borrow<'a>;
}

/// Allows a type to be borrowed by [`World::borrow`], [`World::run`] and worklaods.
pub trait Borrow<'a> {
    #[allow(missing_docs)]
    type View;

    /// This function is where the actual borrowing happens.
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage>;
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct AllStoragesBorrower;

impl IntoBorrow for AllStoragesView<'_> {
    type Borrow = AllStoragesBorrower;
}

impl<'a> Borrow<'a> for AllStoragesBorrower {
    type View = AllStoragesView<'a>;

    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        world
            .all_storages
            .borrow()
            .map(AllStoragesView)
            .map_err(error::GetStorage::AllStoragesBorrow)
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct AllStoragesMutBorrower;

impl IntoBorrow for AllStoragesViewMut<'_> {
    type Borrow = AllStoragesMutBorrower;
}

impl<'a> Borrow<'a> for AllStoragesMutBorrower {
    type View = AllStoragesViewMut<'a>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        world
            .all_storages
            .borrow_mut()
            .map(AllStoragesViewMut)
            .map_err(error::GetStorage::AllStoragesBorrow)
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct UnitBorrower;

impl IntoBorrow for () {
    type Borrow = UnitBorrower;
}

impl<'a> Borrow<'a> for UnitBorrower {
    type View = ();

    #[inline]
    fn borrow(
        _: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct EntitiesBorrower;

impl IntoBorrow for EntitiesView<'_> {
    type Borrow = EntitiesBorrower;
}

impl<'a> Borrow<'a> for EntitiesBorrower {
    type View = EntitiesView<'a>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let entities = all_storages.entities()?;

        let (entities, borrow) = unsafe { Ref::destructure(entities) };

        Ok(EntitiesView {
            entities,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        })
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct EntitiesMutBorrower;

impl IntoBorrow for EntitiesViewMut<'_> {
    type Borrow = EntitiesMutBorrower;
}

impl<'a> Borrow<'a> for EntitiesMutBorrower {
    type View = EntitiesViewMut<'a>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let entities = all_storages.entities_mut()?;

        let (entities, borrow) = unsafe { RefMut::destructure(entities) };

        Ok(EntitiesViewMut {
            entities,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        })
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct ViewBorrower<T>(T);

impl<T: Send + Sync + Component> IntoBorrow for View<'_, T>
where
    T::Tracking: Send + Sync,
{
    type Borrow = ViewBorrower<T>;
}

impl<'a, T: Send + Sync + Component> Borrow<'a> for ViewBorrower<T>
where
    T::Tracking: Send + Sync,
{
    type View = View<'a, T>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run
                .unwrap_or_else(|| current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component> IntoBorrow for NonSend<View<'_, T>>
where
    T::Tracking: Sync,
{
    type Borrow = NonSend<ViewBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> Borrow<'a> for NonSend<ViewBorrower<T>>
where
    T::Tracking: Sync,
{
    type View = NonSend<View<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component> IntoBorrow for NonSync<View<'_, T>>
where
    T::Tracking: Send,
{
    type Borrow = NonSync<ViewBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> Borrow<'a> for NonSync<ViewBorrower<T>>
where
    T::Tracking: Send,
{
    type View = NonSync<View<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IntoBorrow for NonSendSync<View<'_, T>> {
    type Borrow = NonSendSync<ViewBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> Borrow<'a> for NonSendSync<ViewBorrower<T>> {
    type View = NonSendSync<View<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        }))
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct ViewMutBorrower<T>(T);

impl<T: Send + Sync + Component> IntoBorrow for ViewMut<'_, T>
where
    T::Tracking: Send + Sync,
{
    type Borrow = ViewMutBorrower<T>;
}

impl<'a, T: Send + Sync + Component> Borrow<'a> for ViewMutBorrower<T>
where
    T::Tracking: Send + Sync,
{
    type View = ViewMut<'a, T>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run
                .unwrap_or_else(|| current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component> IntoBorrow for NonSend<ViewMut<'_, T>>
where
    T::Tracking: Sync,
{
    type Borrow = NonSend<ViewMutBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> Borrow<'a> for NonSend<ViewMutBorrower<T>>
where
    T::Tracking: Sync,
{
    type View = NonSend<ViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component> IntoBorrow for NonSync<ViewMut<'_, T>>
where
    T::Tracking: Send,
{
    type Borrow = NonSync<ViewMutBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> Borrow<'a> for NonSync<ViewMutBorrower<T>>
where
    T::Tracking: Send,
{
    type View = NonSync<ViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IntoBorrow for NonSendSync<ViewMut<'_, T>> {
    type Borrow = NonSendSync<ViewMutBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> Borrow<'a> for NonSendSync<ViewMutBorrower<T>> {
    type View = NonSendSync<ViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct UniqueViewBorrower<T>(T);

impl<T: Send + Sync + Component> IntoBorrow for UniqueView<'_, T> {
    type Borrow = UniqueViewBorrower<T>;
}

impl<'a, T: Send + Sync + Component> Borrow<'a> for UniqueViewBorrower<T> {
    type View = UniqueView<'a, T>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component> IntoBorrow for NonSend<UniqueView<'_, T>> {
    type Borrow = NonSend<UniqueViewBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> Borrow<'a> for NonSend<UniqueViewBorrower<T>> {
    type View = NonSend<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component> IntoBorrow for NonSync<UniqueView<'_, T>> {
    type Borrow = NonSync<UniqueViewBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> Borrow<'a> for NonSync<UniqueViewBorrower<T>> {
    type View = NonSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IntoBorrow for NonSendSync<UniqueView<'_, T>> {
    type Borrow = NonSendSync<UniqueViewBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> Borrow<'a> for NonSendSync<UniqueViewBorrower<T>> {
    type View = NonSendSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        }))
    }
}

/// Helper struct allowing GAT-like behavior in stable.
pub struct UniqueViewMutBorrower<T>(T);

impl<T: Send + Sync + Component> IntoBorrow for UniqueViewMut<'_, T> {
    type Borrow = UniqueViewMutBorrower<T>;
}

impl<'a, T: Send + Sync + Component> Borrow<'a> for UniqueViewMutBorrower<T> {
    type View = UniqueViewMut<'a, T>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component> IntoBorrow for NonSend<UniqueViewMut<'_, T>> {
    type Borrow = NonSend<UniqueViewMutBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> Borrow<'a> for NonSend<UniqueViewMutBorrower<T>> {
    type View = NonSend<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component> IntoBorrow for NonSync<UniqueViewMut<'_, T>> {
    type Borrow = NonSync<UniqueViewMutBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> Borrow<'a> for NonSync<UniqueViewMutBorrower<T>> {
    type View = NonSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> IntoBorrow for NonSendSync<UniqueViewMut<'_, T>> {
    type Borrow = NonSendSync<UniqueViewMutBorrower<T>>;
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> Borrow<'a> for NonSendSync<UniqueViewMutBorrower<T>> {
    type View = NonSendSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &'a World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
            _phantom: PhantomData,
        }))
    }
}

impl<T: IntoBorrow> IntoBorrow for Option<T> {
    type Borrow = Option<T::Borrow>;
}

impl<'a, T: Borrow<'a>> Borrow<'a> for Option<T> {
    type View = Option<T::View>;

    #[inline]
    fn borrow(
        world: &'a World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View, error::GetStorage> {
        Ok(T::borrow(world, last_run, current).ok())
    }
}

macro_rules! impl_world_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: IntoBorrow),+> IntoBorrow for ($($type,)+) {
            type Borrow = ($($type::Borrow,)+);
        }

        impl<'a, $($type: Borrow<'a>),+> Borrow<'a> for ($($type,)+) {
            type View = ($($type::View,)+);

            #[inline]
            fn borrow(world: &'a World, last_run: Option<u32>, current: u32) -> Result<Self::View, error::GetStorage> {
                Ok(($($type::borrow(world, last_run, current)?,)+))
            }
        }
    }
}

macro_rules! world_borrow {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_world_borrow![$(($type, $index))*];
        world_borrow![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_world_borrow![$(($type, $index))*];
    }
}

world_borrow![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
