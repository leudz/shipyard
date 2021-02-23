mod custom_delete_any;

pub use custom_delete_any::CustomDeleteAny;

use crate::all_storages::AllStorages;
use crate::storage::StorageId;
use crate::unknown_storage::UnknownStorage;
use hashbrown::hash_set::HashSet;

/// Trait used as a bound for AllStorages::delete_any.
pub trait DeleteAny {
    fn delete_any(all_storages: &mut AllStorages);
}

impl<T: 'static + UnknownStorage + CustomDeleteAny> DeleteAny for T {
    #[inline]
    fn delete_any(all_storages: &mut AllStorages) {
        let mut ids = HashSet::new();

        let storages = all_storages.storages.get_mut();

        if let Some(storage) = storages.get_mut(&StorageId::of::<T>()) {
            storage.get_mut_exclusive::<T>().delete_any(&mut ids);
        }

        for id in ids {
            all_storages.delete_entity(id);
        }
    }
}

macro_rules! impl_delete_any {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: 'static + UnknownStorage + CustomDeleteAny),+> DeleteAny for ($($storage,)+) {
            fn delete_any(all_storages: &mut AllStorages) {
                let mut ids = HashSet::default();

                let storages = all_storages.storages.get_mut();

                $(
                    if let Some(storage) = storages.get_mut(&StorageId::of::<$storage>()) {
                        storage.get_mut_exclusive::<$storage>().delete_any(&mut ids);
                    }
                )+

                for id in ids {
                    all_storages.delete_entity(id);
                }
            }
        }
    }
}

macro_rules! delete_any {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_delete_any![$(($storage, $index))*];
        delete_any![$(($storage, $index))* ($storage1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_delete_any![$(($storage, $index))*];
    }
}

delete_any![(StorageA, 0) (StorageB, 1); (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)];
