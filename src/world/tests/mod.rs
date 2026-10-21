mod miri;

use crate::World;
use crate::all_storages::{LockPresent, ThreadIdPresent};
use crate::world::WorldBuilder;

#[cfg(test)]
impl World {
    /// Creates a `World` usable from unit tests with or without the `std` feature.
    ///
    /// See [`World::builder_for_test`] for how it's built without `std`.
    pub(crate) fn new_for_test() -> Self {
        #[cfg(feature = "std")]
        {
            Self::new()
        }
        #[cfg(not(feature = "std"))]
        {
            Self::builder_for_test().build()
        }
    }

    /// Returns a `WorldBuilder` usable from unit tests with or without the `std` feature.
    ///
    /// Without `std` there is no default lock, so we use `parking_lot`'s.\
    /// There is no default thread id either, so with `thread_local` every thread gets the same id,
    /// like on a platform without threads.
    pub(crate) fn builder_for_test() -> WorldBuilder<LockPresent, ThreadIdPresent> {
        #[cfg(feature = "std")]
        {
            Self::builder()
        }
        #[cfg(all(not(feature = "std"), not(feature = "thread_local")))]
        {
            Self::builder().with_custom_lock::<parking_lot::RawRwLock>()
        }
        #[cfg(all(not(feature = "std"), feature = "thread_local"))]
        {
            Self::builder()
                .with_custom_lock::<parking_lot::RawRwLock>()
                .with_custom_thread_id(|| 0)
        }
    }
}
