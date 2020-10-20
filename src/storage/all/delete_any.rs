use crate::sparse_set::SparseSet;
use crate::storage::{AllStorages, EntityId, StorageId, TypeIdHasher};
use alloc::vec::Vec;
use core::hash::BuildHasherDefault;
use hashbrown::hash_set::HashSet;

/// Trait used as a bound for AllStorages::delete_any.
pub trait DeleteAny {
    fn delete_any(all_storages: &mut AllStorages);
}

impl<T: 'static> DeleteAny for (T,) {
    fn delete_any(all_storages: &mut AllStorages) {
        let mut ids = Vec::new();

        {
            // we have an exclusive reference so it's ok to not lock and still get a reference
            let storages = unsafe { &mut *all_storages.storages.get() };

            if let Some(storage) = storages.get_mut(&StorageId::of::<SparseSet<T>>()) {
                // SAFE this is not `AllStorages`
                let sparse_set = storage.get_mut_exclusive::<SparseSet<T>>();
                ids = sparse_set.dense.clone();
                sparse_set.clear();
            }
        }

        for id in ids {
            all_storages.delete(id);
        }
    }
}

macro_rules! impl_delete_any {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static),+> DeleteAny for ($($type,)+) {
            fn delete_any(all_storages: &mut AllStorages) {
                let mut ids: HashSet<EntityId, BuildHasherDefault<TypeIdHasher>> = HashSet::default();

                {
                    // we have an exclusive reference so it's ok to not lock and still get a reference
                    let storages = unsafe { &mut *all_storages.storages.get() };

                    $(
                        if let Some(storage) = storages.get_mut(&StorageId::of::<SparseSet<$type>>()) {
                            // SAFE this is not `AllStorages`
                            let sparse_set = storage.get_mut_exclusive::<SparseSet::<$type>>();
                            ids.extend(&sparse_set.dense);
                            sparse_set.clear();
                        }
                    )+
                }

                for id in ids {
                    all_storages.delete(id);
                }
            }
        }
    }
}

macro_rules! delete_any {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_delete_any![$(($type, $index))*];
        delete_any![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_delete_any![$(($type, $index))*];
    }
}

delete_any![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
