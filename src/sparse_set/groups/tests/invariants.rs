use super::*;

/// Asserts that several ranges, rotated or empty, are accepted.
#[test]
fn several_ranges() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let ad = group(&[0, 3]);
    let mut ab_range = range(&ab, 0, 2);
    ab_range.shift = 5;
    let mut ad_range = range(&ad, 2, 3);
    ad_range.shift = -2;
    let groups = Groups {
        storage_id: None,
        ranges: vec![ab_range, range(&ac, 2, 0), ad_range],
        unevaluated_start: 5,
    };

    groups.assert_invariants(5);
}

/// Asserts that a range overlapping the previous one is rejected.
#[test]
#[should_panic(expected = "group ranges must tile the start of the storage in order")]
fn tiling_overlap() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2), range(&ac, 1, 3)],
        unevaluated_start: 4,
    };

    groups.assert_invariants(4);
}

/// Asserts that a first range that doesn't start at index 0 is rejected.
#[test]
#[should_panic(expected = "group ranges must tile the start of the storage in order")]
fn tiling_first_start() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 1, 2)],
        unevaluated_start: 3,
    };

    groups.assert_invariants(3);
}

/// Asserts that a gap between two ranges is rejected.
#[test]
#[should_panic(expected = "group ranges must tile the start of the storage in order")]
fn tiling_gap() {
    let ab = group(&[0, 1]);
    let ac = group(&[0, 2]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2), range(&ac, 3, 3)],
        unevaluated_start: 6,
    };

    groups.assert_invariants(6);
}

/// Asserts that a range trusted over its whole length is accepted.
#[test]
fn trust() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 2,
    };

    groups.assert_invariants(2);
}

/// Asserts that a range trusted past its length is rejected.
#[test]
#[should_panic(expected = "a range's trust cannot exceed its length")]
fn trust_past_len() {
    let ab = group(&[0, 1]);
    let mut ab_range = range(&ab, 0, 2);
    ab_range.trust = 3;
    let groups = Groups {
        storage_id: None,
        ranges: vec![ab_range],
        unevaluated_start: 2,
    };

    groups.assert_invariants(2);
}

/// Asserts that a rotated range is accepted, whatever its shift.
#[test]
fn shift_rotated() {
    let ab = group(&[0, 1]);
    let mut ab_range = range(&ab, 0, 2);
    ab_range.shift = isize::MIN;
    let groups = Groups {
        storage_id: None,
        ranges: vec![ab_range],
        unevaluated_start: 2,
    };

    groups.assert_invariants(2);
}

/// Asserts that an unevaluated region starting right after the last range is accepted.
#[test]
fn unevaluated_after_ranges() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 2,
    };

    groups.assert_invariants(4);
}

/// Asserts that an unevaluated region starting inside a range is rejected.
#[test]
#[should_panic(expected = "the unevaluated region cannot start inside a group range")]
fn unevaluated_inside_range() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 1,
    };

    groups.assert_invariants(4);
}

/// Asserts that an unevaluated region starting at the end of the storage is accepted.
#[test]
fn unevaluated_at_end() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 4,
    };

    groups.assert_invariants(4);
}

/// Asserts that an unevaluated region starting past the end of the storage is rejected.
#[test]
#[should_panic(expected = "the unevaluated region cannot start past the end of the storage")]
fn unevaluated_past_end() {
    let ab = group(&[0, 1]);
    let groups = Groups {
        storage_id: None,
        ranges: vec![range(&ab, 0, 2)],
        unevaluated_start: 5,
    };

    groups.assert_invariants(4);
}

/// Asserts that a storage without ranges and its unused boundary at 0 is accepted.
#[test]
fn no_ranges() {
    let groups = Groups::default();

    groups.assert_invariants(3);
}

/// Asserts that an empty storage without ranges is accepted.
#[test]
fn no_ranges_empty() {
    let groups = Groups::default();

    groups.assert_invariants(0);
}

/// Asserts that a storage without ranges but a nonzero stored boundary is rejected,
/// the boundary isn't tracked while the storage is implicitly ungrouped.
#[test]
#[should_panic(
    expected = "a storage without group ranges keeps its unused unevaluated boundary at 0"
)]
fn no_ranges_ungrouped() {
    let groups = Groups {
        storage_id: None,
        ranges: vec![],
        unevaluated_start: 2,
    };

    groups.assert_invariants(3);
}
