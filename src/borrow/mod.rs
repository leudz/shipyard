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

use crate::atomic_refcell::Ref;
use crate::error;
use crate::sparse_set::SparseSet;
use crate::storage::{AllStorages, Entities, StorageId, Unique};
use crate::view::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
use crate::world::{TypeInfo, World};
use alloc::vec::Vec;
use core::any::type_name;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutability {
    Shared,
    Exclusive,
}

/// Allows a type to be borrowed by [`World::borrow`], [`World::run`] and worklaods.
pub trait Borrow<'a> {
    /// This function is where the actual borrowing happens.
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage>
    where
        Self: Sized;
    /// This information is used during workload creation to determine which systems can run in parallel.
    ///
    /// A borrow error might happen if the information is not correct.
    fn borrow_info(info: &mut Vec<TypeInfo>);
}

impl<'a> Borrow<'a> for () {
    #[inline]
    fn try_borrow(_: &'a World) -> Result<Self, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn borrow_info(_: &mut Vec<TypeInfo>) {}
}

impl<'a> Borrow<'a> for AllStoragesViewMut<'a> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        world
            .all_storages
            .try_borrow_mut()
            .map(AllStoragesViewMut)
            .map_err(error::GetStorage::AllStoragesBorrow)
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<AllStorages>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<AllStorages>(),
            is_send: true,
            is_sync: true,
        });
    }
}

impl<'a> Borrow<'a> for EntitiesView<'a> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .private_get_or_insert(Entities::new)
            .map(|entities| EntitiesView {
                entities,
                all_borrow: Some(all_borrow),
            })
            .map_err(error::GetStorage::Entities)
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Entities>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<Entities>(),
            is_send: true,
            is_sync: true,
        });
    }
}

impl<'a> Borrow<'a> for EntitiesViewMut<'a> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .private_get_or_insert_mut(Entities::new)
            .map(|entities| EntitiesViewMut {
                entities,
                _all_borrow: Some(all_borrow),
            })
            .map_err(error::GetStorage::Entities)
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Entities>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<Entities>(),
            is_send: true,
            is_sync: true,
        });
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for View<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert(SparseSet::new)
            .map(|sparse_set| View {
                sparse_set,
                all_borrow: Some(all_borrow),
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: true,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<View<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert_non_send(SparseSet::new)
            .map(|sparse_set| {
                NonSend(View {
                    sparse_set,
                    all_borrow: Some(all_borrow),
                })
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: false,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<View<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert_non_sync(SparseSet::new)
            .map(|sparse_set| {
                NonSync(View {
                    sparse_set,
                    all_borrow: Some(all_borrow),
                })
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: true,
            is_sync: false,
        });
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<View<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert_non_send_sync(SparseSet::new)
            .map(|sparse_set| {
                NonSendSync(View {
                    sparse_set,
                    all_borrow: Some(all_borrow),
                })
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: false,
            is_sync: false,
        });
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for ViewMut<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert_mut(SparseSet::new)
            .map(|sparse_set| ViewMut {
                sparse_set,
                _all_borrow: Some(all_borrow),
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: true,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert_non_send_mut(SparseSet::new)
            .map(|sparse_set| {
                NonSend(ViewMut {
                    sparse_set,
                    _all_borrow: Some(all_borrow),
                })
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: false,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert_non_sync_mut(SparseSet::new)
            .map(|sparse_set| {
                NonSync(ViewMut {
                    sparse_set,
                    _all_borrow: Some(all_borrow),
                })
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: true,
            is_sync: false,
        });
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages
            .get_or_insert_non_send_sync_mut(SparseSet::new)
            .map(|sparse_set| {
                NonSendSync(ViewMut {
                    sparse_set,
                    _all_borrow: Some(all_borrow),
                })
            })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<SparseSet<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            is_send: false,
            is_sync: false,
        });
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueView<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get().map(|unique| UniqueView {
            unique,
            all_borrow: Some(all_borrow),
        })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: true,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get().map(|unique| {
            NonSend(UniqueView {
                unique,
                all_borrow: Some(all_borrow),
            })
        })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: false,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get().map(|unique| {
            NonSync(UniqueView {
                unique,
                all_borrow: Some(all_borrow),
            })
        })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: true,
            is_sync: false,
        });
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get().map(|unique| {
            NonSendSync(UniqueView {
                unique,
                all_borrow: Some(all_borrow),
            })
        })
    }
    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: false,
            is_sync: false,
        });
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueViewMut<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get_mut().map(|unique| UniqueViewMut {
            unique,
            _all_borrow: Some(all_borrow),
        })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: true,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get_mut().map(|unique| {
            NonSend(UniqueViewMut {
                unique,
                _all_borrow: Some(all_borrow),
            })
        })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: false,
            is_sync: true,
        });
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get_mut().map(|unique| {
            NonSync(UniqueViewMut {
                unique,
                _all_borrow: Some(all_borrow),
            })
        })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: true,
            is_sync: false,
        });
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .try_borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        all_storages.get_mut().map(|unique| {
            NonSendSync(UniqueViewMut {
                unique,
                _all_borrow: Some(all_borrow),
            })
        })
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Unique<T>>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<Unique<T>>(),
            is_send: false,
            is_sync: false,
        });
    }
}

impl<T: 'static> Borrow<'_> for FakeBorrow<T> {
    #[inline]
    fn try_borrow(_: &World) -> Result<Self, error::GetStorage> {
        Ok(FakeBorrow::new())
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<T>(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<T>(),
            is_send: true,
            is_sync: true,
        });
    }
}

impl<'a, T: Borrow<'a>> Borrow<'a> for Option<T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        Ok(T::try_borrow(world).ok())
    }

    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        T::borrow_info(infos);
    }
}

macro_rules! impl_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: Borrow<'a>),+> Borrow<'a> for ($($type,)+) {
            #[inline]
            fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
                Ok(($(
                    <$type as Borrow>::try_borrow(world)?,
                )+))
            }

            fn borrow_info(infos: &mut Vec<TypeInfo>) {
                $(
                    $type::borrow_info(infos);
                )+
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
