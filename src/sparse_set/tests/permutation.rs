//! `repair_rows` moves components and timestamps to follow reordered ids, inside its domain only.

use super::*;

/// Component forcing a large alignment, so a swap with the wrong layout would show.
#[repr(align(64))]
struct Aligned(u64);

impl Component for Aligned {
    type Tracking = crate::track::Untracked;
}

/// Puts the ids of `domain` in the order `order` gives, then repairs the domain's rows.
///
/// `order[k]` is the row whose id goes to `domain.start + k`.
fn permute<T: Component>(sparse_set: &mut SparseSet<T>, domain: Range<usize>, order: &[usize]) {
    let mut rows = order.to_vec();
    rows.sort_unstable();
    assert!(
        rows.into_iter().eq(domain.clone()),
        "{order:?} isn't a permutation"
    );

    let ids: Vec<EntityId> = order.iter().map(|&row| sparse_set.dense[row]).collect();
    sparse_set.dense[domain.clone()].copy_from_slice(&ids);

    // SAFE: `domain` holds the same ids it did, we just checked `order` is a permutation of it.
    // They come from the storage so their pages are allocated, and nothing touched the sparse
    // array or the rows yet: each entry still holds its entity's own row, one of `domain`'s.
    // `dense`, `data` and the enabled timestamp vectors have the storage's length.
    unsafe {
        repair_rows(
            domain,
            &sparse_set.dense,
            &mut sparse_set.sparse,
            &mut sparse_set.data,
            sparse_set
                .is_tracking_insertion
                .then_some(&mut sparse_set.insertion_data[..]),
            sparse_set
                .is_tracking_modification
                .then_some(&mut sparse_set.modification_data[..]),
        );
    }
}

/// Returns the entity index of each row, in physical order.
fn indices<T: Component>(sparse_set: &SparseSet<T>) -> Vec<u64> {
    sparse_set
        .dense
        .iter()
        .map(|entity| entity.index())
        .collect()
}

/// Asserts that a domain left in its order keeps every row and sparse entry.
#[test]
fn fixed_points() {
    let mut sparse_set = ints_with_tails(&[(3, 1)], 4, 0);

    permute(&mut sparse_set, 0..7, &[0, 1, 2, 3, 4, 5, 6]);

    assert_rows(&sparse_set, int_value);
    assert_eq!(indices(&sparse_set), [0, 1, 2, 3, 4, 5, 6]);
}

/// Asserts that two three-row cycles and a fixed point in between each bring their rows
/// to the ids' new positions.
#[test]
fn cycles() {
    let mut sparse_set = ints_with_tails(&[], 0, 7);

    permute(&mut sparse_set, 0..7, &[1, 2, 0, 3, 6, 4, 5]);

    assert_rows(&sparse_set, int_value);
    assert_eq!(indices(&sparse_set), [1, 2, 0, 3, 6, 4, 5]);
}

/// Asserts that a single cycle through every row of the domain moves all of them.
#[test]
fn long_cycle() {
    let mut sparse_set = ints_with_tails(&[], 0, 6);

    permute(&mut sparse_set, 0..6, &[5, 0, 1, 2, 3, 4]);

    assert_rows(&sparse_set, int_value);
    assert_eq!(indices(&sparse_set), [5, 0, 1, 2, 3, 4]);
}

/// Asserts that a domain spanning a rotated range, the ungrouped region and the unevaluated one
/// repairs cycles crossing their boundaries, with fixed points inside, and never touches
/// the rows around it.
#[test]
fn across_regions() {
    let mut sparse_set = ints_with_tails(&[(2, 0), (4, -1)], 3, 3);

    // The domain `1..11` starts in the first range and ends in the unevaluated region.
    // Rows 2, 5 and 7 are fixed points. The cycle 1, 9, 3, 6, 4 goes through both ranges,
    // the ungrouped region and the unevaluated one, 8 and 10 swap across the last boundary.
    permute(&mut sparse_set, 1..11, &[9, 2, 6, 1, 5, 4, 7, 10, 3, 8]);

    assert_rows(&sparse_set, int_value);
    assert_eq!(indices(&sparse_set), [0, 9, 2, 6, 1, 5, 4, 7, 10, 3, 8, 11]);
}

/// Asserts that each timestamp a tracked storage holds moves with its component.
#[test]
fn tracked() {
    let mut sparse_set = storage_with_tails(&[(3, 1)], 3, 0, |index| {
        TrackedI32(i32::try_from(index).unwrap())
    });

    permute(&mut sparse_set, 0..6, &[3, 5, 4, 0, 2, 1]);

    assert_rows(&sparse_set, |component| {
        Some(u64::try_from(component.0).unwrap())
    });
    assert_eq!(indices(&sparse_set), [3, 5, 4, 0, 2, 1]);
}

/// Asserts that a storage of zero-sized components still points each entity at its new row.
#[test]
fn zero_sized() {
    let mut sparse_set = storage_with_tails(&[], 0, 5, |_| Zst);

    permute(&mut sparse_set, 0..5, &[4, 3, 0, 1, 2]);

    assert_rows(&sparse_set, |_| None);
    assert_eq!(indices(&sparse_set), [4, 3, 0, 1, 2]);
}

/// Asserts that over-aligned components follow their entity.
#[test]
fn over_aligned() {
    let mut sparse_set = storage_with_tails(&[], 0, 5, Aligned);

    permute(&mut sparse_set, 0..5, &[2, 4, 1, 0, 3]);

    assert_rows(&sparse_set, |component| Some(component.0));
    assert_eq!(indices(&sparse_set), [2, 4, 1, 0, 3]);
}

/// Asserts that the repair neither drops nor duplicates a component:
/// none is dropped during it and each one is dropped once with the storage.
#[test]
fn drops() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut sparse_set = storage_with_tails(&[], 0, 6, |index| Counted(index, drops.clone()));

    permute(&mut sparse_set, 0..6, &[1, 0, 4, 5, 3, 2]);

    assert_rows(&sparse_set, |component| Some(component.0));
    assert_eq!(drops.load(AtomicOrdering::Relaxed), 0);

    drop(sparse_set);
    assert_eq!(drops.load(AtomicOrdering::Relaxed), 6);
}

/// Builds a tracked storage of 6 components, 3 of them in a rotated range.
fn tracked_storage() -> SparseSet<TrackedI32> {
    storage_with_tails(&[(3, 1)], 3, 0, |index| {
        TrackedI32(i32::try_from(index).unwrap())
    })
}

/// Returns what each row holds besides its id: the sparse entry of the entity with the row's index,
/// the component, and its insertion and modification timestamps.
fn rows(sparse_set: &SparseSet<TrackedI32>) -> Vec<(Option<EntityId>, i32, u64, u64)> {
    (0..sparse_set.len())
        .map(|row| {
            (
                sparse_set.sparse.get(EntityId::new(row as u64)),
                sparse_set.data[row].0,
                sparse_set.insertion_data[row].get(),
                sparse_set.modification_data[row].get(),
            )
        })
        .collect()
}

/// Reverses the ids of a `tracked_storage` through the erased `reorder_ids`, sorting them by their
/// frozen sparse entry while reading the groups from the same borrow, and checks nothing else moved.
/// Then repairs every row through the erased `repair_rows` and checks every association.
///
/// `inner` reaches the `SparseSet` behind `storage`.
fn reverse_erased<S: Storage>(mut storage: S, inner: impl Fn(&S) -> &SparseSet<TrackedI32>) {
    let before = rows(inner(&storage));
    let len = inner(&storage).len();

    let erased: &mut dyn Storage = &mut storage;
    // SAFE: we only sort the ids, and repair every row before any other use of the storage.
    let parts = unsafe { erased.reorder_ids() }.unwrap();
    parts.dense.sort_unstable_by_key(|&entity| {
        core::cmp::Reverse(parts.sparse.get(entity).unwrap().uindex())
    });
    assert_eq!(parts.groups.ranges.len(), 1);
    assert_eq!(parts.groups.ungrouped_range(len), 3..6);

    assert_eq!(indices(inner(&storage)), [5, 4, 3, 2, 1, 0]);
    assert_eq!(rows(inner(&storage)), before);

    let erased: &mut dyn Storage = &mut storage;
    // SAFE: `0..len` is the whole storage. Its ids were only permuted and nothing repaired them yet,
    // so their sparse entries hold their original rows, which are all of `0..len`.
    unsafe { erased.repair_rows(0..len) };

    assert_rows(inner(&storage), |component| {
        Some(u64::try_from(component.0).unwrap())
    });
    assert_eq!(indices(inner(&storage)), [5, 4, 3, 2, 1, 0]);
}

/// Asserts that reordering a `SparseSet`'s ids through the erased storage leaves its sparse array,
/// components and timestamps in place, until the erased repair brings each one to its entity.
#[test]
fn erased() {
    reverse_erased(tracked_storage(), |storage| storage);
}

/// Asserts that a `NonSend` storage forwards reordering and repair to its `SparseSet`,
/// with the same results as [`erased`].
#[cfg(feature = "thread_local")]
#[test]
fn erased_non_send() {
    reverse_erased(crate::borrow::NonSend(tracked_storage()), |storage| {
        &storage.0
    });
}

/// Asserts that a `NonSync` storage forwards reordering and repair to its `SparseSet`,
/// with the same results as [`erased`].
#[cfg(feature = "thread_local")]
#[test]
fn erased_non_sync() {
    reverse_erased(crate::borrow::NonSync(tracked_storage()), |storage| {
        &storage.0
    });
}

/// Asserts that a `NonSendSync` storage forwards reordering and repair to its `SparseSet`,
/// with the same results as [`erased`].
#[cfg(feature = "thread_local")]
#[test]
fn erased_non_send_sync() {
    reverse_erased(crate::borrow::NonSendSync(tracked_storage()), |storage| {
        &storage.0
    });
}

/// Storage that isn't a `SparseSet`.
struct Custom;

impl Storage for Custom {}

/// Asserts that a custom storage that doesn't support grouping has no ids to reorder.
#[test]
fn erased_custom() {
    let mut custom = Custom;
    let erased: &mut dyn Storage = &mut custom;

    // SAFE: the default implementation returns `None`, there are no ids to reorder or repair.
    assert!(unsafe { erased.reorder_ids() }.is_none());
}
