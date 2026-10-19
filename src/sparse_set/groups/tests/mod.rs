#[cfg(debug_assertions)]
mod invariants;

use super::*;
use alloc::vec;

fn group(ids: &[u64]) -> Arc<[StorageId]> {
    ids.iter().map(|&id| StorageId::Custom(id)).collect()
}

fn range(group: &Arc<[StorageId]>, start: usize, len: usize) -> GroupRange {
    GroupRange {
        group: group.clone(),
        start,
        len,
        shift: 0,
        trust: len,
    }
}

/// Asserts that a storage of 5 components without ranges is entirely ungrouped,
/// with an empty unevaluated region at its end, while its stored boundary stays at 0.
#[test]
fn regions_empty() {
    let groups = Groups::default();

    assert!(!groups.has_ranges());
    assert_eq!(groups.grouped_end(), 0);
    assert_eq!(groups.ungrouped_range(5), 0..5);
    assert_eq!(groups.unevaluated_range(5), 5..5);
    assert_eq!(groups.unevaluated_start, 0);
}

/// Asserts that an empty storage without ranges has empty ungrouped and unevaluated regions.
#[test]
fn regions_empty_storage() {
    let groups = Groups::default();

    assert_eq!(groups.ungrouped_range(0), 0..0);
    assert_eq!(groups.unevaluated_range(0), 0..0);
}

/// Asserts that a storage of 4 components with an attached but empty range and the boundary
/// at 0 is grouped: nothing is ungrouped and every component is unevaluated.
#[test]
fn regions_attached_empty() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 0)],
        unevaluated_start: 0,
    };

    assert!(groups.has_ranges());
    assert_eq!(groups.ungrouped_range(4), 0..0);
    assert_eq!(groups.unevaluated_range(4), 0..4);
}

/// Asserts that a storage of 4 components with an attached but empty range and the boundary
/// at 3 has an ungrouped region `[0, 3)` and an unevaluated region `[3, 4)`.
#[test]
fn regions_attached_empty_evaluated() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 0)],
        unevaluated_start: 3,
    };

    assert_eq!(groups.ungrouped_range(4), 0..3);
    assert_eq!(groups.unevaluated_range(4), 3..4);
}

/// Asserts that with one range of 3 and 2 ungrouped components,
/// the regions of a storage of 8 components are `[0, 3)`, `[3, 5)` and `[5, 8)`.
#[test]
fn regions_one() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 3)],
        unevaluated_start: 5,
    };

    assert!(groups.has_ranges());
    assert_eq!(groups.grouped_end(), 3);
    assert_eq!(groups.ungrouped_range(8), 3..5);
    assert_eq!(groups.unevaluated_range(8), 5..8);
}

/// Asserts that with ranges of 2 and 3 and 1 ungrouped component,
/// the grouped part ends after the second range.
#[test]
fn regions_two() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2), range(&ac, 2, 3)],
        unevaluated_start: 6,
    };

    assert_eq!(groups.grouped_end(), 5);
    assert_eq!(groups.ungrouped_range(9), 5..6);
    assert_eq!(groups.unevaluated_range(9), 6..9);
}

/// Asserts that with one range of 2 and 2 ungrouped components,
/// `region_of` maps indices 0 and 1 to the range, 2 and 3 to the ungrouped region and 4 to the unevaluated region.
#[test]
fn region_of() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 4,
    };

    for index in 0..2 {
        assert!(
            matches!(groups.region_of(index), GroupRegion::Range(group) if Arc::ptr_eq(group, &ab))
        );
    }
    for index in 2..4 {
        assert!(matches!(groups.region_of(index), GroupRegion::Ungrouped));
    }
    assert!(matches!(groups.region_of(4), GroupRegion::Unevaluated));
}

/// Asserts that `region_of` maps every index of a storage without ranges to the ungrouped region.
#[test]
fn region_of_empty() {
    let groups = Groups::default();

    assert!(matches!(groups.region_of(0), GroupRegion::Ungrouped));
    assert!(matches!(groups.region_of(3), GroupRegion::Ungrouped));
}

/// Asserts that `region_of` maps every index of a storage with only an empty range and the boundary
/// at 0 to the unevaluated region, an attached range makes the storage grouped.
#[test]
fn region_of_attached_empty() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 0)],
        unevaluated_start: 0,
    };

    assert!(matches!(groups.region_of(0), GroupRegion::Unevaluated));
    assert!(matches!(groups.region_of(3), GroupRegion::Unevaluated));
}

/// Asserts that `range` finds a group by pointer, among several ranges.
#[test]
fn lookup() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2), range(&ac, 2, 3)],
        unevaluated_start: 5,
    };

    assert_eq!(groups.range(&ac).unwrap().start, 2);
}

/// Asserts that `range` doesn't match an equal group that was allocated separately.
#[test]
fn lookup_by_pointer() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 2,
    };

    let other_ab = group(&[0, 1]);

    assert_eq!(ab, other_ab);
    assert!(groups.range(&other_ab).is_none());
}

/// Asserts that `range_mut` finds a group by pointer and modifies it in place.
#[test]
fn lookup_mut() {
    let ab = group(&[0, 1]);
    let mut groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 2,
    };

    groups.range_mut(&ab).unwrap().trust = 1;

    assert_eq!(groups.ranges[0].trust, 1);
    assert!(groups.range_mut(&group(&[0, 1])).is_none());
}

/// Asserts that `reset` empties and unrotates every range and empties the ungrouped region,
/// but keeps ranges attached.
#[test]
fn reset() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let mut ab_range = range(&ab, 0, 2);
    ab_range.shift = isize::MIN;
    let mut ac_range = range(&ac, 2, 3);
    ac_range.shift = 5;
    let mut groups = Groups {
        storage_id: Some(StorageId::Custom(0)),
        ranges: vec![ab_range, ac_range],
        unevaluated_start: 6,
    };

    groups.reset();

    assert_eq!(groups.ranges.len(), 2);
    for range in &groups.ranges {
        assert_eq!(range.start, 0);
        assert_eq!(range.len, 0);
        assert_eq!(range.shift, 0);
        assert_eq!(range.trust, 0);
    }
    assert!(Arc::ptr_eq(&groups.ranges[0].group, &ab));
    assert!(Arc::ptr_eq(&groups.ranges[1].group, &ac));
    assert_eq!(groups.unevaluated_start, 0);
    assert_eq!(groups.storage_id, Some(StorageId::Custom(0)));
}

/// Asserts that `attach` on a storage without ranges adds an empty range at index 0,
/// leaves the boundary at 0 so every component becomes unevaluated, and records the storage's id.
#[test]
fn attach() {
    let ab = group(&[0, 1]);
    let mut groups = Groups::default();

    groups.attach(ab.clone(), StorageId::Custom(0));

    assert_eq!(groups.ranges.len(), 1);
    assert!(Arc::ptr_eq(&groups.ranges[0].group, &ab));
    assert_eq!(groups.ranges[0].start, 0);
    assert_eq!(groups.ranges[0].len, 0);
    assert_eq!(groups.ranges[0].shift, 0);
    assert_eq!(groups.ranges[0].trust, 0);
    assert_eq!(groups.unevaluated_start, 0);
    assert_eq!(groups.unevaluated_range(3), 0..3);
    assert_eq!(groups.storage_id, Some(StorageId::Custom(0)));
}

/// Asserts that `find_by_value` matches an equal group that was allocated separately.
#[test]
fn find_by_value() {
    let ab = group(&[0, 1]);
    let mut groups = Groups::default();
    groups.attach(ab.clone(), StorageId::Custom(0));

    let other_ab = [StorageId::Custom(0), StorageId::Custom(1)];

    assert!(Arc::ptr_eq(groups.find_by_value(&other_ab).unwrap(), &ab));
}

/// Asserts that `find_by_value` doesn't match a different group.
#[test]
fn find_by_value_other() {
    let mut groups = Groups::default();
    groups.attach(group(&[0, 1]), StorageId::Custom(0));

    let ac = [StorageId::Custom(0), StorageId::Custom(2)];

    assert!(groups.find_by_value(&ac).is_none());
}

fn ring(start: usize, len: usize, shift: isize) -> Ring {
    Ring { start, len, shift }
}

/// Returns the physical index of every logical position of `ring`, in logical order.
fn physical_order(ring: Ring) -> Vec<usize> {
    (0..ring.len).map(|k| ring.physical_of(k)).collect()
}

/// Asserts that an unrotated ring maps logical `k` to `start + k`.
#[test]
fn ring_unrotated() {
    assert_eq!(physical_order(ring(2, 4, 0)), [2, 3, 4, 5]);
}

/// Asserts that a shift of 2 puts logical 0 two slots after `start`,
/// the last two logical positions wrapping to the front: `[L3 L4 L0 L1 L2]`.
#[test]
fn ring_positive() {
    assert_eq!(physical_order(ring(3, 5, 2)), [5, 6, 7, 3, 4]);
}

/// Asserts that a shift of -1 puts logical 0 on the ring's last slot.
#[test]
fn ring_negative() {
    assert_eq!(physical_order(ring(0, 5, -1)), [4, 0, 1, 2, 3]);
}

/// Asserts that shifts larger than the length, in either direction, only count by their remainder.
#[test]
fn ring_large_shift() {
    assert_eq!(physical_order(ring(3, 5, 12)), [5, 6, 7, 3, 4]);
    assert_eq!(physical_order(ring(3, 5, -13)), [5, 6, 7, 3, 4]);
    assert_eq!(physical_order(ring(3, 5, -10)), [3, 4, 5, 6, 7]);
}

/// Asserts that the most extreme shifts give the same remainder as an exact `rem_euclid`.
#[test]
fn ring_extreme_shift() {
    for shift in [isize::MIN, isize::MIN + 1, isize::MAX] {
        for len in 1..=7usize {
            let expected = i128::try_from(shift)
                .unwrap()
                .rem_euclid(i128::try_from(len).unwrap());

            assert_eq!(
                i128::try_from(ring(0, len, shift).wrap_offset()).unwrap(),
                expected,
                "shift {shift}, len {len}"
            );
        }
    }
}

/// Asserts that a ring of length 1 always holds logical 0 at `start`.
#[test]
fn ring_len_one() {
    for shift in [-3, -1, 0, 1, 7] {
        assert_eq!(physical_order(ring(4, 1, shift)), [4]);
    }
}

/// Asserts that, for every small length and shift, logical positions map to every slot once
/// and `logical_of` maps each slot back.
#[test]
fn ring_round_trip() {
    for len in 1..=6 {
        for shift in -13..=13 {
            let ring = ring(3, len, shift);

            let mut slots = physical_order(ring);
            for (logical, &physical) in slots.iter().enumerate() {
                assert_eq!(ring.logical_of(physical), logical, "{ring:?}");
            }

            slots.sort_unstable();
            assert_eq!(slots, (3..3 + len).collect::<Vec<_>>(), "{ring:?}");
        }
    }
}

/// Asserts that an empty ring has no wrap point instead of taking a remainder by zero.
#[test]
#[should_panic(expected = "an empty ring has no wrap point")]
fn ring_empty() {
    let _ = ring(0, 0, 3).wrap_offset();
}

/// Asserts that an empty ring has no logical position.
#[test]
#[should_panic(expected = "logical positions are inside the ring")]
fn ring_empty_position() {
    let _ = ring(0, 0, 0).physical_of(0);
}

/// Asserts that `logical_of` rejects slots outside the ring.
#[test]
#[should_panic(expected = "physical indices are inside the ring")]
fn ring_outside() {
    let _ = ring(2, 3, 1).logical_of(5);
}

/// Asserts that consecutive logical positions are physically consecutive up to the wrap point
/// and up to the ring's end, `[L3 L4 L0 L1 L2]` splitting into `L0..L3` and `L3..L5`.
#[test]
fn ring_contiguous_len() {
    let rotated = ring(3, 5, 2);

    assert_eq!(rotated.contiguous_len(0), 3);
    assert_eq!(rotated.contiguous_len(2), 1);
    assert_eq!(rotated.contiguous_len(3), 2);
    assert_eq!(rotated.contiguous_len(4), 1);
    assert_eq!(ring(3, 5, 0).contiguous_len(1), 4);
}

/// Returns a range over `ring` without a meaningful group or trust.
fn ring_range(ring: Ring) -> GroupRange {
    GroupRange {
        group: group(&[0, 1]),
        start: ring.start,
        len: ring.len,
        shift: ring.shift,
        trust: 0,
    }
}

/// Asserts that moving a range left keeps every logical position,
/// only the physical last component lands on the slot before the old start.
#[test]
fn move_left() {
    for len in 1..=4 {
        for shift in -5..=5 {
            let before = ring(3, len, shift);
            let mut range = ring_range(before);

            range.move_left();

            let expected: Vec<usize> = physical_order(before)
                .into_iter()
                .map(|physical| {
                    if physical == before.end() - 1 {
                        before.start - 1
                    } else {
                        physical
                    }
                })
                .collect();
            assert_eq!(physical_order(range.ring()), expected, "{before:?}");
        }
    }
}

/// Asserts that moving a range right keeps every logical position,
/// only the physical first component lands on the slot after the old end.
#[test]
fn move_right() {
    for len in 1..=4 {
        for shift in -5..=5 {
            let before = ring(3, len, shift);
            let mut range = ring_range(before);

            range.move_right();

            let expected: Vec<usize> = physical_order(before)
                .into_iter()
                .map(|physical| {
                    if physical == before.start {
                        before.end()
                    } else {
                        physical
                    }
                })
                .collect();
            assert_eq!(physical_order(range.ring()), expected, "{before:?}");
        }
    }
}

/// Asserts that moving an empty range only moves its start.
#[test]
fn move_empty() {
    let mut range = ring_range(ring(3, 0, -2));

    range.move_left();
    assert_eq!(range.start, 2);
    range.move_right();
    range.move_right();
    assert_eq!(range.start, 4);
}

/// Asserts that resizing picks the requested wrap point by moving the shift by the difference
/// between the new and current remainders, instead of normalizing it.
#[test]
fn resize() {
    let mut range = ring_range(ring(0, 5, 17));

    range.resize(4, 1);

    assert_eq!(range.ring().wrap_offset(), 1);
    // 17 has a remainder of 1 modulo 4, it's already right.
    assert_eq!(range.shift, 17);

    range.resize(6, 4);
    // 17 has a remainder of 5 modulo 6, one less gives 4.
    assert_eq!((range.ring().wrap_offset(), range.shift), (4, 16));

    range.resize(3, 2);
    // 16 has a remainder of 1 modulo 3, one more gives 2.
    assert_eq!((range.ring().wrap_offset(), range.shift), (2, 17));
}

/// Asserts that resizing to 0 keeps the shift, an empty range has no wrap point.
#[test]
fn resize_empty() {
    let mut range = ring_range(ring(0, 2, -9));

    range.resize(0, 0);

    assert_eq!((range.len, range.shift), (0, -9));
}

/// Asserts that an empty range with a large retained shift grows like any other.
#[test]
fn resize_from_empty() {
    let mut range = ring_range(ring(2, 0, isize::MIN + 1));

    range.resize(1, 0);
    assert_eq!(physical_order(range.ring()), [2]);
    range.resize(2, 1);
    assert_eq!(physical_order(range.ring()), [3, 2]);
}

/// Asserts that a reset storage grows its ranges again from an unrotated ring, whatever shift
/// they had before.
#[test]
fn reset_reuse() {
    let ab = group(&[0, 1]);
    let mut ab_range = range(&ab, 0, 3);
    ab_range.shift = -7;
    let mut groups = Groups {
        storage_id: None,
        ranges: vec![ab_range],
        unevaluated_start: 3,
    };

    groups.reset();
    groups.ranges[0].resize(1, 0);
    groups.unevaluated_start = 1;

    assert_eq!(groups.ranges[0].shift, 0);
    assert_eq!(physical_order(groups.ranges[0].ring()), [0]);
    #[cfg(debug_assertions)]
    groups.assert_invariants(1);
}

/// Asserts that empty ranges between non-empty ones hold nothing,
/// every slot is classified in the range actually covering it.
#[test]
fn region_of_empty_interior() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let ad = group(&[0, 3]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2), range(&ac, 2, 0), range(&ad, 2, 3)],
        unevaluated_start: 6,
    };

    assert_eq!(groups.range_index_of(1), Some(0));
    assert_eq!(groups.range_index_of(2), Some(2));
    assert_eq!(groups.range_index_of(4), Some(2));
    assert_eq!(groups.range_index_of(5), None);
    assert!(matches!(groups.region_of(2), GroupRegion::Range(group) if Arc::ptr_eq(group, &ad)));
    assert!(matches!(groups.region_of(5), GroupRegion::Ungrouped));
    assert_eq!(groups.range_index(&ac), Some(1));
    assert_eq!(groups.range_index(&group(&[0, 2])), None);
}

/// Asserts that rotations don't change which region holds a slot.
#[test]
fn region_of_rotated() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let mut ab_range = range(&ab, 0, 2);
    ab_range.shift = 1;
    let mut ac_range = range(&ac, 2, 3);
    ac_range.shift = -4;
    let groups = Groups {
        storage_id: None,
        ranges: vec![ab_range, ac_range],
        unevaluated_start: 5,
    };

    assert_eq!(groups.range_index_of(0), Some(0));
    assert_eq!(groups.range_index_of(1), Some(0));
    assert_eq!(groups.range_index_of(4), Some(1));
    assert!(matches!(groups.region_of(5), GroupRegion::Unevaluated));
}
