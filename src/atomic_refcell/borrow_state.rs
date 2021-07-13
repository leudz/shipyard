use crate::error;
use core::sync::atomic::{AtomicUsize, Ordering};

const HIGH_BIT: usize = !(usize::MAX >> 1);
const MAX_FAILED_BORROWS: usize = HIGH_BIT + (HIGH_BIT >> 1);

pub(super) struct BorrowState(AtomicUsize);

/// Unlocks a shared borrow on drop.
pub struct SharedBorrow<'a>(&'a BorrowState);

impl Drop for SharedBorrow<'_> {
    fn drop(&mut self) {
        (self.0).0.fetch_sub(1, Ordering::Release);
    }
}

impl Clone for SharedBorrow<'_> {
    fn clone(&self) -> Self {
        self.0.read().unwrap()
    }
}

/// Unlocks an exclusive borrow on drop.
pub struct ExclusiveBorrow<'a>(&'a BorrowState);

impl Drop for ExclusiveBorrow<'_> {
    fn drop(&mut self) {
        (self.0).0.store(0, Ordering::Release);
    }
}

impl BorrowState {
    pub(super) fn new() -> Self {
        BorrowState(AtomicUsize::new(0))
    }
    pub(super) fn read(&self) -> Result<SharedBorrow<'_>, error::Borrow> {
        let new = self.0.fetch_add(1, Ordering::Acquire) + 1;
        if new & HIGH_BIT != 0 {
            self.check_overflow(new);

            Err(error::Borrow::Unique)
        } else {
            Ok(SharedBorrow(self))
        }
    }

    // todo: use
    #[allow(unused)]
    pub(super) fn exclusive_read(&self) -> Result<SharedBorrow<'_>, error::Borrow> {
        let old = match self
            .0
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(x) => x,
            Err(x) => x,
        };

        if old == 0 {
            Ok(SharedBorrow(self))
        } else if old & HIGH_BIT == 0 {
            Err(error::Borrow::Shared)
        } else {
            Err(error::Borrow::Unique)
        }
    }

    pub(super) fn write(&self) -> Result<ExclusiveBorrow<'_>, error::Borrow> {
        let old = match self
            .0
            .compare_exchange(0, HIGH_BIT, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(x) => x,
            Err(x) => x,
        };

        if old == 0 {
            Ok(ExclusiveBorrow(self))
        } else if old & HIGH_BIT == 0 {
            Err(error::Borrow::Shared)
        } else {
            Err(error::Borrow::Unique)
        }
    }

    #[cold]
    #[inline(never)]
    fn check_overflow(&self, new: usize) {
        if new == HIGH_BIT {
            self.0.fetch_sub(1, Ordering::Release);

            panic!("too many immutable borrows");
        } else if new >= MAX_FAILED_BORROWS {
            struct ForceAbort;
            impl Drop for ForceAbort {
                fn drop(&mut self) {
                    panic!("Aborting to avoid unsound state of AtomicRefCell");
                }
            }
            let _abort = ForceAbort;
            panic!("Too many failed borrows");
        }
    }
}
