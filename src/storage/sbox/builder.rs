use crate::storage::{SBox, Storage};
#[cfg(feature = "thread_local")]
use alloc::sync::Arc;
use core::marker::PhantomData;

/// Encapsulates the erased boxed storage to apply correct `Send` and `Sync` information.
///
/// Used as a return type for [`Storage::try_clone`].
pub struct SBoxBuilder {
    pub(crate) sbox: SBox,
    pub(crate) _non_send_sync: PhantomData<*const ()>,
}

impl SBoxBuilder {
    /// Erases the type of the storage to store it in a [`World`](crate::world::World).
    #[cfg(not(feature = "thread_local"))]
    pub fn new<T: Storage + Send + Sync + 'static>(value: T) -> SBoxBuilder {
        SBoxBuilder {
            sbox: SBox::new(value),
            _non_send_sync: PhantomData,
        }
    }

    /// Erases the type of the storage to store it in a [`World`](crate::world::World).
    #[cfg(feature = "thread_local")]
    pub fn new<T: Storage + 'static>(value: T) -> SBoxBuilder {
        SBoxBuilder {
            // We cannot use `unreachable!()` here as we are going to use it in `SBox::new_non_send_sync`
            // before it gets overwritten with the correct closure.
            sbox: SBox::new_non_send_sync(value, Arc::new(|| u64::MAX)),
            _non_send_sync: PhantomData,
        }
    }

    #[cfg(not(feature = "thread_local"))]
    pub(crate) fn build(self) -> SBox {
        self.sbox
    }

    /// # Safety
    ///
    /// The information passed into this function must be accurate to this type.
    #[cfg(feature = "thread_local")]
    pub(crate) unsafe fn build(
        self,
        thread_id_generator: Arc<dyn Fn() -> u64 + Send + Sync>,
        is_send: bool,
        is_sync: bool,
    ) -> SBox {
        let storage = unsafe { &mut *self.sbox.0 };
        storage.override_send_sync(thread_id_generator, is_send, is_sync);

        self.sbox
    }
}
