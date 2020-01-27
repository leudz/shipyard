mod storage_borrow;
mod system;
mod system_data;

pub use storage_borrow::StorageBorrow;
pub(crate) use system::Dispatch;
pub use system::System;
pub(crate) use system_data::Mutation;
pub use system_data::SystemData;

use crate::atomic_refcell::AtomicRefCell;
use crate::error;
use crate::storage::AllStorages;
#[cfg(feature = "parallel")]
use rayon::ThreadPool;

pub trait Run<'a> {
    type Storage;

    fn try_run<R, F: FnOnce(Self::Storage) -> R>(
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
        f: F,
    ) -> Result<R, error::GetStorage>;
}

impl<'a, T: SystemData<'a>> Run<'a> for T {
    type Storage = T::View;

    fn try_run<R, F: FnOnce(Self::Storage) -> R>(
        storages: &'a AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'a ThreadPool,
        f: F,
    ) -> Result<R, error::GetStorage> {
        let storage = {
            #[cfg(feature = "parallel")]
            {
                T::try_borrow(storages, thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                T::try_borrow(storages)?
            }
        };

        Ok(f(storage))
    }
}
