use crate::all_storages::AllStorages;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::{Component, Unique};
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::unique::UniqueStorage;

pub trait TupleClone {
    fn register_clone(all_storages: &mut AllStorages);
}

impl TupleClone for () {
    fn register_clone(_all_storages: &mut AllStorages) {}
}

impl<T: Component + Clone + Send + Sync> TupleClone for SparseSet<T> {
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::<T>::new)
            .register_clone();
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Clone + Send + Sync> TupleClone for NonSend<SparseSet<T>> {
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<NonSend<SparseSet<T>>>(), || {
                NonSend(SparseSet::<T>::new())
            })
            .register_clone();
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Clone + Send + Sync> TupleClone for NonSync<SparseSet<T>> {
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<NonSync<SparseSet<T>>>(), || {
                NonSync(SparseSet::<T>::new())
            })
            .register_clone();
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Clone + Send + Sync> TupleClone for NonSendSync<SparseSet<T>> {
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<NonSendSync<SparseSet<T>>>(), || {
                NonSendSync(SparseSet::<T>::new())
            })
            .register_clone();
    }
}

impl<T: Unique + Clone + Send + Sync> TupleClone for UniqueStorage<T> {
    #[track_caller]
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_mut::<UniqueStorage<T>>()
            .unwrap()
            .register_clone();
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique + Clone + Sync> TupleClone for NonSend<UniqueStorage<T>> {
    #[track_caller]
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_mut_by_id::<NonSend<UniqueStorage<T>>>(StorageId::of::<
                UniqueStorage<T>,
            >())
            .unwrap()
            .register_clone();
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique + Clone + Send> TupleClone for NonSync<UniqueStorage<T>> {
    #[track_caller]
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_mut_by_id::<NonSync<UniqueStorage<T>>>(StorageId::of::<
                UniqueStorage<T>,
            >())
            .unwrap()
            .register_clone();
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique + Clone> TupleClone for NonSendSync<UniqueStorage<T>> {
    #[track_caller]
    fn register_clone(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_mut_by_id::<NonSendSync<UniqueStorage<T>>>(StorageId::of::<
                UniqueStorage<T>,
            >())
            .unwrap()
            .register_clone();
    }
}

macro_rules! impl_clone {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: TupleClone),+> TupleClone for ($($storage,)+) {
            #[track_caller]
            fn register_clone(all_storages: &mut AllStorages) {
                $(
                    $storage::register_clone(all_storages);
                )+
            }
        }
    }
}

macro_rules! clone {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_clone![$(($storage, $index))*];
        clone![$(($storage, $index))* ($storage1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_clone![$(($storage, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
clone![(StorageA, 0); (StorageB, 1) (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)];
#[cfg(feature = "extended_tuple")]
clone![
    (StorageA, 0); (StorageB, 1) (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)
    (StorageK, 10) (StorageL, 11) (StorageM, 12) (StorageN, 13) (StorageO, 14) (StorageP, 15) (StorageQ, 16) (StorageR, 17) (StorageS, 18) (StorageT, 19)
    (StorageU, 20) (StorageV, 21) (StorageW, 22) (StorageX, 23) (StorageY, 24) (StorageZ, 25) (StorageAA, 26) (StorageBB, 27) (StorageCC, 28) (StorageDD, 29)
    (StorageEE, 30) (StorageFF, 31)
];
