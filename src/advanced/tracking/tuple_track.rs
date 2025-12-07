use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
#[cfg(doc)]
use crate::world::World;

/// Trait used as bound for `World::track_*` and `AllStorages::track_*`.
pub trait TupleTrack {
    #[allow(missing_docs)]
    fn track_insertion(all_storages: &mut AllStorages);
    #[allow(missing_docs)]
    fn track_modification(all_storages: &mut AllStorages);
    #[allow(missing_docs)]
    fn track_deletion(all_storages: &mut AllStorages);
    #[allow(missing_docs)]
    fn track_removal(all_storages: &mut AllStorages);
    #[allow(missing_docs)]
    fn track_all(all_storages: &mut AllStorages);
}

impl<T: Send + Sync + Component> TupleTrack for T {
    #[inline]
    fn track_insertion(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::<T>::new)
            .track_insertion();
    }

    #[inline]
    fn track_modification(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::<T>::new)
            .track_modification();
    }

    #[inline]
    fn track_deletion(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::<T>::new)
            .track_deletion();
    }

    #[inline]
    fn track_removal(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::<T>::new)
            .track_removal();
    }

    #[inline]
    fn track_all(all_storages: &mut AllStorages) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::<T>::new)
            .track_all();
    }
}

macro_rules! impl_track {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Send + Sync + Component,)+> TupleTrack for ($($type,)+) {
            #[inline]
            fn track_insertion(all_storages: &mut AllStorages) {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::<$type>::new)
                        .track_insertion();
                )+
            }
            #[inline]
            fn track_modification(all_storages: &mut AllStorages) {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::<$type>::new)
                        .track_modification();
                )+
            }
            #[inline]
            fn track_deletion(all_storages: &mut AllStorages) {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::<$type>::new)
                        .track_deletion();
                )+
            }
            #[inline]
            fn track_removal(all_storages: &mut AllStorages) {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::<$type>::new)
                        .track_removal();
                )+
            }
            #[inline]
            fn track_all(all_storages: &mut AllStorages) {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::<$type>::new)
                        .track_all();
                )+
            }
        }
    };
}

macro_rules! track {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_track![$(($type, $index))*];
        track![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_track![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
track![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
track![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
