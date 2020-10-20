use crate::storage::EntityId;
use crate::storage::{AllStorages, StorageId};
use crate::unknown_storage::UnknownStorage;

pub trait StripExcept {
    fn strip_except(all_storage: &mut AllStorages, entity: EntityId);
}

impl StripExcept for () {
    #[inline]
    fn strip_except(_: &mut AllStorages, _: EntityId) {}
}

impl<Storage: 'static + UnknownStorage> StripExcept for Storage {
    #[inline]
    fn strip_except(all_storages: &mut AllStorages, entity: EntityId) {
        all_storages.strip_except_storage(entity, &[StorageId::of::<Storage>()]);
    }
}

macro_rules! impl_strip_except {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: 'static + UnknownStorage),+> StripExcept for ($($storage,)+) {
            #[inline]
            fn strip_except(all_storages: &mut AllStorages, entity: EntityId) {
                all_storages.strip_except_storage(entity, &[$(StorageId::of::<$storage>()),+]);
            }
        }
    }
}

macro_rules! strip_except {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_strip_except![$(($storage, $index))*];
        strip_except![$(($storage, $index))* ($storage1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_strip_except![$(($storage, $index))*];
    }
}

strip_except![(StorageA, 0) (StorageB, 1); (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)];
