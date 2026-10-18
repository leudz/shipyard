//! Focused Miri scenarios for type-erased storage boxes.
//!
//! `SBox` turns a typed storage into a raw `*mut AtomicRefCell<dyn Storage>` and frees it when
//! dropped. These tests reach the storage back through that pointer and count drops, so a lost
//! or doubled free shows up as a wrong count (or a Miri error).

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
    // SAFE: the builder holds a `Counted`, which only owns a `Vec<u32>` and an
    // `Arc<AtomicUsize>`, so it really is both `Send` and `Sync`.
    unsafe { builder.build(Arc::new(crate::std_thread_id_generator), true, true) }
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
