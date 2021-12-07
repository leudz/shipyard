use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::track;

/// Trait used as bound for [`World::delete_component`] and [`AllStorages::delete_component`].
pub trait TupleDelete {
    /// See [`World::delete_component`] and [`AllStorages::delete_component`].
    fn delete(all_storages: &mut AllStorages, entity: EntityId) -> bool;
}

impl<T: Send + Sync + Component> TupleDelete for (T,)
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Send + Sync,
{
    #[inline]
    fn delete(all_storages: &mut AllStorages, entity: EntityId) -> bool {
        all_storages
            .exclusive_storage_or_insert_mut(
                StorageId::of::<SparseSet<T, T::Tracking>>(),
                SparseSet::<T>::new,
            )
            .delete(entity)
    }
}

macro_rules! impl_delete_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Send + Sync + Component,)+> TupleDelete for ($($type,)+)
        where
            $(<$type::Tracking as track::Tracking<$type>>::DeletionData: Send + Sync),+
        {
            fn delete(all_storages: &mut AllStorages, entity: EntityId) -> bool {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type, $type::Tracking>>(), SparseSet::<$type>::new)
                        .delete(entity)
                )||+
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
