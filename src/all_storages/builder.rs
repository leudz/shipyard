#[cfg(not(feature = "std"))]
use crate::all_storages::MissingLock;
#[cfg(all(not(feature = "std"), feature = "thread_local"))]
use crate::all_storages::MissingThreadId;
use crate::all_storages::{AllStorages, Grouping, LockPresent, StorageMaps, ThreadIdPresent};
use crate::atomic_refcell::AtomicRefCell;
use crate::entities::Entities;
use crate::public_transport::{RwLock, ShipyardRwLock};
#[cfg(feature = "std")]
use crate::std_thread_id_generator;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::marker::PhantomData;
use core::sync::atomic::AtomicU64;

pub(crate) struct AllStoragesBuilder<Lock, ThreadId> {
    pub(crate) custom_lock: Option<Box<dyn ShipyardRwLock + Send + Sync>>,
    pub(crate) custom_thread_id: Option<Arc<dyn Fn() -> u64 + Send + Sync>>,
    pub(crate) grouping: Grouping,
    pub(crate) _phantom: PhantomData<(Lock, ThreadId)>,
}

impl<Lock, ThreadId> AllStoragesBuilder<Lock, ThreadId> {
    #[cfg(feature = "std")]
    pub(crate) fn new() -> AllStoragesBuilder<LockPresent, ThreadIdPresent> {
        AllStoragesBuilder {
            custom_lock: None,
            custom_thread_id: Some(Arc::new(std_thread_id_generator)),
            grouping: Grouping::default(),
            _phantom: PhantomData,
        }
    }

    #[cfg(all(not(feature = "std"), not(feature = "thread_local")))]
    pub(crate) fn new() -> AllStoragesBuilder<MissingLock, ThreadIdPresent> {
        AllStoragesBuilder {
            custom_lock: None,
            custom_thread_id: None,
            grouping: Grouping::default(),
            _phantom: PhantomData,
        }
    }

    #[cfg(all(not(feature = "std"), feature = "thread_local"))]
    pub(crate) fn new() -> AllStoragesBuilder<MissingLock, MissingThreadId> {
        AllStoragesBuilder {
            custom_lock: None,
            custom_thread_id: None,
            grouping: Grouping::default(),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn with_custom_lock<L: ShipyardRwLock + Send + Sync>(
        self,
    ) -> AllStoragesBuilder<LockPresent, ThreadId> {
        AllStoragesBuilder {
            custom_lock: Some(L::new()),
            custom_thread_id: self.custom_thread_id,
            grouping: self.grouping,
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
            grouping: self.grouping,
            _phantom: PhantomData,
        }
    }
}

impl AllStoragesBuilder<LockPresent, ThreadIdPresent> {
    #[track_caller]
    pub(crate) fn build(self, counter: Arc<AtomicU64>) -> AtomicRefCell<AllStorages> {
        let storages = StorageMaps::new();

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

        let mut all_storages = AllStorages {
            entities: AtomicRefCell::new(Entities::new()),
            storages,
            #[cfg(feature = "thread_local")]
            main_thread_id,
            #[cfg(feature = "thread_local")]
            thread_id_generator: thread_id_generator.clone(),
            counter,
            grouping: Grouping::default(),
        };

        self.grouping.install(&mut all_storages);

        #[cfg(feature = "thread_local")]
        {
            AtomicRefCell::new_non_send(all_storages, thread_id_generator)
        }
        #[cfg(not(feature = "thread_local"))]
        {
            AtomicRefCell::new(all_storages)
        }
    }
}
