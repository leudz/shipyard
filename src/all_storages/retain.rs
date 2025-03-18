use crate::all_storages::AllStorages;
use crate::entity_id::EntityId;
use crate::storage::{Storage, StorageId};
#[cfg(doc)]
use crate::world::World;

/// Trait used as bound for [`World::retain_storage`] and [`AllStorages::retain_storage`].
pub trait TupleRetainStorage {
    /// See [`World::retain_storage`] and [`AllStorages::retain_storage`].
    fn retain(all_storage: &mut AllStorages, entity: EntityId);
}

impl TupleRetainStorage for () {
    #[inline]
    fn retain(_: &mut AllStorages, _: EntityId) {}
}

impl<S: 'static + Storage> TupleRetainStorage for S {
    #[inline]
    fn retain(all_storages: &mut AllStorages, entity: EntityId) {
        all_storages.retain_storage_by_id(entity, &[StorageId::of::<S>()]);
    }
}

macro_rules! impl_retain {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: 'static + Storage),+> TupleRetainStorage for ($($storage,)+) {
            #[inline]
            fn retain(all_storages: &mut AllStorages, entity: EntityId) {
                all_storages.retain_storage_by_id(entity, &[$(StorageId::of::<$storage>()),+]);
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

#[cfg(not(feature = "extended_tuple"))]
retain![(StorageA, 0); (StorageB, 1) (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)];
#[cfg(feature = "extended_tuple")]
retain![
    (StorageA, 0); (StorageB, 1) (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)
    (StorageK, 10) (StorageL, 11) (StorageM, 12) (StorageN, 13) (StorageO, 14) (StorageP, 15) (StorageQ, 16) (StorageR, 17) (StorageS, 18) (StorageT, 19)
    (StorageU, 20) (StorageV, 21) (StorageW, 22) (StorageX, 23) (StorageY, 24) (StorageZ, 25) (StorageAA, 26) (StorageBB, 27) (StorageCC, 28) (StorageDD, 29)
    (StorageEE, 30) (StorageFF, 31)
];
