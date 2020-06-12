use crate::storage::AllStorages;
use crate::storage::EntityId;
use crate::storage::TypeIdHasher;
use core::any::TypeId;
use core::hash::BuildHasherDefault;
use hashbrown::hash_set::HashSet;

/// Trait used as a bound for AllStorages::delete_any.
pub trait DeleteAny {
    fn delete_any(all_storages: &mut AllStorages);
}

impl<T: 'static> DeleteAny for (T,) {
    fn delete_any(all_storages: &mut AllStorages) {
        // we have an exclusive reference so it's ok to not lock and still get a reference
        let storages = unsafe { &*all_storages.storages.get() };
        if let Some(storage) = storages.get(&TypeId::of::<T>().into()) {
            if let Ok(mut sparse_set) = storage.sparse_set_mut::<T>() {
                let ids = sparse_set.dense.clone();
                sparse_set.clear();
                drop(sparse_set);
                for id in ids {
                    all_storages.delete(id);
                }
            }
        }
    }
}

macro_rules! impl_delete_any {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static),+> DeleteAny for ($($type,)+) {
            fn delete_any(all_storages: &mut AllStorages) {
                // we have an exclusive reference so it's ok to not lock and still get a reference
                let storages = unsafe { &*all_storages.storages.get() };
                let mut ids: HashSet<EntityId, BuildHasherDefault<TypeIdHasher>> = HashSet::default();

                $(
                    if let Some(storage) = storages.get(&TypeId::of::<$type>().into()) {
                        if let Ok(mut sparse_set) = storage.sparse_set_mut::<$type>() {
                            ids.extend(&sparse_set.dense);
                            sparse_set.clear();
                        }
                    }
                )+

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
