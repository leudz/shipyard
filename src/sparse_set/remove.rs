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

impl<T: Send + Sync + Component> TupleRemove for T {
    type Out = Option<T>;

    #[inline]
    fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out {
        let current = all_storages.get_current();

        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::new)
            .dyn_remove(entity, current)
    }
}

macro_rules! impl_remove_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Send + Sync + Component,)+> TupleRemove for ($($type,)+) {
            type Out = ($(Option<$type>,)+);

            fn remove(all_storages: &mut AllStorages, entity: EntityId) -> Self::Out {
                let current = all_storages.get_current();

                ($(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type>>(), SparseSet::new)
                        .dyn_remove(entity, current),
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

#[cfg(not(feature = "extended_tuple"))]
remove_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
remove_component![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
