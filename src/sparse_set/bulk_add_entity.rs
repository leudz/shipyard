use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::component::Component;
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::SparseSet;
use crate::tracking::TrackingTimestamp;
#[cfg(doc)]
use crate::world::World;
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

impl BulkInsert for () {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_>
    where
        Self: Sized,
    {
        let iter = iter.into_iter();
        let len = iter.size_hint().0;

        let entities = all_storages.exclusive_storage_mut::<Entities>().unwrap();
        let entities_len = entities.data.len();

        entities.bulk_generate(len);
        for _ in iter.skip(len) {
            entities.generate();
        }
        BulkEntityIter {
            iter: entities.data[entities_len..].iter().copied(),
            slice: &entities.data[entities_len..],
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
        let mut entities = all_storages.entities_mut().unwrap();
        let mut sparse_set = all_storages
            .custom_storage_or_insert_mut(SparseSet::<T>::new)
            .unwrap();

        // add components to the storage
        sparse_set.data.extend(iter);

        // generate new EntityId for the entities created
        let entities_len = entities.data.len();
        let old_len = sparse_set.dense.len();
        let new_entities_count = sparse_set.data.len() - old_len;
        let new_entities = entities.bulk_generate(new_entities_count);

        // add new EntityId to the storage for the components we added above
        sparse_set.dense.extend_from_slice(new_entities);

        // add tracking info if needed
        if sparse_set.is_tracking_insertion() {
            sparse_set
                .insertion_data
                .extend(new_entities.iter().map(|_| current));
        }
        if sparse_set.is_tracking_modification() {
            sparse_set
                .modification_data
                .extend(new_entities.iter().map(|_| TrackingTimestamp::origin()));
        }

        let SparseSet { sparse, dense, .. } = &mut *sparse_set;

        // update sparse to reflect the new state of dense and data
        sparse.bulk_allocate(dense[old_len], dense[dense.len() - 1]);
        for (i, &entity) in dense[old_len..].iter().enumerate() {
            unsafe {
                *sparse.get_mut_unchecked(entity) = EntityId::new((old_len + i) as u64);
            }
        }

        drop((entities, sparse_set));

        let entities = all_storages.exclusive_storage_mut::<Entities>().unwrap();

        BulkEntityIter {
            iter: entities.data[entities_len..].iter().copied(),
            slice: &entities.data[entities_len..],
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
                let mut entities = all_storages.entities_mut().unwrap();
                let mut $sparse_set1 = all_storages.custom_storage_or_insert_mut(SparseSet::<$type1>::new).unwrap();
                $(
                    let mut $sparse_set = all_storages.custom_storage_or_insert_mut(SparseSet::<$type>::new).unwrap();
                )*

                $sparse_set1.reserve(size_hint);
                $(
                    $sparse_set.reserve(size_hint);
                )*

                for ($type1, $($type,)*) in iter {
                    $sparse_set1.data.push($type1);
                    $(
                        $sparse_set.data.push($type);
                    )*
                }

                let entities_len = entities.data.len();
                let new_entities_count = $sparse_set1.data.len() - $sparse_set1.dense.len();
                let new_entities = entities.bulk_generate(new_entities_count);

                $sparse_set1.dense.extend_from_slice(new_entities);
                $(
                    $sparse_set.dense.extend_from_slice(new_entities);
                )*

                if $sparse_set1.is_tracking_insertion() {
                    $sparse_set1.insertion_data.extend(new_entities.iter().map(|_| TrackingTimestamp::new(0)));
                }
                if $sparse_set1.is_tracking_modification() {
                    $sparse_set1.modification_data.extend(new_entities.iter().map(|_| TrackingTimestamp::new(0)));
                }
                $(
                    if $sparse_set.is_tracking_insertion() {
                        $sparse_set.insertion_data.extend(new_entities.iter().map(|_| TrackingTimestamp::new(0)));
                    }
                    if $sparse_set.is_tracking_modification() {
                        $sparse_set.modification_data.extend(new_entities.iter().map(|_| TrackingTimestamp::new(0)));
                    }
                )*

                let old_len = $sparse_set1.dense.len() - new_entities_count;
                let SparseSet { sparse, dense, .. } = &mut *$sparse_set1;

                sparse.bulk_allocate(dense[old_len], dense[dense.len() - 1]);
                for (i, &entity) in dense[old_len..].iter().enumerate() {
                    unsafe {
                        *sparse.get_mut_unchecked(entity) = EntityId::new((old_len + i) as u64);
                    }
                }
                $(
                    let old_len = $sparse_set.dense.len() - new_entities_count;
                    let SparseSet { sparse, dense, .. } = &mut *$sparse_set;

                    sparse.bulk_allocate(dense[old_len], dense[dense.len() - 1]);
                    for (i, &entity) in dense[old_len..].iter().enumerate() {
                        unsafe {
                            *sparse.get_mut_unchecked(entity) = EntityId::new((old_len + i) as u64);
                        }
                    }
                )*

                drop((entities, $sparse_set1, $($sparse_set),*));

                let entities = all_storages.exclusive_storage_mut::<Entities>().unwrap();

                BulkEntityIter {
                    iter: entities.data[entities_len..].iter().copied(),
                    slice: &entities.data[entities_len..],
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
