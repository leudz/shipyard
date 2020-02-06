use super::FakeBorrow;
use crate::atomic_refcell::AtomicRefCell;
use crate::storage::{AllStorages, Entities, EntitiesMut};
use crate::views::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
#[cfg(feature = "non_send")]
use crate::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
use crate::NonSendSync;
#[cfg(feature = "non_sync")]
use crate::NonSync;
use crate::{error, Unique};
use alloc::vec::Vec;
use core::any::TypeId;
use core::convert::TryInto;
#[cfg(feature = "parallel")]
use rayon::ThreadPool;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutation {
    Shared,
    Unique,
}

pub trait SystemData<'a> {
    type View;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage>;

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>);

    fn is_send_sync() -> bool;
}

impl<'a> SystemData<'a> for () {
    type View = ();

    fn try_borrow(
        _: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        Ok(())
    }

    fn borrow_infos(_: &mut Vec<(TypeId, Mutation)>) {}

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> SystemData<'a> for AllStorages {
    type View = AllStoragesViewMut<'a>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        all_storages.try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<AllStorages>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> SystemData<'a> for Entities {
    type View = EntitiesView<'a>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
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

impl<'a> SystemData<'a> for EntitiesMut {
    type View = EntitiesViewMut<'a>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<'a> SystemData<'a> for crate::ThreadPool {
    type View = &'a ThreadPool;

    fn try_borrow(
        _: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        Ok(thread_pool)
    }

    fn borrow_infos(_: &mut Vec<(TypeId, Mutation)>) {}

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> SystemData<'a> for &T {
    type View = View<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
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

impl<'a, T: 'static + Send + Sync> SystemData<'a> for &mut T {
    type View = ViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
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

impl<'a, T: 'static + Send + Sync> SystemData<'a> for Unique<&T> {
    type View = UniqueView<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&T as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <&T as SystemData>::is_send_sync()
    }
}

impl<'a, T: 'static + Send + Sync> SystemData<'a> for Unique<&mut T> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        all_storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_into()
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&mut T as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <&mut T as SystemData>::is_send_sync()
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> SystemData<'a> for NonSend<&T> {
    type View = View<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        View::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> SystemData<'a> for NonSend<&mut T> {
    type View = ViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        ViewMut::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> SystemData<'a> for Unique<NonSend<&T>> {
    type View = UniqueView<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        UniqueView::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSend<&T> as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSend<&T> as SystemData>::is_send_sync()
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> SystemData<'a> for Unique<NonSend<&mut T>> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        UniqueViewMut::try_from_non_send(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSend<&mut T> as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSend<&mut T> as SystemData>::is_send_sync()
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> SystemData<'a> for NonSync<&T> {
    type View = View<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        View::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> SystemData<'a> for NonSync<&mut T> {
    type View = ViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        ViewMut::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> SystemData<'a> for Unique<NonSync<&T>> {
    type View = UniqueView<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        UniqueView::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSync<&T> as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSync<&T> as SystemData>::is_send_sync()
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> SystemData<'a> for Unique<NonSync<&mut T>> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        UniqueViewMut::try_from_non_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSync<&mut T> as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSync<&mut T> as SystemData>::is_send_sync()
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> SystemData<'a> for NonSendSync<&T> {
    type View = View<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        View::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> SystemData<'a> for NonSendSync<&mut T> {
    type View = ViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        ViewMut::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> SystemData<'a> for Unique<NonSendSync<&T>> {
    type View = UniqueView<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        UniqueView::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSendSync<&T> as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSendSync<&T> as SystemData>::is_send_sync()
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> SystemData<'a> for Unique<NonSendSync<&mut T>> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        UniqueViewMut::try_from_non_send_sync(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <NonSendSync<&mut T> as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync() -> bool {
        <NonSendSync<&mut T> as SystemData>::is_send_sync()
    }
}

impl<'a, T: 'static> SystemData<'a> for FakeBorrow<T> {
    type View = ();

    fn try_borrow(
        _: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        Ok(())
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique))
    }

    fn is_send_sync() -> bool {
        true
    }
}

macro_rules! impl_system_data {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: SystemData<'a>),+> SystemData<'a> for ($($type,)+) {
            type View = ($($type::View,)+);

            fn try_borrow(
                storages: &'a AtomicRefCell<AllStorages>,
                #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
            ) -> Result<Self::View, error::GetStorage> {
                #[cfg(feature = "parallel")]
                {
                    Ok(($(
                        <$type as SystemData>::try_borrow(storages, thread_pool)?,
                    )+))
                }
                #[cfg(not(feature = "parallel"))]
                {
                    Ok(($(
                        <$type as SystemData>::try_borrow(storages)?,
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

macro_rules! system_data {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_system_data![$(($type, $index))*];
        system_data![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_system_data![$(($type, $index))*];
    }
}

system_data![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
