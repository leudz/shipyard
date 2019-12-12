use crate::atomic_refcell::{AtomicRefCell, Borrow, Ref, RefMut};
use crate::not::Not;
use crate::sparse_set::{View, ViewMut};
use crate::storage::{
    AllStorages, AllStoragesViewMut, Entities, EntitiesMut, EntitiesView, EntitiesViewMut,
};
use crate::{error, Unique};
#[cfg(feature = "parallel")]
use rayon::ThreadPool;
use std::any::{type_name, TypeId};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mutation {
    Shared,
    Unique,
}

pub trait SystemData<'a> {
    type View;

    /// # Safety
    ///
    /// Borrow has to be dropped after Self::View.
    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage>;

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>);
}

impl<'a> SystemData<'a> for AllStorages {
    type View = AllStoragesViewMut<'a>;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        all_storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, borrow) = RefMut::destructure(
            all_storages
                .try_borrow_mut()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        );
        borrows.push(borrow);
        Ok(all_storages.view_mut())
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<AllStorages>(), Mutation::Unique));
    }
}

impl<'a> SystemData<'a> for Entities {
    type View = EntitiesView<'a>;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = Ref::destructure(
            storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        );
        let (entities, borrow) = Ref::destructure(
            all_storages.0[&TypeId::of::<Entities>()]
                .entities()
                .map_err(error::GetStorage::Entities)?,
        );
        borrows.push(borrow);
        borrows.push(all_borrow);
        Ok(entities.view())
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<Entities>(), Mutation::Shared));
    }
}

impl<'a> SystemData<'a> for EntitiesMut {
    type View = EntitiesViewMut<'a>;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = Ref::destructure(
            storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        );
        let (entities, borrow) = RefMut::destructure(
            all_storages.0[&TypeId::of::<Entities>()]
                .entities_mut()
                .map_err(error::GetStorage::Entities)?,
        );
        borrows.push(borrow);
        borrows.push(all_borrow);
        Ok(entities.view_mut())
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<Entities>(), Mutation::Unique));
    }
}

#[cfg(feature = "parallel")]
impl<'a> SystemData<'a> for crate::ThreadPool {
    type View = &'a ThreadPool;

    unsafe fn try_borrow(
        _: &mut Vec<Borrow<'a>>,
        _: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        Ok(thread_pool)
    }

    fn borrow_infos(_: &mut Vec<(TypeId, Mutation)>) {}
}

impl<'a, T: 'static> SystemData<'a> for &T {
    type View = View<'a, T>;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = Ref::destructure(
            storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        );
        let (array, borrow) = Ref::destructure(
            all_storages
                .0
                .get(&TypeId::of::<T>())
                .ok_or_else(|| error::GetStorage::MissingComponent(type_name::<T>()))?
                .sparse_set()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
        );
        borrows.push(borrow);
        borrows.push(all_borrow);
        Ok(array.view())
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Shared));
    }
}

impl<'a, T: 'static> SystemData<'a> for &mut T {
    type View = ViewMut<'a, T>;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let (all_storages, all_borrow) = Ref::destructure(
            storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        );
        let (array, borrow) = RefMut::destructure(
            all_storages
                .0
                .get(&TypeId::of::<T>())
                .ok_or_else(|| error::GetStorage::MissingComponent(type_name::<T>()))?
                .sparse_set_mut()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
        );
        borrows.push(borrow);
        borrows.push(all_borrow);
        Ok(array.view_mut())
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        infos.push((TypeId::of::<T>(), Mutation::Unique));
    }
}

impl<'a, T: 'static> SystemData<'a> for Not<&T> {
    type View = Not<<&'a T as SystemData<'a>>::View>;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let view = {
            #[cfg(feature = "parallel")]
            {
                <&T as SystemData>::try_borrow(borrows, storages, thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                <&T as SystemData>::try_borrow(borrows, storages)?
            }
        };
        Ok(Not(view))
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&T as SystemData>::borrow_infos(infos)
    }
}

impl<'a, T: 'static> SystemData<'a> for Not<&mut T> {
    type View = Not<<&'a mut T as SystemData<'a>>::View>;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let view = {
            #[cfg(feature = "parallel")]
            {
                <&mut T as SystemData>::try_borrow(borrows, storages, thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                <&mut T as SystemData>::try_borrow(borrows, storages)?
            }
        };
        Ok(Not(view))
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&mut T as SystemData>::borrow_infos(infos)
    }
}

impl<'a, T: 'static> SystemData<'a> for Unique<&T> {
    type View = &'a T;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let view = {
            #[cfg(feature = "parallel")]
            {
                <&T as SystemData>::try_borrow(borrows, storages, thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                <&T as SystemData>::try_borrow(borrows, storages)?
            }
        };

        if view.is_unique() {
            Ok(view.data.get_unchecked(0))
        } else {
            Err(error::GetStorage::NonUnique((
                type_name::<T>(),
                error::Borrow::Shared,
            )))
        }
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&T as SystemData>::borrow_infos(infos)
    }
}

impl<'a, T: 'static> SystemData<'a> for Unique<&mut T> {
    type View = &'a mut T;

    unsafe fn try_borrow(
        borrows: &mut Vec<Borrow<'a>>,
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
    ) -> Result<Self::View, error::GetStorage> {
        let view = {
            #[cfg(feature = "parallel")]
            {
                <&mut T as SystemData>::try_borrow(borrows, storages, thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                <&mut T as SystemData>::try_borrow(borrows, storages)?
            }
        };

        if view.is_unique() {
            Ok(view.data.get_unchecked_mut(0))
        } else {
            Err(error::GetStorage::NonUnique((
                type_name::<T>(),
                error::Borrow::Unique,
            )))
        }
    }

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
        <&mut T as SystemData>::borrow_infos(infos)
    }
}

macro_rules! impl_system_data {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: SystemData<'a>),+> SystemData<'a> for ($($type,)+) {
            type View = ($($type::View,)+);

            unsafe fn try_borrow(
                borrows: &mut Vec<Borrow<'a>>,
                storages: &'a AtomicRefCell<AllStorages>,
                #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
            ) -> Result<Self::View, error::GetStorage> {
                #[cfg(feature = "parallel")]
                {
                    Ok(($(
                        <$type as SystemData>::try_borrow(borrows, storages, thread_pool)?,
                    )+))
                }
                #[cfg(not(feature = "parallel"))]
                {
                    Ok(($(
                        <$type as SystemData>::try_borrow(borrows, storages)?,
                    )+))
                }
            }

            fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
                $(
                    $type::borrow_infos(infos);
                )+
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
