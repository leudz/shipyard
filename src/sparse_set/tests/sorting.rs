//! Sorting only reorders a storage's ungrouped region, whatever its layout and tracking.

use super::*;
use core::sync::atomic::AtomicU32;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct InsertedI32(i32);

impl Component for InsertedI32 {
    type Tracking = crate::track::Insertion;
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct ModifiedI32(i32);

impl Component for ModifiedI32 {
    type Tracking = crate::track::Modification;
}

/// Holds minus its entity's index, so its `Ord` sorts entities by descending index.
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Negated(i64);

impl Component for Negated {
    type Tracking = crate::track::Untracked;
}

/// Component counting the comparisons it took part in, through a shared reference.
struct Touched(u64, AtomicU32);

impl Component for Touched {
    type Tracking = crate::track::Untracked;
}

/// Reads the entity index a test component was built with.
trait Value {
    fn value(&self) -> u64;
}

impl Value for I32 {
    fn value(&self) -> u64 {
        u64::try_from(self.0).unwrap()
    }
}

impl Value for TrackedI32 {
    fn value(&self) -> u64 {
        u64::try_from(self.0).unwrap()
    }
}

impl Value for InsertedI32 {
    fn value(&self) -> u64 {
        u64::try_from(self.0).unwrap()
    }
}

impl Value for ModifiedI32 {
    fn value(&self) -> u64 {
        u64::try_from(self.0).unwrap()
    }
}

impl Value for Negated {
    fn value(&self) -> u64 {
        u64::try_from(-self.0).unwrap()
    }
}

impl Value for Counted {
    fn value(&self) -> u64 {
        self.0
    }
}

impl Value for Touched {
    fn value(&self) -> u64 {
        self.0
    }
}

fn int(index: u64) -> i32 {
    i32::try_from(index).unwrap()
}

/// Sort key ordering components by descending value.
fn descending(value: u64) -> u64 {
    u64::MAX - value
}

/// Builds a storage with rotated ranges of 3 and 4, an empty range between them,
/// 5 ungrouped components at `7..12` and 3 unevaluated ones at `12..15`.\
/// The first range only trusts its first position.
fn tails<T: Component>(make: impl Fn(u64) -> T) -> SparseSet<T> {
    let mut sparse_set = storage_with_tails(&[(3, 1), (0, 0), (4, -6)], 5, 3, make);
    sparse_set.groups.ranges[0].trust = 1;

    sparse_set
}

/// Builds a storage without any range holding `len` components.
fn group_free(len: usize) -> SparseSet<I32> {
    // Without range the stored boundary stays at 0, every component is implicitly ungrouped.
    ints_with_tails(&[], 0, len)
}

/// Returns the values of the storage's components, in physical order.
fn values<T: Value + Component>(sparse_set: &SparseSet<T>) -> Vec<u64> {
    sparse_set.data.iter().map(Value::value).collect()
}

/// Sorts `sparse_set` by the `key` of its components' values.\
/// Returns the number of comparisons.
fn sort_by_key<T: Value + Component>(
    sparse_set: &mut SparseSet<T>,
    key: impl Fn(u64) -> u64,
) -> usize {
    let mut comparisons = 0;
    sparse_set.sort_unstable_by(|a, b| {
        comparisons += 1;
        key(a.value()).cmp(&key(b.value()))
    });

    comparisons
}

/// What sorting has to keep: every row outside the sortable region with its sparse entry,
/// the entities inside it and the group metadata.
#[derive(PartialEq, Debug)]
struct Kept {
    /// `(index, entity, sparse entry, value, insertion, modification)` of the rows outside.
    rows: Vec<(usize, EntityId, EntityId, u64, Option<u64>, Option<u64>)>,
    /// Sorted entities of the sortable region.
    sortable: Vec<EntityId>,
    /// `(group, start, len, shift, trust)` of each range.
    ranges: Vec<(*const StorageId, usize, usize, isize, usize)>,
    unevaluated_start: usize,
}

fn kept<T: Value + Component>(sparse_set: &SparseSet<T>) -> Kept {
    let sortable = sparse_set.groups.ungrouped_range(sparse_set.len());

    let rows = (0..sparse_set.len())
        .filter(|index| !sortable.contains(index))
        .map(|index| {
            let entity = sparse_set.dense[index];

            (
                index,
                entity,
                sparse_set.sparse.get(entity).unwrap(),
                sparse_set.data[index].value(),
                sparse_set.insertion_data.get(index).map(|t| t.get()),
                sparse_set.modification_data.get(index).map(|t| t.get()),
            )
        })
        .collect();

    let mut sortable = sparse_set.dense[sortable].to_vec();
    sortable.sort_unstable();

    let ranges = sparse_set
        .groups
        .ranges
        .iter()
        .map(|range| {
            (
                Arc::as_ptr(&range.group).cast::<StorageId>(),
                range.start,
                range.len,
                range.shift,
                range.trust,
            )
        })
        .collect();

    Kept {
        rows,
        sortable,
        ranges,
        unevaluated_start: sparse_set.groups.unevaluated_start,
    }
}

/// Panics unless `sparse_set` kept what `before` recorded, every row still holds its entity's
/// component and timestamps, and the sortable region is ordered by `key`.
fn assert_sorted<T: Value + Component>(
    sparse_set: &SparseSet<T>,
    before: &Kept,
    key: impl Fn(u64) -> u64,
) {
    assert_rows(sparse_set, |component| Some(component.value()));
    assert_eq!(&kept(sparse_set), before);

    let sortable = sparse_set.groups.ungrouped_range(sparse_set.len());
    let keys: Vec<u64> = sparse_set.data[sortable]
        .iter()
        .map(|component| key(component.value()))
        .collect();
    assert!(keys.is_sorted(), "{keys:?}");
}

/// Asserts that sorting a storage without any range sorts all its components,
/// its stored boundary staying at 0.
#[test]
fn group_free_all() {
    let mut sparse_set = group_free(6);
    let before = kept(&sparse_set);

    sort_by_key(&mut sparse_set, descending);

    assert_sorted(&sparse_set, &before, descending);
    assert_eq!(values(&sparse_set), [5, 4, 3, 2, 1, 0]);
    assert_eq!(sparse_set.groups.unevaluated_start, 0);
}

/// Asserts that sorting an empty storage or a storage of 1 component never calls the comparator.
#[test]
fn group_free_small() {
    for len in 0..2 {
        let mut sparse_set = group_free(len);

        assert_eq!(sort_by_key(&mut sparse_set, descending), 0);
        assert_eq!(sparse_set.len(), len);
    }
}

/// Asserts that sorting a storage with rotated ranges, a partially trusted one, an empty one
/// between them and unevaluated components only reorders its ungrouped region.
#[test]
fn tails_only() {
    let mut sparse_set = tails(|index| I32(int(index)));
    let before = kept(&sparse_set);

    sort_by_key(&mut sparse_set, descending);

    assert_sorted(&sparse_set, &before, descending);
    assert_eq!(values(&sparse_set)[7..12], [11, 10, 9, 8, 7]);
}

/// Asserts that a storage with only an empty range attached sorts its ungrouped components,
/// not its unevaluated ones.
#[test]
fn attached_empty() {
    let mut sparse_set = ints_with_tails(&[(0, 0)], 4, 2);
    let before = kept(&sparse_set);

    sort_by_key(&mut sparse_set, descending);

    assert_sorted(&sparse_set, &before, descending);
    assert_eq!(values(&sparse_set), [3, 2, 1, 0, 4, 5]);
}

/// Asserts that a storage with an empty range and only unevaluated components, like right after
/// its first group is attached, doesn't sort anything and never calls the comparator.
#[test]
fn attached_empty_unevaluated() {
    let mut sparse_set = ints_with_tails(&[(0, 0)], 0, 4);
    let before = kept(&sparse_set);

    assert_eq!(sort_by_key(&mut sparse_set, descending), 0);

    assert_eq!(kept(&sparse_set), before);
    assert_eq!(values(&sparse_set), [0, 1, 2, 3]);
}

/// Asserts that a grouped storage without ungrouped component never calls the comparator.
#[test]
fn no_sortable() {
    let mut sparse_set = ints_with_tails(&[(3, 1)], 0, 3);
    let before = kept(&sparse_set);

    assert_eq!(sort_by_key(&mut sparse_set, descending), 0);

    assert_eq!(kept(&sparse_set), before);
}

/// Asserts that a grouped storage with a single ungrouped component never calls the comparator.
#[test]
fn one_sortable() {
    let mut sparse_set = ints_with_tails(&[(3, 1)], 1, 3);
    let before = kept(&sparse_set);

    assert_eq!(sort_by_key(&mut sparse_set, descending), 0);

    assert_eq!(kept(&sparse_set), before);
    assert_eq!(values(&sparse_set), [0, 1, 2, 3, 4, 5, 6]);
}

/// Asserts that sorting an already sorted ungrouped region, made only of fixed points,
/// doesn't move anything.
#[test]
fn ascending() {
    let mut sparse_set = ints_with_tails(&[(2, 1)], 5, 2);
    let before = kept(&sparse_set);
    let dense = sparse_set.dense.clone();

    sort_by_key(&mut sparse_set, |value| value);

    assert_sorted(&sparse_set, &before, |value| value);
    assert_eq!(sparse_set.dense, dense);
}

/// Asserts that reversing an odd ungrouped region, two-row cycles around a fixed point,
/// moves every row to its mirror position.
#[test]
fn descending_odd() {
    let mut sparse_set = ints_with_tails(&[(2, 1)], 5, 2);
    let before = kept(&sparse_set);

    sort_by_key(&mut sparse_set, descending);

    assert_sorted(&sparse_set, &before, descending);
    assert_eq!(values(&sparse_set)[2..7], [6, 5, 4, 3, 2]);
}

/// Asserts that rotating the ungrouped region by one, a single cycle through all its rows,
/// moves every row.
#[test]
fn long_cycle() {
    let mut sparse_set = ints_with_tails(&[(2, 1)], 6, 2);
    let before = kept(&sparse_set);
    // Rows `2..8` get keys `1, 2, 3, 4, 5, 0`.
    let key = |value| (value - 2 + 1) % 6;

    sort_by_key(&mut sparse_set, key);

    assert_sorted(&sparse_set, &before, key);
    assert_eq!(values(&sparse_set)[2..8], [7, 2, 3, 4, 5, 6]);
}

/// Asserts that a permutation made of two independent three-row cycles and a fixed point
/// moves each row of the cycles and leaves the fixed point.
#[test]
fn several_cycles() {
    let mut sparse_set = ints_with_tails(&[(1, 0)], 7, 1);
    let before = kept(&sparse_set);
    // Rows `1..8` move to the relative positions their key gives.
    let targets = [1, 2, 0, 4, 5, 3, 6];
    let key = |value: u64| targets[usize::try_from(value - 1).unwrap()];

    sort_by_key(&mut sparse_set, key);

    assert_sorted(&sparse_set, &before, key);
    assert_eq!(values(&sparse_set)[1..8], [3, 1, 2, 6, 4, 5, 7]);
}

/// Asserts that sorting with many equal keys orders the keys, whatever the order of equal ones.
#[test]
fn equal_keys() {
    let mut sparse_set = group_free(8);
    let before = kept(&sparse_set);
    let key = |value| value % 2;

    sort_by_key(&mut sparse_set, key);

    assert_sorted(&sparse_set, &before, key);
}

/// Asserts that `sort_unstable` uses the component's `Ord` and only sorts the ungrouped region.
#[test]
fn sort_unstable_ord() {
    let mut sparse_set = storage_with_tails(&[(2, 1)], 4, 1, |index| {
        Negated(-i64::try_from(index).unwrap())
    });
    let before = kept(&sparse_set);

    sparse_set.sort_unstable();

    assert_sorted(&sparse_set, &before, descending);
    assert_eq!(values(&sparse_set), [0, 1, 5, 4, 3, 2, 6]);
}

/// Asserts that sorting entities spread over several sparse pages, with nonzero generations
/// and indices unrelated to their rows, points each sparse entry at its new row
/// and keeps its generation.
#[test]
fn pages() {
    let mut sparse_set = SparseSet::new();
    for row in 0..6u64 {
        let index = (5 - row) * BUCKET_SIZE as u64 + row;
        let generation = u16::try_from(row).unwrap() + 1;

        sparse_set
            .insert(
                EntityId::new_from_parts(index, generation),
                I32(int(index)),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
    }
    let group: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(1)]);
    sparse_set.groups.ranges.push(GroupRange {
        group,
        start: 0,
        len: 2,
        shift: 0,
        trust: 2,
    });
    sparse_set.groups.unevaluated_start = 6;
    let before = kept(&sparse_set);

    sort_by_key(&mut sparse_set, |value| value);

    assert_sorted(&sparse_set, &before, |value| value);
    for (row, &entity) in sparse_set.dense.iter().enumerate() {
        assert_eq!(
            sparse_set.sparse.get(entity),
            Some(EntityId::new_from_index_and_gen(
                row as u64,
                entity.generation()
            ))
        );
    }
}

/// Sorts a storage built by `tails` with `make` by descending value, and panics unless only
/// its ungrouped region was reordered, each row keeping the timestamps it tracks.
fn sort_tracked<T: Value + Component>(make: impl Fn(u64) -> T) -> SparseSet<T> {
    let mut sparse_set = tails(make);
    let before = kept(&sparse_set);

    sort_by_key(&mut sparse_set, descending);

    assert_sorted(&sparse_set, &before, descending);

    sparse_set
}

/// Asserts that sorting a storage tracking insertion keeps each insertion timestamp with its component.
#[test]
fn tracking_insertion() {
    sort_tracked(|index| InsertedI32(int(index)));
}

/// Asserts that sorting a storage tracking modification keeps each modification timestamp
/// with its component, without marking anything modified.
#[test]
fn tracking_modification() {
    sort_tracked(|index| ModifiedI32(int(index)));
}

/// Asserts that sorting a storage tracking everything keeps each timestamp with its component
/// and doesn't log any removal or deletion.
#[test]
fn tracking_all() {
    let sparse_set = sort_tracked(|index| TrackedI32(int(index)));

    assert!(sparse_set.removal_data.is_empty());
    assert!(sparse_set.deletion_data.is_empty());
}

/// Asserts that sorting doesn't drop any component nor call the insertion or removal callback.
#[test]
fn side_effects() {
    let drops = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut sparse_set = tails(|index| Counted(index, drops.clone()));
    let insertion_calls = calls.clone();
    sparse_set.on_insertion(move |_, _| {
        insertion_calls.fetch_add(1, AtomicOrdering::Relaxed);
    });
    let removal_calls = calls.clone();
    sparse_set.on_removal(move |_, _| {
        removal_calls.fetch_add(1, AtomicOrdering::Relaxed);
    });
    let before = kept(&sparse_set);

    sort_by_key(&mut sparse_set, descending);

    assert_sorted(&sparse_set, &before, descending);
    assert_eq!(drops.load(AtomicOrdering::Relaxed), 0);
    assert_eq!(calls.load(AtomicOrdering::Relaxed), 0);
}

/// Sorts `sparse_set` by the `key` of its components' values, panicking instead of
/// comparing for the `panic_at`th time.\
/// Returns `true` when the comparator panicked.
#[cfg(feature = "std")]
fn sort_panicking<T: Value + Component>(
    sparse_set: &mut SparseSet<T>,
    key: impl Fn(u64) -> u64,
    panic_at: usize,
) -> bool {
    let mut comparisons = 0;

    std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        sparse_set.sort_unstable_by(|a, b| {
            if comparisons == panic_at {
                panic!("comparator panicked");
            }
            comparisons += 1;

            key(a.value()).cmp(&key(b.value()))
        });
    }))
    .is_err()
}

/// Builds a tracked storage with a range of 3 and 24 ungrouped components at `3..27`,
/// enough for the standard sort to go past its small-slice path, and 2 unevaluated ones.
#[cfg(feature = "std")]
fn panic_fixture() -> SparseSet<TrackedI32> {
    storage_with_tails(&[(3, 1)], 24, 2, |index| TrackedI32(int(index)))
}

/// Shuffles the ungrouped rows of `panic_fixture`.
#[cfg(feature = "std")]
fn shuffled(value: u64) -> u64 {
    (value * 7) % 24
}

/// Asserts that whenever the comparator panics, the storage is left valid: every entity keeps its
/// component and timestamps with a correct sparse entry, and the rows outside the ungrouped
/// region and the group metadata are kept.\
/// Some panics happen after the sort moved ids, so the repair actually has work to do.
#[cfg(feature = "std")]
#[test]
fn panic_each_comparison() {
    let comparisons = sort_by_key(&mut panic_fixture(), shuffled);
    let mut reordered = 0;

    for panic_at in 0..comparisons {
        let mut sparse_set = panic_fixture();
        let dense = sparse_set.dense.clone();
        let before = kept(&sparse_set);

        assert!(sort_panicking(&mut sparse_set, shuffled, panic_at));

        assert_rows(&sparse_set, |component| Some(component.value()));
        assert_eq!(kept(&sparse_set), before, "panic at {panic_at}");
        if sparse_set.dense != dense {
            reordered += 1;
        }
    }

    assert!(reordered > 0);
}

/// Asserts that a storage whose sort panicked can still be read, removed from and sorted again.
#[cfg(feature = "std")]
#[test]
fn panic_recovery() {
    let comparisons = sort_by_key(&mut panic_fixture(), shuffled);
    let mut sparse_set = panic_fixture();
    assert!(sort_panicking(&mut sparse_set, shuffled, comparisons / 2));

    assert_eq!(
        sparse_set.private_get(EntityId::new(10)),
        Some(&TrackedI32(10))
    );
    assert_eq!(
        sparse_set.dyn_remove(EntityId::new(10), TrackingTimestamp::new(0)),
        Some(TrackedI32(10))
    );
    assert_eq!(
        sparse_set.dyn_remove(EntityId::new(1), TrackingTimestamp::new(0)),
        Some(TrackedI32(1))
    );
    assert_rows(&sparse_set, |component| Some(component.value()));

    let before = kept(&sparse_set);
    sort_by_key(&mut sparse_set, shuffled);

    assert_sorted(&sparse_set, &before, shuffled);
}

/// Asserts that what the comparator changed through interior mutability before panicking stays,
/// on the components it compared.
#[cfg(feature = "std")]
#[test]
fn panic_interior_mutability() {
    let mut sparse_set =
        storage_with_tails(&[(3, 1)], 24, 2, |index| Touched(index, AtomicU32::new(0)));
    let before = kept(&sparse_set);
    let mut comparisons = 0;

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        sparse_set.sort_unstable_by(|a, b| {
            if comparisons == 10 {
                panic!("comparator panicked");
            }
            comparisons += 1;
            a.1.fetch_add(1, AtomicOrdering::Relaxed);
            b.1.fetch_add(1, AtomicOrdering::Relaxed);

            shuffled(a.0).cmp(&shuffled(b.0))
        });
    }));

    assert!(result.is_err());
    assert_rows(&sparse_set, |component| Some(component.value()));
    assert_eq!(kept(&sparse_set), before);
    let touches: u32 = sparse_set
        .data
        .iter()
        .map(|component| component.1.load(AtomicOrdering::Relaxed))
        .sum();
    assert_eq!(touches, 20);
}
