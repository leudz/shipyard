//! Removal from grouped storages.

use super::*;

/// Removes `entity`, then panics unless the storage lost the component with `entity`'s index and
/// nothing else changed: other ranges keep their logical order and trust, the range it left only
/// trusts its unchanged prefix, every row is consistent and every group stays attached.
///
/// Returns what the removal returned.
fn assert_removal<T: Component>(
    sparse_set: &mut SparseSet<T>,
    entity: EntityId,
    value: impl Fn(&T) -> Option<u64>,
) -> Option<T> {
    let before = Before::of(sparse_set);
    let len = sparse_set.len();

    let removed = sparse_set.dyn_remove(entity, TrackingTimestamp::new(0));

    let context = alloc::format!("{:?}, removing {entity:?}", before.layout);
    if let Some(index) = removed.as_ref().and_then(&value) {
        assert_eq!(index, entity.index(), "{context}");
    }
    assert_eq!(sparse_set.len(), len - 1, "{context}");
    assert_extracted(
        &before,
        layout(sparse_set),
        sparse_set,
        entity.index(),
        &context,
    );
    assert_rows(sparse_set, value);

    removed
}

/// Removes the generation 0 entity at `index`, checking it like [`assert_removal`].
fn remove_int(sparse_set: &mut SparseSet<I32>, index: u64) {
    let removed = assert_removal(sparse_set, EntityId::new(index), int_value);

    assert_eq!(removed, Some(I32(index as i32)));
}

/// Asserts that every physical position of a small first ring, at any rotation, can be removed
/// while the later rings, rotated or empty, keep their order and trust.
#[test]
fn first() {
    for len in 1..=5 {
        for shift in shifts() {
            for entity in 0..len as u64 {
                let mut sparse_set =
                    ints_with_tails(&[(len, shift), (2, 1), (0, 0), (3, -2)], 2, 2);

                remove_int(&mut sparse_set, entity);
            }
        }
    }
}

/// Asserts that every physical position of a ring between other rings can be removed,
/// the earlier ring staying untouched.
#[test]
fn middle() {
    for len in 1..=5 {
        for shift in shifts() {
            for entity in 2..2 + len as u64 {
                let mut sparse_set =
                    ints_with_tails(&[(2, 1), (len, shift), (0, 0), (3, -2)], 2, 2);

                remove_int(&mut sparse_set, entity);
            }
        }
    }
}

/// Asserts that every physical position of the last ring can be removed.
#[test]
fn last() {
    for len in 1..=5 {
        for shift in shifts() {
            for entity in 2..2 + len as u64 {
                let mut sparse_set = ints_with_tails(&[(2, 1), (0, 0), (len, shift)], 2, 2);

                remove_int(&mut sparse_set, entity);
            }
        }
    }
}

/// Asserts that removing from a ring with a damaged prefix, of any length, lowers its trust to
/// exactly what stays unchanged, and that a later ring keeps its own partial trust.
#[test]
fn partial_trust() {
    for trust in 0..=4 {
        for shift in shifts() {
            for entity in 0..4 {
                let mut sparse_set = ints_with_tails(&[(4, shift), (3, 1)], 1, 1);
                sparse_set.groups.ranges[0].trust = trust;
                sparse_set.groups.ranges[1].trust = 1;

                remove_int(&mut sparse_set, entity);
            }
        }
    }
}

/// Asserts that when logical 0 is in the ring's physical last slot, the shrunk ring starts at its
/// first slot and doesn't trust anything anymore.
#[test]
fn wrap_at_new_len() {
    let mut sparse_set = ints_with_tails(&[(4, 3), (2, 1)], 1, 1);

    remove_int(&mut sparse_set, 1);

    assert_eq!(sparse_set.groups.ranges[0].trust, 0);
}

/// Asserts that removing the physical last component only trusts the positions before it.
#[test]
fn physical_last() {
    let mut sparse_set = ints_with_tails(&[(4, 1), (2, 1)], 1, 1);

    remove_int(&mut sparse_set, 3);

    assert_eq!(sparse_set.groups.ranges[0].trust, 2);
}

/// Asserts that emptying a rotated singleton ring leaves it empty and attached, with its shift.
#[test]
fn singleton() {
    let mut sparse_set = ints_with_tails(&[(1, -4), (2, 1)], 1, 1);

    remove_int(&mut sparse_set, 0);

    assert_eq!(sparse_set.groups.ranges[0].len, 0);
    assert_eq!(sparse_set.groups.ranges[0].shift, -4);
}

/// Asserts that empty ranges between and after the later ranges move left with them.
#[test]
fn empty_ranges() {
    let mut sparse_set = ints_with_tails(&[(3, 1), (0, 0), (2, -1), (0, 0)], 1, 1);

    remove_int(&mut sparse_set, 1);

    let starts: Vec<usize> = sparse_set
        .groups
        .ranges
        .iter()
        .map(|range| range.start)
        .collect();
    assert_eq!(starts, [0, 2, 2, 4]);
}

/// Asserts that removing from a range, from the ungrouped and from the unevaluated region works
/// with each tail empty or not, including a fully grouped storage.
#[test]
fn tails() {
    for ungrouped_len in [0, 2] {
        for unevaluated_len in [0, 2] {
            let entity_count = 7 + ungrouped_len + unevaluated_len;

            for entity in 0..entity_count as u64 {
                let mut sparse_set =
                    ints_with_tails(&[(3, 1), (4, -2)], ungrouped_len, unevaluated_len);

                remove_int(&mut sparse_set, entity);
            }
        }
    }
}

/// Asserts that removing every component one after the other, without any regroup,
/// leaves an empty storage with all its ranges attached.
#[test]
fn until_empty() {
    let mut sparse_set = ints_with_tails(&[(3, 1), (0, 0), (4, -2), (2, 5)], 2, 2);

    for entity in [5, 0, 12, 7, 3, 10, 1, 8, 2, 11, 6, 4, 9] {
        remove_int(&mut sparse_set, entity);
    }

    assert!(sparse_set.is_empty());
    assert_eq!(sparse_set.groups.ranges.len(), 4);
    assert_eq!(sparse_set.groups.unevaluated_start, 0);
}

/// Asserts that insertion and modification timestamps follow the components moved by removals.
#[test]
fn tracked() {
    let mut sparse_set = storage_with_tails(&[(4, 3), (3, -1)], 1, 1, |index| {
        TrackedI32(i32::try_from(index).unwrap())
    });
    let value = |component: &TrackedI32| Some(u64::try_from(component.0).unwrap());

    for entity in [1, 5, 8, 0, 4] {
        assert!(assert_removal(&mut sparse_set, EntityId::new(entity), value).is_some());
    }
}

/// Asserts that removals hand back each removed component without dropping any other,
/// and that dropping the storage then drops each remaining one once.
#[test]
fn drops() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut sparse_set = storage_with_tails(&[(4, 3), (0, 0), (3, -1)], 1, 1, |index| {
        Counted(index, drops.clone())
    });
    let value = |component: &Counted| Some(component.0);

    for (dropped, entity) in [2, 6, 7, 0].into_iter().enumerate() {
        let removed = assert_removal(&mut sparse_set, EntityId::new(entity), value);
        assert_eq!(drops.load(AtomicOrdering::Relaxed), dropped);

        drop(removed);
    }

    assert_eq!(drops.load(AtomicOrdering::Relaxed), 4);
    drop(sparse_set);
    assert_eq!(drops.load(AtomicOrdering::Relaxed), 9);
}

/// Asserts that zero-sized components leave grouped storages like any other.
#[test]
fn zst() {
    let mut sparse_set = storage_with_tails(&[(3, 1), (0, 0), (4, -2)], 2, 2, |_| Zst);

    for entity in [1, 5, 8, 10, 3] {
        assert!(assert_removal(&mut sparse_set, EntityId::new(entity), |_| None).is_some());
    }
}

/// Builds a storage with entities 0 to 3 in a single rotated, fully trusted range,
/// entity 1 at generation 2 and the others at generation 0.
///
/// Returns it with a counter of its removal callback calls.
fn generations() -> (SparseSet<I32>, Arc<AtomicUsize>) {
    let mut sparse_set = SparseSet::new();
    for index in 0..4 {
        let generation = if index == 1 { 2 } else { 0 };

        sparse_set
            .insert(
                EntityId::new_from_parts(index, generation),
                I32(index as i32),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
    }

    sparse_set.groups.ranges.push(GroupRange {
        group: Arc::from([StorageId::Custom(0), StorageId::Custom(1)]),
        start: 0,
        len: 4,
        shift: 1,
        trust: 4,
    });
    sparse_set.groups.unevaluated_start = 4;

    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = calls.clone();
    sparse_set.on_removal(move |_, _| {
        callback_calls.fetch_add(1, AtomicOrdering::Relaxed);
    });

    (sparse_set, calls)
}

/// Asserts that removing with the stored generation hands the component back
/// and calls the removal callback.
#[test]
fn generation_matching() {
    let (mut sparse_set, calls) = generations();

    let removed = assert_removal(&mut sparse_set, EntityId::new_from_parts(1, 2), int_value);

    assert_eq!(removed, Some(I32(1)));
    assert_eq!(calls.load(AtomicOrdering::Relaxed), 1);
}

/// Asserts that removing with a newer generation removes the stale component from its range
/// but doesn't hand it back nor call the removal callback.
#[test]
fn generation_newer() {
    let (mut sparse_set, calls) = generations();

    let removed = assert_removal(&mut sparse_set, EntityId::new_from_parts(1, 3), int_value);

    assert_eq!(removed, None);
    assert_eq!(sparse_set.index_of(EntityId::new_from_parts(1, 2)), None);
    assert_eq!(calls.load(AtomicOrdering::Relaxed), 0);
}

/// Asserts that removing with an older generation doesn't change anything.
#[test]
fn generation_older() {
    let (mut sparse_set, calls) = generations();
    let before = layout(&sparse_set);

    let removed = sparse_set.dyn_remove(EntityId::new_from_parts(1, 1), TrackingTimestamp::new(0));

    assert_eq!(removed, None);
    assert_eq!(layout(&sparse_set), before);
    assert_eq!(
        sparse_set.private_get(EntityId::new_from_parts(1, 2)),
        Some(&I32(1))
    );
    assert_eq!(calls.load(AtomicOrdering::Relaxed), 0);
}

/// Asserts that `retain` on a grouped storage visits every component exactly once
/// and only removes the rejected ones.
#[test]
fn retain() {
    let mut sparse_set = ints_with_tails(&[(3, 1), (0, 0), (4, -2), (2, 5)], 2, 2);
    let mut visited = Vec::new();

    sparse_set.private_retain(TrackingTimestamp::new(0), |entity, component| {
        assert_eq!(component.0 as u64, entity.index());
        visited.push(entity.index());

        entity.index() % 3 != 0
    });

    visited.sort_unstable();
    assert_eq!(visited, (0..13).collect::<Vec<u64>>());
    let mut kept: Vec<u64> = sparse_set
        .dense
        .iter()
        .map(|entity| entity.index())
        .collect();
    kept.sort_unstable();
    assert_eq!(kept, [1, 2, 4, 5, 7, 8, 10, 11]);
    assert_rows(&sparse_set, int_value);
}

/// Asserts that `retain_mut` on a grouped storage visits every component exactly once,
/// and that the changes it makes stay with the components it keeps.
#[test]
fn retain_mut() {
    let mut sparse_set = ints_with_tails(&[(3, 1), (0, 0), (4, -2), (2, 5)], 2, 2);
    let mut visited = Vec::new();

    sparse_set.private_retain_mut(TrackingTimestamp::new(0), |entity, mut component| {
        assert_eq!(component.0 as u64, entity.index());
        visited.push(entity.index());
        component.0 += 100;

        entity.index() % 2 == 0
    });

    visited.sort_unstable();
    assert_eq!(visited, (0..13).collect::<Vec<u64>>());
    let mut kept: Vec<u64> = sparse_set
        .dense
        .iter()
        .map(|entity| entity.index())
        .collect();
    kept.sort_unstable();
    assert_eq!(kept, [0, 2, 4, 6, 8, 10, 12]);
    assert_rows(&sparse_set, |component| Some(component.0 as u64 - 100));
}
