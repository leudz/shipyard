use crate::atomic_refcell::AtomicRefCell;
use crate::not::Not;
use crate::storage::{AllStorages, Entities, EntitiesMut};
use crate::views::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
use crate::{error, Unique};
use core::convert::TryInto;
#[cfg(feature = "parallel")]
use rayon::ThreadPool;
use std::any::{type_name, TypeId};

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

    fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload>;
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

    fn is_send_sync(_: &AllStorages) -> Result<bool, error::AddWorkload> {
        Ok(true)
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

    fn is_send_sync(_: &AllStorages) -> Result<bool, error::AddWorkload> {
        Ok(true)
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

    fn is_send_sync(_: &AllStorages) -> Result<bool, error::AddWorkload> {
        Ok(true)
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

    fn is_send_sync(_: &AllStorages) -> Result<bool, error::AddWorkload> {
        Ok(true)
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

    fn is_send_sync(_: &AllStorages) -> Result<bool, error::AddWorkload> {
        Ok(true)
    }
}

impl<'a, T: 'static> SystemData<'a> for &T {
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

    fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload> {
        Ok(all_storages
            .0
            .get(&TypeId::of::<T>())
            .ok_or_else(|| error::AddWorkload::MissingComponent(type_name::<T>()))?
            .is_send_sync())
    }
}

impl<'a, T: 'static> SystemData<'a> for &mut T {
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

    fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload> {
        Ok(all_storages
            .0
            .get(&TypeId::of::<T>())
            .ok_or_else(|| error::AddWorkload::MissingComponent(type_name::<T>()))?
            .is_send_sync())
    }
}

impl<'a, T: 'static> SystemData<'a> for Not<&T> {
    type View = Not<<&'a T as SystemData<'a>>::View>;

    fn try_borrow(
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let view = {
            #[cfg(feature = "parallel")]
            {
                <&T as SystemData>::try_borrow(storages, thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                <&T as SystemData>::try_borrow(storages)?
            }
        };
        Ok(Not(view))
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&T as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload> {
        <&T as SystemData>::is_send_sync(all_storages)
    }
}

impl<'a, T: 'static> SystemData<'a> for Not<&mut T> {
    type View = Not<<&'a mut T as SystemData<'a>>::View>;

    fn try_borrow(
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let view = {
            #[cfg(feature = "parallel")]
            {
                <&mut T as SystemData>::try_borrow(storages, thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                <&mut T as SystemData>::try_borrow(storages)?
            }
        };
        Ok(Not(view))
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&mut T as SystemData>::borrow_infos(infos)
    }

    fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload> {
        <&mut T as SystemData>::is_send_sync(all_storages)
    }
}

impl<'a, T: 'static> SystemData<'a> for Unique<&T> {
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

    fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload> {
        <&T as SystemData>::is_send_sync(all_storages)
    }
}

impl<'a, T: 'static> SystemData<'a> for Unique<&mut T> {
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

    fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload> {
        <&mut T as SystemData>::is_send_sync(all_storages)
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

            fn is_send_sync(all_storages: &AllStorages) -> Result<bool, error::AddWorkload> {
                Ok($($type::is_send_sync(all_storages)?)&&+)
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
