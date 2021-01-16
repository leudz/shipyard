#[cfg(feature = "non_send")]
use super::non_send::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
use super::non_send_sync::NonSendSync;
#[cfg(feature = "non_sync")]
use super::non_sync::NonSync;
use super::{FakeBorrow, Mutability};
use crate::all_storages::AllStorages;
use crate::entities::Entities;
use crate::scheduler::TypeInfo;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::unique::Unique;
use crate::view::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
use alloc::vec::Vec;
use core::any::type_name;

pub unsafe trait BorrowInfo {
    /// This information is used during workload creation to determine which systems can run in parallel.
    ///
    /// A borrow error might happen if the information is not correct.
    fn borrow_info(info: &mut Vec<TypeInfo>);
}

unsafe impl BorrowInfo for () {
    fn borrow_info(_: &mut Vec<TypeInfo>) {}
}

unsafe impl<'a> BorrowInfo for AllStoragesViewMut<'a> {
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

unsafe impl<'a> BorrowInfo for EntitiesView<'a> {
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

unsafe impl<'a> BorrowInfo for EntitiesViewMut<'a> {
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

unsafe impl<'a, T: 'static + Send + Sync> BorrowInfo for View<'a, T> {
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
unsafe impl<'a, T: 'static + Sync> BorrowInfo for NonSend<View<'a, T>> {
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
unsafe impl<'a, T: 'static + Send> BorrowInfo for NonSync<View<'a, T>> {
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
unsafe impl<'a, T: 'static> BorrowInfo for NonSendSync<View<'a, T>> {
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

unsafe impl<'a, T: 'static + Send + Sync> BorrowInfo for ViewMut<'a, T> {
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
unsafe impl<'a, T: 'static + Sync> BorrowInfo for NonSend<ViewMut<'a, T>> {
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
unsafe impl<'a, T: 'static + Send> BorrowInfo for NonSync<ViewMut<'a, T>> {
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
unsafe impl<'a, T: 'static> BorrowInfo for NonSendSync<ViewMut<'a, T>> {
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

unsafe impl<'a, T: 'static + Send + Sync> BorrowInfo for UniqueView<'a, T> {
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
unsafe impl<'a, T: 'static + Sync> BorrowInfo for NonSend<UniqueView<'a, T>> {
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
unsafe impl<'a, T: 'static + Send> BorrowInfo for NonSync<UniqueView<'a, T>> {
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
unsafe impl<'a, T: 'static> BorrowInfo for NonSendSync<UniqueView<'a, T>> {
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

unsafe impl<'a, T: 'static + Send + Sync> BorrowInfo for UniqueViewMut<'a, T> {
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
unsafe impl<'a, T: 'static + Sync> BorrowInfo for NonSend<UniqueViewMut<'a, T>> {
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
unsafe impl<'a, T: 'static + Send> BorrowInfo for NonSync<UniqueViewMut<'a, T>> {
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
unsafe impl<'a, T: 'static> BorrowInfo for NonSendSync<UniqueViewMut<'a, T>> {
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

unsafe impl<T: 'static> BorrowInfo for FakeBorrow<T> {
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

unsafe impl<'a, T: BorrowInfo> BorrowInfo for Option<T> {
    fn borrow_info(infos: &mut Vec<TypeInfo>) {
        T::borrow_info(infos);
    }
}

macro_rules! impl_borrow_info {
    ($(($type: ident, $index: tt))+) => {
        unsafe impl<'a, $($type: BorrowInfo),+> BorrowInfo for ($($type,)+) {
            fn borrow_info(infos: &mut Vec<TypeInfo>) {
                $(
                    $type::borrow_info(infos);
                )+
            }
        }
    }
}

macro_rules! borrow_info {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_borrow_info![$(($type, $index))*];
        borrow_info![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_borrow_info![$(($type, $index))*];
    }
}

borrow_info![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
