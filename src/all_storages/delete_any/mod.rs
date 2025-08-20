mod custom_delete_any;

use crate::all_storages::AllStorages;
use crate::storage::{Storage, StorageId};
#[cfg(doc)]
use crate::world::World;
use crate::ShipHashSet;
pub use custom_delete_any::CustomDeleteAny;

/// Trait used as a bound for [`World::delete_any`] and [AllStorages::delete_any].
pub trait TupleDeleteAny {
    /// See [`World::delete_any`] and [`AllStorages::delete_any`]
    fn delete_any(all_storages: &mut AllStorages);
}

impl<T: 'static + Storage + CustomDeleteAny> TupleDeleteAny for T {
    #[inline]
    #[track_caller]
    fn delete_any(all_storages: &mut AllStorages) {
        let mut ids = ShipHashSet::new();

        let current = all_storages.get_current();
        let storages = all_storages.storages.get_mut();

        if let Some(storage) = storages.get_mut(&StorageId::of::<T>()) {
            unsafe { &mut *storage.0 }
                .get_mut()
                .as_any_mut()
                .downcast_mut::<T>()
                .unwrap()
                .delete_any(&mut ids, current);
        }

        for id in ids {
            all_storages.delete_entity(id);
        }
    }
}

macro_rules! impl_delete_any {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: 'static + Storage + CustomDeleteAny),+> TupleDeleteAny for ($($storage,)+) {
            fn delete_any(all_storages: &mut AllStorages) {
                let mut ids = ShipHashSet::new();

                let current = all_storages.get_current();
                let storages = all_storages.storages.get_mut();

                $(
                    if let Some(storage) = storages.get_mut(&StorageId::of::<$storage>()) {
                        unsafe { &mut *storage.0 }.get_mut().as_any_mut().downcast_mut::<$storage>().unwrap().delete_any(&mut ids, current);
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

#[cfg(not(feature = "extended_tuple"))]
delete_any![(StorageA, 0); (StorageB, 1) (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)];
#[cfg(feature = "extended_tuple")]
delete_any![
    (StorageA, 0); (StorageB, 1) (StorageC, 2) (StorageD, 3) (StorageE, 4) (StorageF, 5) (StorageG, 6) (StorageH, 7) (StorageI, 8) (StorageJ, 9)
    (StorageK, 10) (StorageL, 11) (StorageM, 12) (StorageN, 13) (StorageO, 14) (StorageP, 15) (StorageQ, 16) (StorageR, 17) (StorageS, 18) (StorageT, 19)
    (StorageU, 20) (StorageV, 21) (StorageW, 22) (StorageX, 23) (StorageY, 24) (StorageZ, 25) (StorageAA, 26) (StorageBB, 27) (StorageCC, 28) (StorageDD, 29)
    (StorageEE, 30) (StorageFF, 31)
];
