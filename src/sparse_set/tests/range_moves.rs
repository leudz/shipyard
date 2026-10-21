use super::*;

/// Range layout shared by the tests: `(len, shift)` pairs, rotated rings and an empty one.
///
/// The first range isn't rotated, so its physical last component is also its logical last.
const RANGES: [(usize, isize); 5] = [(3, 0), (2, 1), (0, 0), (4, -3), (1, 5)];

/// Builds a storage holding entities `0..12` in physical order, each with `make(index)`.
///
/// `RANGES` are placed back to back and fully trusted, the last 2 components are ungrouped.\
/// Tracked entities are inserted at timestamp `index` and modified at `100 + index`.
fn storage<T: Component>(make: impl Fn(u64) -> T) -> SparseSet<T> {
    storage_with(&RANGES, make)
}

/// Same as `storage`, with `ranges` as `(len, shift)` pairs instead of `RANGES`.
fn storage_with<T: Component>(ranges: &[(usize, isize)], make: impl Fn(u64) -> T) -> SparseSet<T> {
    storage_with_tails(ranges, 2, 0, make)
}

fn ints() -> SparseSet<I32> {
    storage(|index| I32(i32::try_from(index).unwrap()))
}

/// Returns the entity indices of each range, in logical order.
fn logical_order<T: Component>(sparse_set: &SparseSet<T>) -> Vec<Vec<u64>> {
    sparse_set
        .groups
        .ranges
        .iter()
        .map(|range| {
            let ring = range.ring();

            (0..ring.len)
                .map(|logical| sparse_set.dense[ring.physical_of(logical)].index())
                .collect()
        })
        .collect()
}

fn starts<T: Component>(sparse_set: &SparseSet<T>) -> Vec<usize> {
    sparse_set
        .groups
        .ranges
        .iter()
        .map(|range| range.start)
        .collect()
}

fn trusts<T: Component>(sparse_set: &SparseSet<T>) -> Vec<usize> {
    sparse_set
        .groups
        .ranges
        .iter()
        .map(|range| range.trust)
        .collect()
}

/// Takes entity 2, the first range's last component, out of it and moves the later ranges left.
fn shrink_first<T: Component>(sparse_set: &mut SparseSet<T>) {
    sparse_set.groups.ranges[0].len = 2;
    sparse_set.groups.ranges[0].trust = 2;

    sparse_set.move_ranges_left(1);
}

/// Asserts that moving ranges left keeps each later range's logical order,
/// rotated or empty, and moves each of them one slot down.
#[test]
fn left() {
    let mut sparse_set = ints();
    let before = logical_order(&sparse_set);

    shrink_first(&mut sparse_set);

    assert_eq!(logical_order(&sparse_set)[1..], before[1..]);
    assert_eq!(starts(&sparse_set), [0, 2, 4, 4, 8]);
    assert_rows(&sparse_set, int_value);
}

/// Asserts that moving ranges left carries the component before them past all of them,
/// leaving it as the first ungrouped component.
#[test]
fn left_carried() {
    let mut sparse_set = ints();

    shrink_first(&mut sparse_set);

    let grouped_end = sparse_set.groups.grouped_end();
    assert_eq!(grouped_end, 9);
    assert_eq!(sparse_set.dense[grouped_end], EntityId::new(2));
}

/// Asserts that moving ranges left leaves their trust alone, partial or full.
#[test]
fn left_trust() {
    let mut sparse_set = ints();
    sparse_set.groups.ranges[1].trust = 1;
    sparse_set.groups.ranges[3].trust = 0;

    shrink_first(&mut sparse_set);

    assert_eq!(trusts(&sparse_set), [2, 1, 0, 0, 1]);
}

/// Asserts that moving only empty ranges moves their starts and nothing else.
#[test]
fn empty_ranges() {
    let mut sparse_set = ints();
    sparse_set.groups.ranges.truncate(3);
    sparse_set.groups.ranges[0].len = 2;
    sparse_set.groups.ranges[0].trust = 2;
    sparse_set.groups.ranges[1].len = 0;
    sparse_set.groups.ranges[1].trust = 0;
    sparse_set.groups.ranges[2].start = 3;
    let dense = sparse_set.dense.clone();

    sparse_set.move_ranges_left(1);

    assert_eq!(starts(&sparse_set), [0, 2, 2]);
    assert_eq!(sparse_set.dense, dense);
}

/// Asserts that insertion and modification timestamps follow their components.
#[test]
fn tracked() {
    let mut sparse_set = storage(|index| TrackedI32(i32::try_from(index).unwrap()));

    shrink_first(&mut sparse_set);

    assert_rows(&sparse_set, |component| {
        Some(u64::try_from(component.0).unwrap())
    });
}

/// Asserts that moving ranges never drops a component,
/// and that dropping the storage then drops each of them once.
#[test]
fn drops() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut sparse_set = storage(|index| Counted(index, drops.clone()));

    shrink_first(&mut sparse_set);
    assert_rows(&sparse_set, |component| Some(component.0));

    assert_eq!(drops.load(AtomicOrdering::Relaxed), 0);
    drop(sparse_set);
    assert_eq!(drops.load(AtomicOrdering::Relaxed), 12);
}
