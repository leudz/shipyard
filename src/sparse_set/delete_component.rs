use crate::sparse_set::SparseSet;
use crate::storage::{AllStorages, EntityId, StorageId};

pub trait DeleteComponent {
    fn delete_component(all_storages: &mut AllStorages, entity: EntityId);
}

impl<T: 'static + Send + Sync> DeleteComponent for (T,) {
    #[inline]
    fn delete_component(all_storages: &mut AllStorages, entity: EntityId) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::<T>::new)
            .delete(entity);
    }
}

macro_rules! impl_delete_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static + Send + Sync,)+> DeleteComponent for ($($type,)+) {
            fn delete_component(all_storages: &mut AllStorages, entity: EntityId) {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::<$type>::new)
                        .delete(entity);
                )+
            }
        }
    };
}

macro_rules! delete_component {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_delete_component![$(($type, $index))*];
        delete_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_delete_component![$(($type, $index))*];
    }
}

delete_component![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
