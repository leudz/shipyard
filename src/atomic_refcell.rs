use crate::error;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Threadsafe `RefCell`-like container.
/// Can be shared across threads as long as the inner type is `Sync`.
pub(crate) struct AtomicRefCell<T: ?Sized> {
    borrow_state: BorrowState,
    inner: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for AtomicRefCell<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for AtomicRefCell<T> {}

impl<T> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    pub(crate) fn new(value: T) -> Self {
        AtomicRefCell {
            inner: UnsafeCell::new(value),
            borrow_state: Default::default(),
        }
    }
    /// Immutably borrows the wrapped value, returning an error if the value is currently mutably
    /// borrowed.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple shared borrows can be
    /// taken out at the same time.
    pub(crate) fn try_borrow(&self) -> Result<Ref<T>, error::Borrow> {
        Ok(Ref {
            inner: unsafe { &*self.inner.get() },
            borrow: self.borrow_state.try_borrow()?,
        })
    }
    /// Mutably borrows the wrapped value, returning an error if the value is currently borrowed.
    ///
    /// The borrow lasts until the returned `RefMut` exits scope. The value cannot be borrowed while this borrow is
    /// active.
    pub(crate) fn try_borrow_mut(&self) -> Result<RefMut<T>, error::Borrow> {
        Ok(RefMut {
            inner: unsafe { &mut *self.inner.get() },
            borrow: self.borrow_state.try_borrow_mut()?,
        })
    }
}

/// `BorrowState` keeps track of which borrow is currently active.
// If `HIGH_BIT` is set, it is a unique borrow, in all other cases it is a shared borrowed
#[doc(hidden)]
pub struct BorrowState(AtomicUsize);

const HIGH_BIT: usize = !(std::usize::MAX >> 1);
const MAX_FAILED_BORROWS: usize = HIGH_BIT + (HIGH_BIT >> 1);

impl BorrowState {
    // Each borrow will add one, check if no unique borrow is active before returning
    // Even in case of failure the incrementation leave the value in a valid state
    pub(crate) fn try_borrow(&self) -> Result<Borrow, error::Borrow> {
        let new = self.0.fetch_add(1, Ordering::Acquire) + 1;

        if new & HIGH_BIT != 0 {
            Err(Self::try_recover(self, new))
        } else {
            Ok(Borrow::Shared(self))
        }
    }
    // Can only make a unique borrow when no borrows are active
    // Use `compare_exchange` to keep the value in a valid state even in case of failure
    pub(crate) fn try_borrow_mut(&self) -> Result<Borrow, error::Borrow> {
        match self
            .0
            .compare_exchange(0, HIGH_BIT, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(_) => Ok(Borrow::Unique(self)),
            Err(x) if x & HIGH_BIT == 0 => Err(error::Borrow::Unique),
            _ => Err(error::Borrow::Shared),
        }
    }
    // In case of a failled shared borrow, check all possible causes and recover from it when possible
    // If `new == HIGH_BIT` there is `isize::MAX` active or forgotten shared borrows
    // If `new >= MAX_FAILED_BORROWS` there is a unique borrows and `isize::MAX` attenpts to borrow immutably
    // In all other cases, a unique borrow is active
    fn try_recover(&self, new: usize) -> error::Borrow {
        if new == HIGH_BIT {
            self.0.fetch_sub(1, Ordering::Release);
            panic!("Too many shared borrows");
        } else if new >= MAX_FAILED_BORROWS {
            println!("Too many failed borrows");
            std::process::exit(1);
        } else {
            // Tries to go back to the previous state, even if it fails the state is still valid
            // Going back only allow more tries before hitting `MAX_FAILED_BORROWS`
            let _ = self
                .0
                .compare_exchange(new, new - 1, Ordering::Release, Ordering::Relaxed);
            error::Borrow::Unique
        }
    }
}

impl Default for BorrowState {
    fn default() -> Self {
        BorrowState(AtomicUsize::new(0))
    }
}

pub enum Borrow<'a> {
    Shared(&'a BorrowState),
    Unique(&'a BorrowState),
}

impl<'a> Drop for Borrow<'a> {
    fn drop(&mut self) {
        match self {
            Borrow::Shared(borrow) => {
                let old = borrow.0.fetch_sub(1, Ordering::Release);
                debug_assert!(old & HIGH_BIT == 0);
            }
            Borrow::Unique(borrow) => {
                borrow.0.store(0, Ordering::Release);
            }
        }
    }
}

/// A wrapper type for a shared borrow from a `AtomicRefCell<T>`.
pub struct Ref<'a, T: ?Sized> {
    pub(crate) inner: &'a T,
    pub(crate) borrow: Borrow<'a>,
}

impl<T: ?Sized> std::ops::Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner
    }
}

/// A wrapper type for a unique borrow from a `AtomicRefCell<T>`.
pub struct RefMut<'a, T: ?Sized> {
    pub(crate) inner: &'a mut T,
    pub(crate) borrow: Borrow<'a>,
}

impl<T: ?Sized> std::ops::Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner
    }
}

impl<T: ?Sized> std::ops::DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner
    }
}
