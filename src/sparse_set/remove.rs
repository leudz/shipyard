use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
#[cfg(doc)]
use crate::world::World;

/// Trait used as bound for [`World::remove`] and [`AllStorages::remove`].
pub trait TupleRemove {
    #[allow(missing_docs)]
    type Out;
    /// Trait used as bound for [`World::remove`] and [`AllStorages::remove`].
    fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out;
}

impl<T: Send + Sync + Component> TupleRemove for (T,)
where
    T::Tracking: Send + Sync,
{
    type Out = (Option<T>,);

    #[inline]
    fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out {
        let current = all_storages.get_current();

        (all_storages
            .exclusive_storage_or_insert_mut(
                StorageId::of::<SparseSet<T, T::Tracking>>(),
                SparseSet::new,
            )
            .remove(entity, current),)
    }
}

macro_rules! impl_remove_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Send + Sync + Component,)+> TupleRemove for ($($type,)+)
        where
            $($type::Tracking: Send + Sync),+
        {
            type Out = ($(Option<$type>,)+);

            fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out {
                let current = all_storages.get_current();

                ($(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type, $type::Tracking>>(), SparseSet::new)
                        .remove(entity, current),
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
