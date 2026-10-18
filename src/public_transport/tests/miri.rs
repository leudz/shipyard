//! Focused Miri scenarios for the world's `RwLock` and its guards.
//!
//! The custom-lock tests use a real parking_lot lock that we can also inspect, so they check the
//! lock state directly instead of relying on a second acquisition to hang when a release is lost.

use crate::public_transport::{RwLock, ShipyardRwLock};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use lock_api::RawRwLock as _;

/// A parking_lot lock shared between `RwLock` and the test.
///
/// It forwards everything to parking_lot's `ShipyardRwLock` impl, so the lock is fully enforced.
/// The `Arc` only lets the test look at the lock's state while `RwLock` owns the boxed handle.
struct ObservedLock(Arc<parking_lot::RawRwLock>);

impl ShipyardRwLock for ObservedLock {
    fn new() -> Box<dyn ShipyardRwLock + Send + Sync> {
        Box::new(ObservedLock(Arc::new(parking_lot::RawRwLock::INIT)))
    }

    fn lock_shared(&self) {
        ShipyardRwLock::lock_shared(&*self.0);
    }

    unsafe fn unlock_shared(&self) {
        // SAFE: we forward to the same inner lock our caller locked through `lock_shared`, so the
        // shared lock it holds on us is a shared lock on the inner lock.
        unsafe { ShipyardRwLock::unlock_shared(&*self.0) }
    }

    fn lock_exclusive(&self) {
        ShipyardRwLock::lock_exclusive(&*self.0);
    }

    unsafe fn unlock_exclusive(&self) {
        // SAFE: we forward to the same inner lock our caller locked through `lock_exclusive`, so
        // the exclusive lock it holds on us is an exclusive lock on the inner lock.
        unsafe { ShipyardRwLock::unlock_exclusive(&*self.0) }
    }
}

/// Returns a custom `RwLock` holding `value`, along with a handle to inspect its raw lock.
fn observed_lock<T>(value: T) -> (RwLock<T>, Arc<parking_lot::RawRwLock>) {
    let raw = Arc::new(parking_lot::RawRwLock::INIT);
    let lock = RwLock::new_custom(Box::new(ObservedLock(raw.clone())), value);

    (lock, raw)
}

/// Asserts that two read guards from a custom lock both read the value while holding the lock
/// in shared mode, and that the lock is released only once both are dropped.
#[test]
fn custom_read() {
    let (lock, raw) = observed_lock(vec![1u32, 2]);

    let first = lock.read();
    let second = lock.read();
    assert_eq!(first[0] + second[1], 3);
    assert!(!raw.try_lock_exclusive());

    drop(first);
    assert!(raw.is_locked());
    assert_eq!(second.len(), 2);

    drop(second);
    assert!(!raw.is_locked());
}

/// Asserts that writes through a custom write guard reach the value, that the lock is held
/// exclusively while the guard is alive, and that the next reader sees the writes.
#[test]
fn custom_write() {
    let (lock, raw) = observed_lock(vec![1u32]);

    let mut guard = lock.write();
    guard.push(2);
    assert!(raw.is_locked_exclusive());
    assert!(!raw.try_lock_shared());
    guard[0] = 3;

    drop(guard);
    assert!(!raw.is_locked());
    assert_eq!(*lock.read(), [3, 2]);
}

/// Asserts that `get_mut` on a custom lock reaches the value without touching the lock.
#[test]
fn custom_get_mut() {
    let (mut lock, raw) = observed_lock(vec![1u32]);

    lock.get_mut().push(2);
    assert!(!raw.is_locked());

    assert_eq!(*lock.read(), [1, 2]);
}

/// Asserts that a panic while a custom write guard is held unlocks the lock as it unwinds, so
/// the next writer gets in and sees the writes made before the panic.
#[cfg(feature = "std")]
#[test]
fn custom_write_unwind() {
    let (lock, raw) = observed_lock(vec![1u32]);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let mut guard = lock.write();
        guard.push(2);
        panic!("panic while holding the write guard");
    }));
    assert!(result.is_err());
    assert!(!raw.is_locked());

    lock.write().push(3);
    assert_eq!(*lock.read(), [1, 2, 3]);
}

/// Asserts that the std-backed lock's read guards share the lock and keep writers out, and that
/// its write guard and `get_mut` both reach the value once the readers are gone.
#[cfg(feature = "std")]
#[test]
fn std_guards() {
    let mut lock = RwLock::new_std(vec![1u32]);

    {
        let RwLock::Std { lock: std_lock } = &lock else {
            unreachable!()
        };

        let first = lock.read();
        let second = lock.read();
        assert_eq!(first[0] + second[0], 2);
        assert!(std_lock.try_write().is_err());

        drop((first, second));
        assert!(std_lock.try_write().is_ok());
    }

    lock.write().push(2);
    lock.get_mut().push(3);

    assert_eq!(*lock.read(), [1, 2, 3]);
}
