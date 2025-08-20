use crate::all_storages::{AllStorages, LockPresent, ThreadIdPresent};
use crate::atomic_refcell::AtomicRefCell;
use crate::entities::Entities;
use crate::public_transport::{RwLock, ShipyardRwLock};
use crate::std_thread_id_generator;
use crate::storage::{SBox, StorageId};
use crate::ShipHashMap;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::marker::PhantomData;
use core::sync::atomic::AtomicU64;

pub(crate) struct AllStoragesBuilder<Lock, ThreadId> {
    pub(crate) custom_lock: Option<Box<dyn ShipyardRwLock + Send + Sync>>,
    pub(crate) custom_thread_id: Option<Arc<dyn Fn() -> u64 + Send + Sync>>,
    pub(crate) _phantom: PhantomData<(Lock, ThreadId)>,
}

impl<Lock, ThreadId> AllStoragesBuilder<Lock, ThreadId> {
    #[cfg(feature = "std")]
    pub(crate) fn new() -> AllStoragesBuilder<LockPresent, ThreadIdPresent> {
        AllStoragesBuilder {
            custom_lock: None,
            custom_thread_id: Some(Arc::new(std_thread_id_generator)),
            _phantom: PhantomData,
        }
    }

    #[cfg(all(not(feature = "std"), not(feature = "thread_local")))]
    pub(crate) fn new() -> AllStoragesBuilder<MissingLock, ThreadIdPresent> {
        AllStoragesBuilder {
            custom_lock: None,
            custom_thread_id: None,
            _phantom: PhantomData,
        }
    }

    #[cfg(all(not(feature = "std"), feature = "thread_local"))]
    pub(crate) fn new() -> AllStoragesBuilder<MissingLock, MissingThreadId> {
        AllStoragesBuilder {
            custom_lock: None,
            custom_thread_id: None,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn with_custom_lock<L: ShipyardRwLock + Send + Sync>(
        self,
    ) -> AllStoragesBuilder<LockPresent, ThreadId> {
        AllStoragesBuilder {
            custom_lock: Some(L::new()),
            custom_thread_id: self.custom_thread_id,
            _phantom: PhantomData,
        }
    }

    #[cfg(feature = "thread_local")]
    pub(crate) fn with_custom_thread_id(
        self,
        thread_id: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> AllStoragesBuilder<Lock, ThreadIdPresent> {
        AllStoragesBuilder {
            custom_lock: self.custom_lock,
            custom_thread_id: Some(Arc::new(thread_id)),
            _phantom: PhantomData,
        }
    }
}

impl AllStoragesBuilder<LockPresent, ThreadIdPresent> {
    #[track_caller]
    pub(crate) fn build(self, counter: Arc<AtomicU64>) -> AtomicRefCell<AllStorages> {
        let mut storages = ShipHashMap::new();

        storages.insert(StorageId::of::<Entities>(), SBox::new(Entities::new()));

        let storages = if let Some(custom_lock) = self.custom_lock {
            RwLock::new_custom(custom_lock, storages)
        } else {
            #[cfg(feature = "std")]
            {
                RwLock::new_std(storages)
            }
            #[cfg(not(feature = "std"))]
            {
                unreachable!()
            }
        };

        #[cfg(feature = "thread_local")]
        let thread_id_generator = self.custom_thread_id.unwrap();
        #[cfg(feature = "thread_local")]
        let main_thread_id = (thread_id_generator)();

        #[cfg(feature = "thread_local")]
        {
            AtomicRefCell::new_non_send(
                AllStorages {
                    storages,
                    main_thread_id,
                    thread_id_generator: thread_id_generator.clone(),
                    counter,
                },
                thread_id_generator,
            )
        }
        #[cfg(not(feature = "thread_local"))]
        {
            AtomicRefCell::new(AllStorages { storages, counter })
        }
    }
}
