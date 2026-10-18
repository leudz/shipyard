mod miri;

use crate::World;

#[cfg(test)]
impl World {
    /// Creates a `World` usable from unit tests with or without the `std` feature.
    ///
    /// Without `std` there is no default lock, so we build the world with `parking_lot`'s.
    pub(crate) fn new_for_test() -> Self {
        #[cfg(feature = "std")]
        {
            Self::new()
        }
        #[cfg(not(feature = "std"))]
        {
            Self::builder()
                .with_custom_lock::<parking_lot::RawRwLock>()
                .build()
        }
    }
}
