//! Focused Miri scenarios for type-erased storage boxes.
//!
//! `SBox` turns a typed storage into a raw `*mut AtomicRefCell<dyn Storage>` and frees it when
//! dropped. These tests reach the storage back through that pointer and count drops, so a lost
//! or doubled free shows up as a wrong count (or a Miri error).

#[cfg(all(feature = "std", feature = "thread_local"))]
use crate::error::Borrow;
use crate::storage::{SBox, SBoxBuilder, Storage};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Heap-owning custom storage that counts how many times it's dropped.
struct Counted {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
}

impl Counted {
    fn new(drops: &Arc<AtomicUsize>) -> Counted {
        Counted {
            values: vec![1, 2],
            drops: drops.clone(),
        }
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

impl Storage for Counted {}

/// Finishes `builder`, the way `AllStorages` does when it stores a cloned storage.
#[cfg(not(feature = "thread_local"))]
fn build(builder: SBoxBuilder) -> SBox {
    builder.build()
}

/// Finishes `builder`, the way `AllStorages` does when it stores a cloned storage.
#[cfg(feature = "thread_local")]
fn build(builder: SBoxBuilder) -> SBox {
    // Without `std` there is no thread id, the tests running without it stay on a single thread.
    #[cfg(feature = "std")]
    let thread_id = Arc::new(crate::std_thread_id_generator);
    #[cfg(not(feature = "std"))]
    let thread_id = Arc::new(|| 0);

    // SAFE: the builder holds a `Counted`, which only owns a `Vec<u32>` and an
    // `Arc<AtomicUsize>`, so it really is both `Send` and `Sync`.
    unsafe { builder.build(thread_id, true, true) }
}

/// Asserts that a storage boxed by `SBox::new` can be written and read back through the erased
/// pointer, and that dropping the `SBox` drops the storage exactly once.
#[test]
fn sbox_drop() {
    let drops = Arc::new(AtomicUsize::new(0));
    let sbox = SBox::new(Counted::new(&drops));

    // SAFE: `sbox.0` comes from `Box::into_raw` in `SBox::new` and is only freed when `sbox` is
    // dropped, which happens after our last use of `cell`.
    let cell = unsafe { &*sbox.0 };

    let mut storage = cell.borrow_mut().unwrap();
    let counted = storage.any_mut().downcast_mut::<Counted>().unwrap();
    counted.values.push(3);
    drop(storage);

    let storage = cell.borrow().unwrap();
    let counted = storage.any().downcast_ref::<Counted>().unwrap();
    assert_eq!(counted.values, [1, 2, 3]);
    drop(storage);

    assert_eq!(drops.load(Ordering::Relaxed), 0);
    drop(sbox);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Asserts that dropping an `SBoxBuilder` without building it frees its storage exactly once.
#[test]
fn builder_unbuilt() {
    let drops = Arc::new(AtomicUsize::new(0));
    let builder = SBoxBuilder::new(Counted::new(&drops));

    drop(builder);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Asserts that building an `SBoxBuilder` hands its allocation over to the `SBox`: the storage
/// is still reachable through the erased pointer, and it's dropped exactly once, when the `SBox`
/// is dropped.
#[test]
fn builder_built() {
    let drops = Arc::new(AtomicUsize::new(0));
    let sbox = build(SBoxBuilder::new(Counted::new(&drops)));
    assert_eq!(drops.load(Ordering::Relaxed), 0);

    // SAFE: `build` returned the builder's `SBox`, whose pointer comes from `Box::into_raw` and
    // is only freed when `sbox` is dropped, after our last use of `cell`.
    let cell = unsafe { &*sbox.0 };

    let storage = cell.borrow().unwrap();
    let counted = storage.any().downcast_ref::<Counted>().unwrap();
    assert_eq!(counted.values, [1, 2]);
    drop(storage);

    drop(sbox);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Heap-owning custom storage that is `Sync` but not `Send`, like a type holding a `MutexGuard`.
#[cfg(all(feature = "std", feature = "thread_local"))]
struct NonSendStorage {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
    _non_send: core::marker::PhantomData<std::sync::MutexGuard<'static, ()>>,
}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Drop for NonSendStorage {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Storage for NonSendStorage {}

/// Heap-owning custom storage that is neither `Send` nor `Sync`, like a type holding an `Rc`.
#[cfg(all(feature = "std", feature = "thread_local"))]
struct NonSendSyncStorage {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
    _non_send_sync: core::marker::PhantomData<alloc::rc::Rc<()>>,
}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Drop for NonSendSyncStorage {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Storage for NonSendSyncStorage {}

/// Asserts that finishing a builder for a storage that isn't `Send` records it on the erased
/// cell: another thread can still share the storage but not borrow it exclusively, the owner
/// thread can, and the storage is dropped exactly once.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn builder_non_send() {
    let drops = Arc::new(AtomicUsize::new(0));
    let builder = SBoxBuilder::new(NonSendStorage {
        values: vec![1, 2],
        drops: drops.clone(),
        _non_send: core::marker::PhantomData,
    });
    // SAFE: `NonSendStorage` holds a `MutexGuard` marker, it really is `Sync` and not `Send`.
    let sbox = unsafe { builder.build(Arc::new(crate::std_thread_id_generator), false, true) };

    // SAFE: `build` returned the builder's `SBox`, whose pointer comes from `Box::into_raw` and
    // is only freed when `sbox` is dropped, after our last use of `cell`.
    let cell = unsafe { &*sbox.0 };
    assert!(!cell.is_send());
    assert!(cell.is_sync());

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let storage = cell.borrow().unwrap();
            let counted = storage.any().downcast_ref::<NonSendStorage>().unwrap();
            assert_eq!(counted.values, [1, 2]);
            assert_eq!(cell.borrow_mut().err(), Some(Borrow::WrongThread));
        });
    });

    let mut storage = cell.borrow_mut().unwrap();
    let counted = storage.any_mut().downcast_mut::<NonSendStorage>().unwrap();
    counted.values.push(3);
    drop(storage);

    drop(sbox);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Asserts that finishing a builder for a storage that is neither `Send` nor `Sync` records it on
/// the erased cell: another thread can't borrow the storage at all, the owner thread can, and the
/// storage is dropped exactly once.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn builder_non_send_sync() {
    let drops = Arc::new(AtomicUsize::new(0));
    let builder = SBoxBuilder::new(NonSendSyncStorage {
        values: vec![1, 2],
        drops: drops.clone(),
        _non_send_sync: core::marker::PhantomData,
    });
    // SAFE: `NonSendSyncStorage` holds an `Rc` marker, it really is neither `Send` nor `Sync`.
    let sbox = unsafe { builder.build(Arc::new(crate::std_thread_id_generator), false, false) };

    // SAFE: Same as in `builder_non_send`.
    let cell = unsafe { &*sbox.0 };
    assert!(!cell.is_send());
    assert!(!cell.is_sync());

    std::thread::scope(|scope| {
        scope.spawn(|| {
            assert_eq!(cell.borrow().err(), Some(Borrow::WrongThread));
            assert_eq!(cell.borrow_mut().err(), Some(Borrow::WrongThread));
        });
    });

    let mut storage = cell.borrow_mut().unwrap();
    let counted = storage
        .any_mut()
        .downcast_mut::<NonSendSyncStorage>()
        .unwrap();
    counted.values.push(3);
    drop(storage);

    drop(sbox);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}
