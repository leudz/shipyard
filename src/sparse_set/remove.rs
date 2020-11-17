use crate::sparse_set::SparseSet;
use crate::storage::{AllStorages, EntityId, StorageId};

pub trait Remove {
    type Out;
    fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out;
}

impl<T: 'static + Send + Sync> Remove for (T,) {
    type Out = (Option<T>,);

    #[inline]
    fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out {
        (all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::new)
            .remove(entity),)
    }
}

macro_rules! impl_remove_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static + Send + Sync,)+> Remove for ($($type,)+) {
            type Out = ($(Option<$type>,)+);

            fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out {
                ($(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::new)
                        .remove(entity),
                )+)
            }
        }
    };
}

macro_rules! remove_component {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_remove_component![$(($type, $index))*];
        remove_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_remove_component![$(($type, $index))*];
    }
}

remove_component![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
