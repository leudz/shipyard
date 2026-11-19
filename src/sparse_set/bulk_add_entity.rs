use crate::all_storages::{AllStorages, component_storage_or_insert_mut_in};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::SparseSet;
use crate::tracking::TrackingTimestamp;
#[cfg(doc)]
use crate::world::World;
use alloc::vec::Vec;
use core::iter::IntoIterator;

/// Trait used as bound for [`World::bulk_add_entity`] and [`AllStorages::bulk_add_entity`].
pub trait BulkAddEntity {
    /// See [`World::bulk_add_entity`] and [`AllStorages::bulk_add_entity`].
    fn bulk_add_entity(self, all_storages: &mut AllStorages) -> BulkEntityIter<'_>;
}

impl<I: IntoIterator> BulkAddEntity for I
where
    I::Item: BulkInsert,
{
    fn bulk_add_entity(self, all_storages: &mut AllStorages) -> BulkEntityIter<'_> {
        <I::Item as BulkInsert>::bulk_insert(all_storages, self)
    }
}

pub trait BulkInsert {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_>
    where
        Self: Sized;
}

/// Rolls back appended values if the source unwinds before we publish their rows.
///
/// The guard only borrows the vector. Forget it after assigning IDs to keep the appended values.
struct PendingComponents<'a, T> {
    data: &'a mut Vec<T>,
    start: usize,
}

impl<'a, T> PendingComponents<'a, T> {
    fn new(data: &'a mut Vec<T>) -> Self {
        Self {
            start: data.len(),
            data,
        }
    }

    fn added(&self) -> usize {
        self.data.len() - self.start
    }
}

impl<T> Drop for PendingComponents<'_, T> {
    fn drop(&mut self) {
        self.data.truncate(self.start);
    }
}

/// Publishes a nonempty batch of fresh contiguous IDs for the values appended to this storage.
#[inline]
fn publish_batch<T: Component>(
    sparse_set: &mut SparseSet<T>,
    new_entities: &[EntityId],
    current: TrackingTimestamp,
) {
    let start = sparse_set.dense.len();
    let end = start + new_entities.len();
    // Appending leaves any group ranges and the start of the unevaluated tail in place.
    sparse_set.dense.extend_from_slice(new_entities);

    if sparse_set.is_tracking_insertion() {
        sparse_set.insertion_data.resize(end, current);
    }
    if sparse_set.is_tracking_modification() {
        sparse_set
            .modification_data
            .resize(end, TrackingTimestamp::origin());
    }

    let sparse = &mut sparse_set.sparse;
    sparse.bulk_allocate(new_entities[0], new_entities[new_entities.len() - 1]);
    for (offset, &entity) in new_entities.iter().enumerate() {
        // SAFE: The IDs are contiguous and `bulk_allocate` allocated every page they use.
        unsafe {
            *sparse.get_mut_unchecked(entity) = EntityId::new((start + offset) as u64);
        }
    }
}

impl BulkInsert for () {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_>
    where
        Self: Sized,
    {
        let count = iter.into_iter().count();
        let entities = all_storages.entities.get_mut();
        let new_entities = entities.bulk_generate(count);

        BulkEntityIter {
            iter: new_entities.iter().copied(),
            slice: new_entities,
        }
    }
}

impl<T: Send + Sync + Component> BulkInsert for T {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_> {
        let iter = iter.into_iter();
        let current = all_storages.get_current();
        // We borrow the fields separately, so the entities don't need a runtime borrow next to
        // the sparse set's.
        let entities = all_storages.entities.get_mut();
        let mut sparse_set =
            component_storage_or_insert_mut_in(&all_storages.storages, SparseSet::<T>::new)
                .unwrap();

        let pending = PendingComponents::new(&mut sparse_set.data);
        pending.data.extend(iter);
        let new_entities_count = pending.added();

        if new_entities_count == 0 {
            return BulkEntityIter {
                iter: [].iter().copied(),
                slice: &[],
            };
        }

        let new_entities = entities.bulk_generate(new_entities_count);
        core::mem::forget(pending);
        publish_batch(&mut sparse_set, new_entities, current);

        BulkEntityIter {
            iter: new_entities.iter().copied(),
            slice: new_entities,
        }
    }
}

impl<T: Send + Sync + Component> BulkInsert for (T,) {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_> {
        T::bulk_insert(all_storages, iter.into_iter().map(|(t,)| t))
    }
}

macro_rules! impl_bulk_insert {
    (($type1: ident, $sparse_set1: ident, $index1: tt) $(($type: ident, $sparse_set: ident, $index: tt))*) => {
        impl<$type1: Send + Sync + Component, $($type: Send + Sync + Component,)*> BulkInsert for ($type1, $($type,)*) {
            #[allow(non_snake_case)]
            fn bulk_insert<Source: IntoIterator<Item = Self>>(all_storages: &mut AllStorages, iter: Source) -> BulkEntityIter<'_> {
                let iter = iter.into_iter();
                let size_hint = iter.size_hint().0;
                let current = all_storages.get_current();
                // We borrow the fields separately, so the entities don't need a runtime borrow next
                // to the sparse sets'.
                let entities = all_storages.entities.get_mut();
                let mut $sparse_set1 = component_storage_or_insert_mut_in(&all_storages.storages, SparseSet::<$type1>::new).unwrap();
                $(
                    let mut $sparse_set = component_storage_or_insert_mut_in(&all_storages.storages, SparseSet::<$type>::new).unwrap();
                )*

                $sparse_set1.reserve(size_hint);
                $(
                    $sparse_set.reserve(size_hint);
                )*

                let pending = (
                    PendingComponents::new(&mut $sparse_set1.data),
                    $(PendingComponents::new(&mut $sparse_set.data),)*
                );
                for ($type1, $($type,)*) in iter {
                    pending.$index1.data.push($type1);
                    $(pending.$index.data.push($type);)*
                }

                let new_entities_count = pending.$index1.added();
                if new_entities_count == 0 {
                    return BulkEntityIter {
                        iter: [].iter().copied(),
                        slice: &[],
                    };
                }

                let new_entities = entities.bulk_generate(new_entities_count);
                core::mem::forget(pending);
                publish_batch(&mut $sparse_set1, new_entities, current);
                $(
                    publish_batch(&mut $sparse_set, new_entities, current);
                )*

                BulkEntityIter {
                    iter: new_entities.iter().copied(),
                    slice: new_entities,
                }
            }
        }
    };
}

macro_rules! bulk_insert {
    ($(($type: ident, $sparse_set: ident, $index: tt))*;($type1: ident, $sparse_set1: ident, $index1: tt) $(($queue_type: ident, $queue_sparse_set: ident, $queue_index: tt))*) => {
        impl_bulk_insert![$(($type, $sparse_set, $index))*];
        bulk_insert![$(($type, $sparse_set, $index))* ($type1, $sparse_set1, $index1); $(($queue_type, $queue_sparse_set, $queue_index))*];
    };
    ($(($type: ident, $sparse_set: ident, $index: tt))*;) => {
        impl_bulk_insert![$(($type, $sparse_set, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
bulk_insert![(A, sparse_set0, 0) (B, sparse_set1, 1); (C, sparse_set2, 2) (D, sparse_set3, 3) (E, sparse_set4, 4) (F, sparse_set5, 5) (G, sparse_set6, 6) (H, sparse_set7, 7) (I, sparse_set8, 8) (J, sparse_set9, 9)];
#[cfg(feature = "extended_tuple")]
bulk_insert![
    (A, sparse_set0, 0) (B, sparse_set1, 1); (C, sparse_set2, 2) (D, sparse_set3, 3) (E, sparse_set4, 4) (F, sparse_set5, 5) (G, sparse_set6, 6) (H, sparse_set7, 7) (I, sparse_set8, 8) (J, sparse_set9, 9)
    (K, sparse_set10, 10) (L, sparse_set11, 11) (M, sparse_set12, 12) (N, sparse_set13, 13) (O, sparse_set14, 14) (P, sparse_set15, 15) (Q, sparse_set16, 16) (R, sparse_set17, 17) (S, sparse_set18, 18) (T, sparse_set19, 19)
    (U, sparse_set20, 20) (V, sparse_set21, 21) (W, sparse_set22, 22) (X, sparse_set23, 23) (Y, sparse_set24, 24) (Z, sparse_set25, 25) (AA, sparse_set26, 26) (BB, sparse_set27, 27) (CC, sparse_set28, 28) (DD, sparse_set29, 29)
    (EE, sparse_set30, 30) (FF, sparse_set31, 31)
];
