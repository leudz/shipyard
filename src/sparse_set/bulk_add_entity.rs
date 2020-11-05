use crate::reserve::BulkEntitiesIter;
use crate::sparse_set::SparseSet;
use crate::storage::{AllStorages, Entities, EntityId};
use core::iter::IntoIterator;

pub trait BulkAddEntity {
    fn bulk_add_entity(self, all_storages: &mut AllStorages) -> BulkEntitiesIter<'_>;
}

impl<I: IntoIterator> BulkAddEntity for I
where
    I::Item: BulkInsert,
{
    fn bulk_add_entity(self, all_storages: &mut AllStorages) -> BulkEntitiesIter<'_> {
        <I::Item as BulkInsert>::bulk_insert(all_storages, self)
    }
}

pub trait BulkInsert {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntitiesIter<'_>
    where
        Self: Sized;
}

impl BulkInsert for () {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntitiesIter<'_>
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
        BulkEntitiesIter(entities.data[entities_len..].iter().copied())
    }
}

impl<T: 'static + Send + Sync> BulkInsert for (T,) {
    fn bulk_insert<I: IntoIterator<Item = Self>>(
        all_storages: &mut AllStorages,
        iter: I,
    ) -> BulkEntitiesIter<'_> {
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
        if sparse_set.metadata.update.is_some() {
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

        if sparse_set.metadata.update.is_some() {
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

        BulkEntitiesIter(
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
            fn bulk_insert<Source: IntoIterator<Item = Self>>(all_storages: &mut AllStorages, iter: Source) -> BulkEntitiesIter<'_> {
                let iter = iter.into_iter();
                let len = iter.size_hint().0;

                let mut entities = all_storages.entities_mut().unwrap();
                let entities_len = entities.data.len();
                let new_entities = entities.bulk_generate(len);

                let mut $sparse_set1 = all_storages.custom_storage_or_insert_mut(SparseSet::<$type1>::new).unwrap();
                $(
                    let mut $sparse_set = all_storages.custom_storage_or_insert_mut(SparseSet::<$type>::new).unwrap();
                )*

                $sparse_set1.reserve(len);
                $(
                    $sparse_set.reserve(len);
                )*

                for ($type1, $($type,)*) in iter {
                    $sparse_set1.data.push($type1);
                    $(
                        $sparse_set.data.push($type);
                    )*
                }

                let old_len = $sparse_set1.dense.len();
                $sparse_set1.dense.extend_from_slice(new_entities);
                let dense_len = $sparse_set1.dense.len();
                let data_len = $sparse_set1.data.len();
                $sparse_set1.dense.extend((0..data_len - dense_len).map(|_| entities.generate()));

                $(
                    if $sparse_set.metadata.update.is_some() {
                        $sparse_set.dense.extend($sparse_set1.dense[old_len..].iter().copied().map(|mut id| {id.set_inserted(); id}));
                    } else {
                        $sparse_set.dense.extend_from_slice(&$sparse_set1.dense[old_len..]);
                    }
                )*

                let start_entity = $sparse_set1.dense[old_len];
                let end_entity = $sparse_set1.dense[data_len - 1];

                $sparse_set1.sparse.bulk_allocate(start_entity, end_entity);
                $(
                    $sparse_set.sparse.bulk_allocate(start_entity, end_entity);
                )*

                let SparseSet {
                    sparse,
                    dense,
                    metadata,
                    ..
                } = &mut *$sparse_set1;

                if metadata.update.is_some() {
                    for (i, entity) in dense[old_len..].iter_mut().enumerate() {
                        unsafe {
                            *sparse.get_mut_unchecked(*entity) = EntityId::new_from_parts((old_len + i) as u64, 0, 0);
                            entity.set_inserted();
                        }
                    }
                } else {
                    for (i, &entity) in dense[old_len..].iter().enumerate() {
                        unsafe {
                            *sparse.get_mut_unchecked(entity) = EntityId::new_from_parts((old_len + i) as u64, 0, 0);
                        }
                    }
                }

                $(
                    for (i, &entity) in dense[old_len..].iter().enumerate() {
                        unsafe {
                            *$sparse_set.sparse.get_mut_unchecked(entity) = EntityId::new_from_parts((old_len + i) as u64, 0, 0);
                        }
                    }
                )*

                drop((entities, $sparse_set1, $($sparse_set),*));

                BulkEntitiesIter(all_storages.exclusive_storage_mut::<Entities>().unwrap().data[entities_len..].iter().copied())
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
