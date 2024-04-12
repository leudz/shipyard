use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::component::Component;
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::SparseSet;
use crate::track::Tracking;
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

impl<T: Send + Sync + Component> BulkInsert for T
where
    T::Tracking: Send + Sync,
{
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_> {
        let iter = iter.into_iter();
        let current = all_storages.get_current();
        let mut entities = all_storages.entities_mut().unwrap();
        let mut sparse_set = all_storages
            .custom_storage_or_insert_mut(SparseSet::<T, T::Tracking>::new)
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
        if T::Tracking::track_insertion() {
            sparse_set
                .insertion_data
                .extend(new_entities.iter().map(|_| current));
        }
        if T::Tracking::track_modification() {
            sparse_set.insertion_data.extend(
                new_entities
                    .iter()
                    .map(|_| current.wrapping_add(u64::MAX / 2)),
            );
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

impl<T: Send + Sync + Component> BulkInsert for (T,)
where
    T::Tracking: Send + Sync,
{
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_> {
        T::bulk_insert(all_storages, iter.into_iter().map(|(t,)| t))
    }
}

macro_rules! impl_bulk_insert {
    (($type1: ident, $sparse_set1: ident, $index1: tt) $(($type: ident, $sparse_set: ident, $index: tt))*) => {
        impl<$type1: Send + Sync + Component, $($type: Send + Sync + Component,)*> BulkInsert for ($type1, $($type,)*)
        where
            $type1::Tracking: Send + Sync,
            $($type::Tracking: Send + Sync),+
        {
            #[allow(non_snake_case)]
            fn bulk_insert<Source: IntoIterator<Item = Self>>(all_storages: &mut AllStorages, iter: Source) -> BulkEntityIter<'_> {
                let iter = iter.into_iter();
                let size_hint = iter.size_hint().0;
                let mut entities = all_storages.entities_mut().unwrap();
                let mut $sparse_set1 = all_storages.custom_storage_or_insert_mut(SparseSet::<$type1, $type1::Tracking>::new).unwrap();
                $(
                    let mut $sparse_set = all_storages.custom_storage_or_insert_mut(SparseSet::<$type, $type::Tracking>::new).unwrap();
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

                if $type1::Tracking::track_insertion() {
                    $sparse_set1.insertion_data.extend(new_entities.iter().map(|_| 0));
                }
                if $type1::Tracking::track_modification() {
                    $sparse_set1.modification_data.extend(new_entities.iter().map(|_| 0));
                }
                $(
                    if $type::Tracking::track_insertion() {
                        $sparse_set.insertion_data.extend(new_entities.iter().map(|_| 0));
                    }
                    if $type::Tracking::track_modification() {
                        $sparse_set.modification_data.extend(new_entities.iter().map(|_| 0));
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

bulk_insert![(A, sparse_set0, 0) (B, sparse_set1, 1); (C, sparse_set2, 2) (D, sparse_set3, 3) (E, sparse_set4, 4) (F, sparse_set5, 5) (G, sparse_set6, 6) (H, sparse_set7, 7) (I, sparse_set8, 8) (J, sparse_set9, 9)];
