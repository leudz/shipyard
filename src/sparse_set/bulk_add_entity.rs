use crate::all_storages::AllStorages;
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::SparseSet;
use core::iter::IntoIterator;

pub trait BulkAddEntity {
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
        BulkEntityIter(entities.data[entities_len..].iter().copied())
    }
}

impl<T: 'static + Send + Sync> BulkInsert for (T,) {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntityIter<'_> {
        let iter = iter.into_iter();
        let len = iter.size_hint().0;

        let mut entities = all_storages.entities_mut().unwrap();
        let entities_len = entities.data.len();
        let new_entities = entities.bulk_generate(len);

        let mut sparse_set = all_storages
            .custom_storage_or_insert_mut(SparseSet::<T>::new)
            .unwrap();

        sparse_set.reserve(len);
        sparse_set.data.extend(iter.map(|(component,)| component));

        let old_len = sparse_set.dense.len();
        if sparse_set.metadata.track_insertion {
            sparse_set
                .dense
                .extend(new_entities.iter().copied().map(|mut id| {
                    id.set_inserted();
                    id
                }));
        } else {
            sparse_set.dense.extend_from_slice(new_entities);
        }

        let dense_len = sparse_set.dense.len();
        let data_len = sparse_set.data.len();

        if sparse_set.metadata.track_insertion {
            sparse_set.dense.extend((0..data_len - dense_len).map(|_| {
                let mut id = entities.generate();
                id.set_inserted();
                id
            }));
        } else {
            sparse_set
                .dense
                .extend((0..data_len - dense_len).map(|_| entities.generate()));
        }

        let SparseSet { sparse, dense, .. } = &mut *sparse_set;

        sparse.bulk_allocate(dense[old_len], dense[data_len - 1]);
        for (i, &entity) in dense[old_len..].iter().enumerate() {
            unsafe {
                *sparse.get_mut_unchecked(entity) =
                    EntityId::new_from_parts((old_len + i) as u64, 0, 0);
            }
        }

        drop((entities, sparse_set));

        BulkEntityIter(
            all_storages
                .exclusive_storage_mut::<Entities>()
                .unwrap()
                .data[entities_len..]
                .iter()
                .copied(),
        )
    }
}

macro_rules! impl_bulk_insert {
    (($type1: ident, $sparse_set1: ident, $index1: tt) $(($type: ident, $sparse_set: ident, $index: tt))*) => {
        impl<$type1: 'static + Send + Sync, $($type: 'static + Send + Sync,)*> BulkInsert for ($type1, $($type,)*) {
            #[allow(non_snake_case)]
            fn bulk_insert<Source: IntoIterator<Item = Self>>(all_storages: &mut AllStorages, iter: Source) -> BulkEntityIter<'_> {
                let iter = iter.into_iter();
                let size_hint = iter.size_hint().0;

                let mut entities = all_storages.entities_mut().unwrap();
                let entities_len = entities.data.len();
                let new_entities = entities.bulk_generate(size_hint);

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

                let len = $sparse_set1.data.len() - $sparse_set1.dense.len();

                let old_len1 = $sparse_set1.dense.len();
                $sparse_set1.dense.extend_from_slice(new_entities);

                let dense_len = $sparse_set1.dense.len();
                let data_len = $sparse_set1.data.len();
                $sparse_set1.dense.extend((0..data_len - dense_len).map(|_| entities.generate()));

                $(
                    $sparse_set.dense.extend_from_slice(&$sparse_set1.dense[old_len1..]);
                )*

                let start_entity = $sparse_set1.dense[old_len1];
                let end_entity = *$sparse_set1.dense.last().unwrap();

                $sparse_set1.sparse.bulk_allocate(start_entity, end_entity);
                $(
                    $sparse_set.sparse.bulk_allocate(start_entity, end_entity);
                )*

                let SparseSet {
                    sparse: sparse1,
                    dense: dense1,
                    metadata: metadata1,
                    ..
                } = &mut *$sparse_set1;

                if metadata1.track_insertion {
                    for (i, &entity) in dense1[old_len1..].iter().enumerate() {
                        unsafe {
                            let mut e = EntityId::new((old_len1 + i) as u64);
                            e.set_inserted();
                            *sparse1.get_mut_unchecked(entity) = e;
                        }
                    }
                } else {
                    for (i, &entity) in dense1[old_len1..].iter().enumerate() {
                        unsafe {
                            *sparse1.get_mut_unchecked(entity) = EntityId::new((old_len1 + i) as u64);
                        }
                    }
                }

                $(
                    let old_len = $sparse_set.dense.len() - len;

                    if $sparse_set.metadata.track_insertion {
                        for (i, &entity) in dense1[old_len1..].iter().enumerate() {
                            unsafe {
                                let mut e = EntityId::new((old_len + i) as u64);
                                e.set_inserted();
                                *$sparse_set.sparse.get_mut_unchecked(entity) = e;
                            }
                        }
                    } else {
                        for (i, &entity) in dense1[old_len1..].iter().enumerate() {
                            unsafe {
                                *$sparse_set.sparse.get_mut_unchecked(entity) = EntityId::new((old_len + i) as u64);
                            }
                        }
                    }
                )*

                drop((entities, $sparse_set1, $($sparse_set),*));

                BulkEntityIter(all_storages.exclusive_storage_mut::<Entities>().unwrap().data[entities_len..].iter().copied())
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
