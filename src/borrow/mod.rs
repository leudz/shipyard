mod all_storages;
mod fake_borrow;
#[cfg(feature = "non_send")]
mod non_send;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
mod non_send_sync;
#[cfg(feature = "non_sync")]
mod non_sync;

pub use all_storages::AllStoragesBorrow;
pub use fake_borrow::FakeBorrow;
#[cfg(feature = "non_send")]
pub use non_send::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
pub use non_send_sync::NonSendSync;
#[cfg(feature = "non_sync")]
pub use non_sync::NonSync;

use crate::atomic_refcell::AtomicRefCell;
use crate::error;
use crate::storage::{AllStorages, Entities};
#[cfg(feature = "parallel")]
use crate::view::ThreadPoolView;
use crate::view::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
use alloc::vec::Vec;
use core::any::TypeId;
use core::convert::TryInto;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutation {
    Shared,
    Unique,
}

pub trait Borrow<'a> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage>
    where
        Self: Sized;

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>);

    fn is_send_sync() -> bool;
}

impl<'a> Borrow<'a> for () {
    fn try_borrow(
        _: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn borrow_infos(_: &mut Vec<(TypeId, Mutation)>) {}

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> Borrow<'a> for AllStoragesViewMut<'a> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        all_storages.try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<AllStorages>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> Borrow<'a> for EntitiesView<'a> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<Entities>(), Mutation::Shared));
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> Borrow<'a> for EntitiesViewMut<'a> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<Entities>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        true
    }
}

#[cfg(feature = "parallel")]
impl<'a> Borrow<'a> for ThreadPoolView<'a> {
    fn try_borrow(
        _: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        Ok(ThreadPoolView(thread_pool))
    }

    fn borrow_infos(_: &mut Vec<(TypeId, Mutation)>) {}

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for View<'a, T> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Shared));
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for ViewMut<'a, T> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueView<'a, T> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <View<'a, T> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <View<'a, T> as Borrow>::is_send_sync()
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueViewMut<'a, T> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <ViewMut<'a, T> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <ViewMut<'a, T> as Borrow>::is_send_sync()
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<View<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        View::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<ViewMut<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        ViewMut::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueView<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        UniqueView::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSend<View<'a, T>> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueViewMut<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        UniqueViewMut::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSend<ViewMut<'a, T>> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<View<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        View::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<ViewMut<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        ViewMut::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueView<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        UniqueView::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSync<View<'a, T>> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSync<View<'a, T>> as Borrow>::is_send_sync()
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueViewMut<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        UniqueViewMut::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSync<ViewMut<'a, T>> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSync<ViewMut<'a, T>> as Borrow>::is_send_sync()
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<View<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        View::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<ViewMut<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        ViewMut::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueView<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        UniqueView::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }
    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSendSync<View<'a, T>> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSendSync<View<'a, T>> as Borrow>::is_send_sync()
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueViewMut<'a, T>> {
    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        UniqueViewMut::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSendSync<ViewMut<'a, T>> as Borrow>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSendSync<ViewMut<'a, T>> as Borrow>::is_send_sync()
    }
}

impl<'a, T: 'static> Borrow<'a> for FakeBorrow<T> {
    fn try_borrow(
        _: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a rayon::ThreadPool,
    ) -> Result<Self, error::GetStorage> {
        Ok(FakeBorrow::new())
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique))
    }

    fn is_send_sync() -> bool {
        true
    }
}

macro_rules! impl_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: Borrow<'a>),+> Borrow<'a> for ($($type,)+) {
            fn try_borrow(
                all_storages: &'a AtomicRefCell<AllStorages>,
                #[cfg(feature = "parallel")] thread_pool: &'a rayon::ThreadPool,
            ) -> Result<Self, error::GetStorage> {
                #[cfg(feature = "parallel")]
                {
                    Ok(($(
                        <$type as Borrow>::try_borrow(all_storages, thread_pool)?,
                    )+))
                }
                #[cfg(not(feature = "parallel"))]
                {
                    Ok(($(
                        <$type as Borrow>::try_borrow(all_storages)?,
                    )+))
                }
            }

            fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
                $(
                    $type::borrow_infos(infos);
                )+
            }

            fn is_send_sync() -> bool {
                $($type::is_send_sync())&&+
            }
        }
    }
}

macro_rules! borrow {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_borrow![$(($type, $index))*];
        borrow![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_borrow![$(($type, $index))*];
    }
}

borrow![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
