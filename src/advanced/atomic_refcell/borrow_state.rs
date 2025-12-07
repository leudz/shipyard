use crate::error;
use core::sync::atomic::{AtomicUsize, Ordering};

const HIGH_BIT: usize = !(usize::MAX >> 1);
const MAX_FAILED_BORROWS: usize = HIGH_BIT + (HIGH_BIT >> 1);

pub(super) struct BorrowState(AtomicUsize);

/// Unlocks a shared borrow on drop.
#[must_use]
pub struct SharedBorrow<'a>(&'a BorrowState);

impl Drop for SharedBorrow<'_> {
    #[inline]
    fn drop(&mut self) {
        (self.0).0.fetch_sub(1, Ordering::Release);
    }
}

impl Clone for SharedBorrow<'_> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.read_reborrow()
    }
}

/// Unlocks an exclusive borrow on drop.
#[must_use]
pub struct ExclusiveBorrow<'a>(&'a BorrowState);

impl ExclusiveBorrow<'_> {
    pub(crate) fn shared_reborrow(&self) -> SharedBorrow<'_> {
        self.0.read_reborrow()
    }
}

impl Drop for ExclusiveBorrow<'_> {
    #[inline]
    fn drop(&mut self) {
        (self.0).0.store(0, Ordering::Release);
    }
}

impl BorrowState {
    #[inline]
    pub(super) fn new() -> Self {
        BorrowState(AtomicUsize::new(0))
    }

    #[inline]
    pub(super) fn read(&self) -> Result<SharedBorrow<'_>, error::Borrow> {
        let new = self.0.fetch_add(1, Ordering::Acquire) + 1;
        if new & HIGH_BIT != 0 {
            self.cold_check_overflow(new);

            Err(error::Borrow::Shared)
        } else {
            Ok(SharedBorrow(self))
        }
    }

    #[allow(unused)]
    #[inline]
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
        } else {
            Err(error::Borrow::Shared)
        }
    }

    #[inline]
    pub(super) fn read_reborrow(&self) -> SharedBorrow<'_> {
        let new = self.0.fetch_add(1, Ordering::Acquire) + 1;

        self.check_overflow(new);

        SharedBorrow(self)
    }

    #[inline]
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
        } else {
            Err(error::Borrow::Unique)
        }
    }

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

    #[cold]
    #[inline(never)]
    fn cold_check_overflow(&self, new: usize) {
        self.check_overflow(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reborrow() {
        let borrow = BorrowState::new();
        let write = borrow.write().unwrap();

        let read = write.shared_reborrow();

        assert_eq!(HIGH_BIT + 1, (read.0).0.load(Ordering::Relaxed));
    }
}
