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
