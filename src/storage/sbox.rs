mod builder;

pub use builder::SBoxBuilder;

use crate::atomic_refcell::AtomicRefCell;
use crate::storage::Storage;
use alloc::boxed::Box;
#[cfg(feature = "thread_local")]
use alloc::sync::Arc;

/// Owns a boxed `AtomicRefCell<dyn Storage>`, so storages of different types fit in the same registry.
///
/// The registry moves `SBox`es around: its index map, its vector of component and custom storages
/// or its unique map reallocating, a removal moving the last vector slot. The box stays put, so
/// borrows of the storage don't depend on where its `SBox` is.
pub(crate) struct SBox(pub(crate) *mut AtomicRefCell<dyn Storage>);

#[cfg(not(feature = "thread_local"))]
unsafe impl Send for SBox {}

unsafe impl Sync for SBox {}

impl Drop for SBox {
    fn drop(&mut self) {
        // SAFE the pointer came from a `Box` of the same type
        unsafe {
            let _ = Box::from_raw(self.0);
        }
    }
}

impl SBox {
    #[inline]
    pub(crate) fn new<T: Storage + Send + Sync + 'static>(value: T) -> SBox {
        SBox(Box::into_raw(Box::new(AtomicRefCell::new(value))))
    }

    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn new_non_send<T: Storage + Sync + 'static>(
        value: T,
        thread_id: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> SBox {
        SBox(Box::into_raw(Box::new(AtomicRefCell::new_non_send(
            value, thread_id,
        ))))
    }

    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn new_non_sync<T: Storage + Send + 'static>(value: T) -> SBox {
        SBox(Box::into_raw(Box::new(AtomicRefCell::new_non_sync(value))))
    }

    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn new_non_send_sync<T: Storage + 'static>(
        value: T,
        thread_id: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> SBox {
        SBox(Box::into_raw(Box::new(AtomicRefCell::new_non_send_sync(
            value, thread_id,
        ))))
    }

    /// Returns the storage this box holds.
    ///
    /// The exclusive reference to the box makes runtime borrow checks unnecessary.
    #[inline]
    pub(crate) fn storage_mut(&mut self) -> &mut (dyn Storage + 'static) {
        // SAFE: `self.0` comes from a `Box` this `SBox` owns, it lives as long as `self`.
        // We borrow `self` mutably for as long as the reference lives, no other reference
        // to this storage can exist.
        unsafe { &mut *self.0 }.get_mut()
    }
}

impl core::fmt::Debug for SBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Ok(storage) = unsafe { &*self.0 }.borrow() {
            f.write_str(&storage.name())
        } else {
            f.write_str("Could not borrow storage")
        }
    }
}
