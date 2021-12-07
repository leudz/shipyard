use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::track;

/// Trait used as bound for [`World::add_entity`], [`World::add_component`], [`AllStorages::add_entity`] and [`AllStorages::add_component`].
pub trait TupleAddComponent {
    /// See [`World::add_entity`], [`World::add_component`], [`AllStorages::add_entity`] and [`AllStorages::add_component`].
    fn add_component(self, all_storages: &mut AllStorages, entity: EntityId);
}

impl TupleAddComponent for () {
    #[inline]
    fn add_component(self, _: &mut AllStorages, _: EntityId) {}
}

impl<T: Send + Sync + Component> TupleAddComponent for (T,)
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Send + Sync,
{
    #[inline]
    fn add_component(self, all_storages: &mut AllStorages, entity: EntityId) {
        all_storages
            .exclusive_storage_or_insert_mut(
                StorageId::of::<SparseSet<T, T::Tracking>>(),
                SparseSet::new,
            )
            .insert(entity, self.0);
    }
}

macro_rules! impl_add_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Send + Sync + Component,)+> TupleAddComponent for ($($type,)+)
        where
            $(<$type::Tracking as track::Tracking<$type>>::DeletionData: Send + Sync),+
        {
            fn add_component(self, all_storages: &mut AllStorages, entity: EntityId) {
                $(
                    all_storages
                        .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<$type, $type::Tracking>>(), SparseSet::new)
                        .insert(entity, self.$index);
                )+
            }
        }
    };
}

macro_rules! add_component {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component![$(($type, $index))*];
        add_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_add_component![$(($type, $index))*];
    }
}

add_component![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
