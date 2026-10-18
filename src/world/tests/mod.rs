mod miri;

use crate::World;

#[cfg(test)]
impl World {
    /// Creates a `World` usable from unit tests with or without the `std` feature.
    ///
    /// Without `std` there is no default lock, so we build the world with `parking_lot`'s.\
    /// There is no default thread id either, so with `thread_local` every thread gets the same id,
    /// like on a platform without threads.
    pub(crate) fn new_for_test() -> Self {
        #[cfg(feature = "std")]
        {
            Self::new()
        }
        #[cfg(all(not(feature = "std"), not(feature = "thread_local")))]
        {
            Self::builder()
                .with_custom_lock::<parking_lot::RawRwLock>()
                .build()
        }
        #[cfg(all(not(feature = "std"), feature = "thread_local"))]
        {
            Self::builder()
                .with_custom_lock::<parking_lot::RawRwLock>()
                .with_custom_thread_id(|| 0)
                .build()
        }
    }
}
