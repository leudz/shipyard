use super::*;

#[derive(PartialEq, Eq, Debug)]
struct ModifiedI32(i32);

impl Component for ModifiedI32 {
    type Tracking = crate::track::Modification;
}

/// Builds a storage with entities 0, 1 and 2 at generation 0, inserted at timestamp 1.
///
/// Entity 1 is then overwritten at timestamp 2, so its modification timestamp isn't the origin.
fn modified_storage<T: Component>(make: impl Fn(i32) -> T) -> SparseSet<T> {
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set
            .insert(
                EntityId::new_from_parts(index, 0),
                make(index as i32),
                TrackingTimestamp::new(1),
            )
            .assert_inserted();
    }

    let _ = sparse_set.insert(
        EntityId::new_from_parts(1, 0),
        make(1),
        TrackingTimestamp::new(2),
    );

    sparse_set
}

/// Places every component of `sparse_set` in a single fully trusted group range.
fn group_all<T: Component>(sparse_set: &mut SparseSet<T>) {
    sparse_set.groups.ranges.push(GroupRange {
        group: Arc::from([StorageId::Custom(0), StorageId::Custom(1)]),
        start: 0,
        len: sparse_set.len(),
        shift: 0,
        trust: sparse_set.len(),
    });
    sparse_set.groups.unevaluated_start = sparse_set.len();
}

/// Returns `entity`'s insertion and modification timestamps, `None` for the untracked ones.
fn timestamps<T: Component>(
    sparse_set: &SparseSet<T>,
    entity: EntityId,
) -> (Option<u64>, Option<u64>) {
    let index = sparse_set.index_of(entity).unwrap();

    let insertion = sparse_set
        .is_tracking_insertion
        .then(|| sparse_set.insertion_data[index].get());
    let modification = sparse_set
        .is_tracking_modification
        .then(|| sparse_set.modification_data[index].get());

    (insertion, modification)
}

/// Inserts `make(10)` for entity 1 at generation 1 and timestamp 3, then panics unless it replaced
/// generation 0's component as a fresh insertion, with an origin modification timestamp.
fn assert_replaced<T: Component + PartialEq + fmt::Debug>(
    mut sparse_set: SparseSet<T>,
    make: impl Fn(i32) -> T,
) {
    let old = EntityId::new_from_parts(1, 0);
    let new = EntityId::new_from_parts(1, 1);

    let result = sparse_set.insert(new, make(10), TrackingTimestamp::new(3));

    assert!(matches!(result, InsertionResult::OtherComponentOverride));
    assert_eq!(sparse_set.index_of(old), None);
    assert_eq!(sparse_set.private_get(new), Some(&make(10)));

    let insertion = sparse_set.is_tracking_insertion.then_some(3);
    assert_eq!(timestamps(&sparse_set, new), (insertion, Some(0)));

    // The other components were only inserted.
    let insertion = sparse_set.is_tracking_insertion.then_some(1);
    for index in [0, 2] {
        let entity = EntityId::new_from_parts(index, 0);
        assert_eq!(timestamps(&sparse_set, entity), (insertion, Some(0)));
    }
}

/// Asserts that replacing a modified component of an older generation, in a storage only
/// tracking modification, gives the new entity an origin modification timestamp.
#[test]
fn modification() {
    let sparse_set = modified_storage(ModifiedI32);

    assert_replaced(sparse_set, ModifiedI32);
}

/// Asserts that replacing a modified component of an older generation, in a storage tracking
/// insertion and modification, marks the new entity inserted and not modified.
#[test]
fn insertion_and_modification() {
    let sparse_set = modified_storage(TrackedI32);

    assert_replaced(sparse_set, TrackedI32);
}

/// Asserts that replacing a modified component of an older generation inside a group range,
/// in a storage only tracking modification, gives the new entity an origin modification timestamp.
#[test]
fn grouped_modification() {
    let mut sparse_set = modified_storage(ModifiedI32);
    group_all(&mut sparse_set);

    assert_replaced(sparse_set, ModifiedI32);
}

/// Asserts that replacing a modified component of an older generation inside a group range,
/// in a storage tracking insertion and modification, marks the new entity inserted and not modified.
#[test]
fn grouped_insertion_and_modification() {
    let mut sparse_set = modified_storage(TrackedI32);
    group_all(&mut sparse_set);

    assert_replaced(sparse_set, TrackedI32);
}

/// Asserts that overwriting a component with the same generation still marks it modified,
/// and keeps its insertion timestamp.
#[test]
fn overwrite() {
    let mut sparse_set = modified_storage(TrackedI32);
    let entity = EntityId::new_from_parts(1, 0);

    let result = sparse_set.insert(entity, TrackedI32(10), TrackingTimestamp::new(3));

    assert!(matches!(
        result,
        InsertionResult::ComponentOverride(TrackedI32(1))
    ));
    assert_eq!(sparse_set.private_get(entity), Some(&TrackedI32(10)));
    assert_eq!(timestamps(&sparse_set, entity), (Some(1), Some(3)));
}

/// Asserts that inserting with a generation older than the stored one is rejected,
/// leaving the stored component and its timestamps unchanged.
#[test]
fn older_generation() {
    let mut sparse_set = modified_storage(TrackedI32);
    let entity = EntityId::new_from_parts(1, 2);
    let _ = sparse_set.insert(entity, TrackedI32(10), TrackingTimestamp::new(3));
    let _ = sparse_set.insert(entity, TrackedI32(11), TrackingTimestamp::new(4));

    let result = sparse_set.insert(
        EntityId::new_from_parts(1, 1),
        TrackedI32(12),
        TrackingTimestamp::new(5),
    );

    assert!(matches!(result, InsertionResult::NotInserted));
    assert_eq!(sparse_set.private_get(entity), Some(&TrackedI32(11)));
    assert_eq!(timestamps(&sparse_set, entity), (Some(3), Some(4)));
}

fn tracked_value(component: &TrackedI32) -> Option<u64> {
    Some(u64::try_from(component.0).unwrap())
}

fn tracked_storage(
    ranges: &[(usize, isize)],
    ungrouped_len: usize,
    unevaluated_len: usize,
) -> SparseSet<TrackedI32> {
    storage_with_tails(ranges, ungrouped_len, unevaluated_len, |index| {
        TrackedI32(i32::try_from(index).unwrap())
    })
}

/// Inserts `TrackedI32(1000 + index)` for entity `index` at generation 1 and timestamp 50, over
/// its generation 0 component, then panics unless the new component is an unevaluated fresh
/// insertion while the old one left like with a removal.
///
/// The other ranges keep their logical order and trust, every other row keeps its component and
/// timestamps, and nothing is tracked as removed or deleted.
fn assert_replacement(sparse_set: &mut SparseSet<TrackedI32>, index: u64) {
    let before = Before::of(sparse_set);
    let len = sparse_set.len();
    let old = EntityId::new(index);
    let new = EntityId::new_from_parts(index, 1);
    let value = TrackedI32(1000 + i32::try_from(index).unwrap());

    let result = sparse_set.insert(new, value, TrackingTimestamp::new(50));

    let context = alloc::format!("{:?}, replacing {index}", before.layout);
    assert!(
        matches!(result, InsertionResult::OtherComponentOverride),
        "{context}"
    );
    assert_eq!(sparse_set.len(), len, "{context}");
    assert_eq!(sparse_set.index_of(old), None, "{context}");

    let new_index = sparse_set.index_of(new).unwrap();
    assert!(
        new_index >= sparse_set.groups.unevaluated_start,
        "{context}"
    );

    // Apart from the new unevaluated component, the storage looks like the old one was removed.
    let mut after = layout(sparse_set);
    after
        .unevaluated
        .retain(|&unevaluated| unevaluated != index);
    assert_extracted(&before, after, sparse_set, index, &context);
    for row in 0..len {
        if sparse_set.dense[row].generation() == 0 {
            assert_row(sparse_set, row, &tracked_value);
        } else {
            assert_replaced_row(sparse_set, row);
        }
    }
    #[cfg(debug_assertions)]
    sparse_set.groups.assert_invariants(len);

    assert!(sparse_set.removal_data.is_empty(), "{context}");
    assert!(sparse_set.deletion_data.is_empty(), "{context}");
}

/// Panics unless the row at `row` holds what `assert_replacement` inserted for its entity.
fn assert_replaced_row(sparse_set: &SparseSet<TrackedI32>, row: usize) {
    let entity = sparse_set.dense[row];

    assert_eq!(entity.generation(), 1);
    assert_eq!(sparse_set.index_of(entity), Some(row));
    assert_eq!(sparse_set.data[row].0 - 1000, entity.index() as i32);
    assert_eq!(timestamps(sparse_set, entity), (Some(50), Some(0)));
}

/// Asserts that replacing every physical position of a small ring, at any rotation, moves the new
/// component to the unevaluated region while the later rings, rotated or empty, keep their order.
#[test]
fn rotated() {
    for len in 1..=4 {
        for shift in shifts() {
            for entity in 0..len as u64 {
                let mut sparse_set =
                    tracked_storage(&[(len, shift), (2, 1), (0, 0), (3, -2)], 1, 1);

                assert_replacement(&mut sparse_set, entity);
            }
        }
    }
}

/// Asserts that replacing a component of a range, of the ungrouped and of the unevaluated region
/// works with each tail empty or not, including a fully grouped storage.
#[test]
fn tails() {
    for ungrouped_len in [0, 2] {
        for unevaluated_len in [0, 2] {
            let entity_count = 7 + ungrouped_len + unevaluated_len;

            for entity in 0..entity_count as u64 {
                let mut sparse_set =
                    tracked_storage(&[(3, 1), (4, -2)], ungrouped_len, unevaluated_len);

                assert_replacement(&mut sparse_set, entity);
            }
        }
    }
}

/// Asserts that replacing an unevaluated component keeps it in place, without moving any row.
#[test]
fn unevaluated_in_place() {
    let mut sparse_set = tracked_storage(&[(3, 1), (4, -2)], 1, 2);
    let mut dense = sparse_set.dense.clone();

    assert_replacement(&mut sparse_set, 9);

    dense[9] = EntityId::new_from_parts(9, 1);
    assert_eq!(sparse_set.dense, dense);
}

/// Asserts that replacing components one after the other, before any regroup, keeps applying
/// the same local damage, including replacing an entity that was already replaced.
#[test]
fn repeated() {
    let mut sparse_set = tracked_storage(&[(3, 1), (0, 0), (4, -2), (2, 5)], 1, 1);

    for entity in [5, 0, 10, 7] {
        assert_replacement(&mut sparse_set, entity);
    }

    // Generation 1 of entity 0 is unevaluated, a newer generation replaces it in place.
    let index = sparse_set.index_of(EntityId::new_from_parts(0, 1)).unwrap();
    let result = sparse_set.insert(
        EntityId::new_from_parts(0, 2),
        TrackedI32(2000),
        TrackingTimestamp::new(60),
    );
    assert!(matches!(result, InsertionResult::OtherComponentOverride));
    assert_eq!(
        sparse_set.index_of(EntityId::new_from_parts(0, 2)),
        Some(index)
    );
}

/// Asserts that replacing a grouped component calls the insertion callback once with the new
/// entity and never calls the removal callback.
#[test]
fn callbacks() {
    let mut sparse_set = tracked_storage(&[(3, 1), (4, -2)], 1, 1);
    let insertions = Arc::new(AtomicUsize::new(0));
    let removals = Arc::new(AtomicUsize::new(0));
    let new = EntityId::new_from_parts(4, 1);

    let counter = insertions.clone();
    sparse_set.on_insertion(move |entity, _| {
        assert_eq!(entity, new);
        counter.fetch_add(1, AtomicOrdering::Relaxed);
    });
    let counter = removals.clone();
    sparse_set.on_removal(move |_, _| {
        counter.fetch_add(1, AtomicOrdering::Relaxed);
    });

    let result = sparse_set.insert(new, TrackedI32(1004), TrackingTimestamp::new(50));

    assert!(matches!(result, InsertionResult::OtherComponentOverride));
    assert_eq!(insertions.load(AtomicOrdering::Relaxed), 1);
    assert_eq!(removals.load(AtomicOrdering::Relaxed), 0);
}

/// Asserts that inserting with an older generation than a grouped component moves nothing.
#[test]
fn grouped_older_generation() {
    let mut sparse_set = tracked_storage(&[(3, 1), (4, -2)], 1, 1);
    assert_replacement(&mut sparse_set, 2);
    // Generation 1 of entity 2 is unevaluated now, we put it back in its range at the end.
    let group = sparse_set.groups.ranges[0].group.clone();
    sparse_set.private_move_to(EntityId::new_from_parts(2, 1), GroupRegion::Range(&group));
    let before = layout(&sparse_set);
    let dense = sparse_set.dense.clone();

    let result = sparse_set.insert(EntityId::new(2), TrackedI32(0), TrackingTimestamp::new(60));

    assert!(matches!(result, InsertionResult::NotInserted));
    assert_eq!(layout(&sparse_set), before);
    assert_eq!(sparse_set.dense, dense);
}
