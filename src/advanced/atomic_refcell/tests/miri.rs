//! Focused Miri scenarios for `AtomicRefCell` and its guards.
//!
//! The values are heap-owning so Miri tracks the borrowed allocation, and every test reads or
//! writes through a guard while it's alive, then checks the cell is usable once it's gone.

use crate::atomic_refcell::{ARef, ARefMut, AtomicRefCell};
use crate::error;
use alloc::vec;
use alloc::vec::Vec;

/// Two separately borrowed fields, so the mapping tests can narrow a guard to one of them.
struct Pair {
    left: Vec<u32>,
    right: Vec<u32>,
}

impl Pair {
    fn new() -> Pair {
        Pair {
            left: vec![1],
            right: vec![2, 3],
        }
    }
}

/// Mapping callback that reads the value it was handed, then unwinds instead of returning.
#[cfg(feature = "std")]
fn panicking_map(pair: &Pair) -> &Vec<u32> {
    assert_eq!(pair.left, [1]);
    panic!("shared mapping callback panicked");
}

/// Mutable mapping callback that writes to the value it was handed, then unwinds.
#[cfg(feature = "std")]
fn panicking_map_mut(pair: &mut Pair) -> &mut Vec<u32> {
    pair.left.push(9);
    panic!("exclusive mapping callback panicked");
}

/// Asserts that two live shared guards both read the value and keep writers out until the last
/// one is dropped, even when they're dropped in acquisition order.
#[test]
fn shared_guards() {
    let cell = AtomicRefCell::new(vec![1u32, 2, 3]);

    let first = cell.borrow().unwrap();
    let second = cell.borrow().unwrap();
    assert_eq!(first[0] + second[2], 4);
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));

    // The second guard is still reading, so dropping the first mustn't unlock the cell.
    drop(first);
    assert_eq!(second.len(), 3);
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));

    drop(second);
    cell.borrow_mut().unwrap().push(4);
    assert_eq!(*cell.borrow().unwrap(), [1, 2, 3, 4]);
}

/// Asserts that writes through an exclusive guard land in the cell, that both readers and
/// writers are rejected while it's alive, and that the cell unlocks once it's dropped.
#[test]
fn exclusive_guard() {
    let cell = AtomicRefCell::new(vec![1u32]);

    let mut guard = cell.borrow_mut().unwrap();
    guard.push(2);
    assert_eq!(cell.borrow().err(), Some(error::Borrow::Shared));
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));
    guard[0] = 10;

    drop(guard);
    assert_eq!(*cell.borrow().unwrap(), [10, 2]);
}

/// Asserts that a cloned shared guard holds its own borrow: the original can be dropped first
/// and the clone still reads the value and keeps writers out.
#[test]
fn cloned_guard() {
    let cell = AtomicRefCell::new(vec![7u32]);

    let original = cell.borrow().unwrap();
    let clone = original.clone();
    drop(original);

    assert_eq!(clone[0], 7);
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));

    drop(clone);
    assert!(cell.borrow_mut().is_ok());
}

/// Asserts that a shared guard mapped to a field reads through the narrowed reference and keeps
/// the original borrow, so writers stay out until the mapped guard is dropped.
#[test]
fn map_shared() {
    let cell = AtomicRefCell::new(Pair::new());

    let right = ARef::map(cell.borrow().unwrap(), |pair| &pair.right);
    assert_eq!(*right, [2, 3]);
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));

    drop(right);
    cell.borrow_mut().unwrap().left.push(4);
    assert_eq!(cell.borrow().unwrap().left, [1, 4]);
}

/// Asserts that an exclusive guard mapped to a field writes through the narrowed reference and
/// keeps the original borrow, so readers stay out until the mapped guard is dropped.
#[test]
fn map_exclusive() {
    let cell = AtomicRefCell::new(Pair::new());

    let mut right = ARefMut::map(cell.borrow_mut().unwrap(), |pair| &mut pair.right);
    right.push(4);
    assert_eq!(cell.borrow().err(), Some(error::Borrow::Shared));

    drop(right);
    assert_eq!(cell.borrow().unwrap().right, [2, 3, 4]);
}

/// Asserts that when the callback passed to `ARef::map` unwinds, the shared borrow it was
/// carrying is released, so a writer gets in afterwards.
#[cfg(feature = "std")]
#[test]
fn map_shared_panic() {
    let cell = AtomicRefCell::new(Pair::new());

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let _ = ARef::map(cell.borrow().unwrap(), panicking_map);
    }));
    assert!(result.is_err());

    cell.borrow_mut().unwrap().right.push(4);
    assert_eq!(cell.borrow().unwrap().right, [2, 3, 4]);
}

/// Asserts that when the callback passed to `ARefMut::map` unwinds, the exclusive borrow it was
/// carrying is released, and the write the callback made before panicking is kept.
#[cfg(feature = "std")]
#[test]
fn map_exclusive_panic() {
    let cell = AtomicRefCell::new(Pair::new());

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let _ = ARefMut::map(cell.borrow_mut().unwrap(), panicking_map_mut);
    }));
    assert!(result.is_err());

    assert_eq!(cell.borrow().unwrap().left, [1, 9]);
    assert!(cell.borrow_mut().is_ok());
}

/// Asserts that `ARef::destructure` returns a reference that its split borrow keeps protected:
/// the reference stays readable while the borrow (or a clone of it) is alive, writers are
/// rejected, and the cell unlocks only once both borrows are dropped.
#[test]
fn destructure_shared() {
    let cell = AtomicRefCell::new(vec![5u32]);

    // SAFE: `values` is last used below, before either borrow is dropped.
    let (values, borrow) = unsafe { ARef::destructure(cell.borrow().unwrap()) };
    let borrow_clone = borrow.clone();
    assert_eq!(*values, [5]);
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));

    drop(borrow);
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));

    drop(borrow_clone);
    assert!(cell.borrow_mut().is_ok());
}

/// Asserts the `ViewMut::as_view` handoff: an exclusive guard is destructured, we write through
/// the `&mut`, then read through a shared reborrow of it while a shared reborrow of the exclusive
/// borrow is alive. Dropping that shared reborrow mustn't release the exclusive lock; dropping
/// the exclusive borrow must.
#[test]
fn exclusive_reborrow() {
    let cell = AtomicRefCell::new(vec![1u32]);

    // SAFE: `values` is last used below, before `exclusive` is dropped.
    let (values, exclusive) = unsafe { ARefMut::destructure(cell.borrow_mut().unwrap()) };
    values.push(2);

    let shared = exclusive.shared_reborrow();
    let view: &Vec<u32> = values;
    assert_eq!(*view, [1, 2]);
    drop(shared);

    // We still hold the exclusive borrow, so readers stay out and we can keep writing.
    assert_eq!(cell.borrow().err(), Some(error::Borrow::Shared));
    values.push(3);

    drop(exclusive);
    assert_eq!(*cell.borrow().unwrap(), [1, 2, 3]);
}

/// Asserts that the borrow state's atomics order a write on one thread before a read on
/// another. A scoped thread writes through an exclusive guard while we spin on shared borrows.
/// Once we see the write, the rest of the value must be visible too; Miri reports a data race if
/// releasing the guard doesn't synchronize with the next borrow.
#[cfg(feature = "std")]
#[test]
fn cross_thread_release() {
    let cell = AtomicRefCell::new(vec![0u32]);

    std::thread::scope(|scope| {
        scope.spawn(|| {
            loop {
                if let Ok(mut values) = cell.borrow_mut() {
                    values[0] = 1;
                    values.push(2);
                    break;
                }
                std::hint::spin_loop();
            }
        });

        loop {
            if let Ok(values) = cell.borrow()
                && values[0] == 1
            {
                assert_eq!(*values, [1, 2]);
                break;
            }
            std::hint::spin_loop();
        }
    });
}

/// Returns the thread id generator worlds use by default.
#[cfg(feature = "thread_local")]
fn thread_id() -> alloc::sync::Arc<dyn Fn() -> u64 + Send + Sync> {
    alloc::sync::Arc::new(crate::std_thread_id_generator)
}

/// Asserts the `new_non_send` contract: another thread can share the value but not borrow it
/// exclusively, and an exclusive borrow held by the owner thread still keeps it out.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    let cell = AtomicRefCell::new_non_send(vec![1u32], thread_id());

    std::thread::scope(|scope| {
        scope.spawn(|| {
            assert_eq!(cell.borrow().unwrap()[0], 1);
            assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::WrongThread));
        });
    });

    let mut values = cell.borrow_mut().unwrap();
    values.push(2);
    std::thread::scope(|scope| {
        scope.spawn(|| assert_eq!(cell.borrow().err(), Some(error::Borrow::Shared)));
    });
    drop(values);

    assert_eq!(*cell.borrow().unwrap(), [1, 2]);
}

/// Asserts the `new_non_sync` contract: shared borrows are exclusive, so a second one is
/// rejected even on the same thread. Once the owner's borrow is released, another thread can
/// borrow the `!Sync` value and the owner sees its write afterwards.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync() {
    use core::cell::Cell;

    let cell = AtomicRefCell::new_non_sync(Cell::new(1u32));

    let value = cell.borrow().unwrap();
    assert_eq!(cell.borrow().err(), Some(error::Borrow::Shared));
    std::thread::scope(|scope| {
        scope.spawn(|| assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique)));
    });
    value.set(2);
    drop(value);

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let value = cell.borrow().unwrap();
            value.set(value.get() + 1);
        });
    });

    assert_eq!(cell.borrow().unwrap().get(), 3);
}

/// Asserts the `new_non_send_sync` contract: other threads can't borrow the value at all, while
/// the owner thread can still share it.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync() {
    use alloc::rc::Rc;

    let cell = AtomicRefCell::new_non_send_sync(Rc::new(1u32), thread_id());

    std::thread::scope(|scope| {
        scope.spawn(|| {
            assert_eq!(cell.borrow().err(), Some(error::Borrow::WrongThread));
            assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::WrongThread));
        });
    });

    let first = cell.borrow().unwrap();
    let second = cell.borrow().unwrap();
    assert_eq!(**first + **second, 2);
    assert_eq!(cell.borrow_mut().err(), Some(error::Borrow::Unique));
}
