use crate::all_storages::AllStorages;
use crate::entity_id::EntityId;
use crate::storage::{Storage, StorageId};

pub trait Retain {
    fn retain(all_storage: &mut AllStorages, entity: EntityId);
}

impl Retain for () {
    #[inline]
    fn retain(_: &mut AllStorages, _: EntityId) {}
}

impl<S: 'static + Storage> Retain for S {
    #[inline]
    fn retain(all_storages: &mut AllStorages, entity: EntityId) {
        all_storages.retain_storage(entity, &[StorageId::of::<S>()]);
    }
}

macro_rules! impl_retain {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: 'static + Storage),+> Retain for ($($storage,)+) {
            #[inline]
            fn retain(all_storages: &mut AllStorages, entity: EntityId) {
                all_storages.retain_storage(entity, &[$(StorageId::of::<$storage>()),+]);
            }
        }
    }
}

macro_rules! retain {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_retain![$(($storage, $index))*];
        retain![$(($storage, $index))* ($storage1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_retain![$(($storage, $index))*];
    }
}

retain![(StorageA, 0) (StorageB, 1); (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)];
