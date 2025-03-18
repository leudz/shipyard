use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::tracking::TrackingTimestamp;
#[cfg(doc)]
use crate::world::World;

/// Trait used as bound for [`World::add_entity`], [`World::add_component`], [`AllStorages::add_entity`] and [`AllStorages::add_component`].
pub trait TupleAddComponent {
    /// See [`World::add_entity`], [`World::add_component`], [`AllStorages::add_entity`] and [`AllStorages::add_component`].
    fn add_component(
        self,
        all_storages: &mut AllStorages,
        entity: EntityId,
        current: TrackingTimestamp,
    );
}

impl TupleAddComponent for () {
    #[inline]
    fn add_component(self, _: &mut AllStorages, _: EntityId, _: TrackingTimestamp) {}
}

impl<T: Send + Sync + Component> TupleAddComponent for T {
    #[inline]
    #[track_caller]
    fn add_component(
        self,
        all_storages: &mut AllStorages,
        entity: EntityId,
        current: TrackingTimestamp,
    ) {
        all_storages
            .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::new)
            .insert(entity, self, current)
            .assert_inserted();
    }
}

impl<T: Send + Sync + Component> TupleAddComponent for Option<T> {
    #[inline]
    #[track_caller]
    fn add_component(
        self,
        all_storages: &mut AllStorages,
        entity: EntityId,
        current: TrackingTimestamp,
    ) {
        if let Some(component) = self {
            all_storages
                .exclusive_storage_or_insert_mut(StorageId::of::<SparseSet<T>>(), SparseSet::new)
                .insert(entity, component, current)
                .assert_inserted();
        }
    }
}

macro_rules! impl_add_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: TupleAddComponent,)+> TupleAddComponent for ($($type,)+) {
            #[track_caller]
            fn add_component(self, all_storages: &mut AllStorages, entity: EntityId, current: TrackingTimestamp) {
                $(
                    self.$index.add_component(all_storages, entity, current);
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

#[cfg(not(feature = "extended_tuple"))]
add_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
add_component![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
