use super::super::*;
use super::{Storage, children, walk_list};
use crate::sparse_set::GroupRange;

const A: u64 = 0;
const B: u64 = 1;
const C: u64 = 2;
const D: u64 = 3;
const E: u64 = 4;
const F: u64 = 5;

/// Returns the group made of the storages `ids`.
fn group(ids: &[u64]) -> Arc<[StorageId]> {
    let mut ids: Vec<StorageId> = ids.iter().map(|&id| StorageId::Custom(id)).collect();
    ids.sort_unstable();

    Arc::from(ids)
}

/// Storage `storage` holding a range for each of `groups`,
/// back to back, each with 2 fully trusted components.
fn window(storage: u64, groups: &[&Arc<[StorageId]>]) -> Storage {
    let ranges: Vec<(&Arc<[StorageId]>, usize, usize)> =
        groups.iter().map(|&group| (group, 2, 2)).collect();

    window_with(storage, &ranges, 0)
}

/// A nested `Mixed` child, it never holds group members of its parent.
fn nested() -> Child<'static> {
    Child {
        can_captain: true,
        can_sailor: true,
        sail_time: 0,
        candidates: Some(0),
        entities: EntityCursor::empty(),
        metadata: NodeMetadata::Mixed {
            lookups: 1,
            ordinary_work: Some(4),
        },
    }
}

/// Returns the groups of `set`'s ranges, in order.
fn range_groups(set: &CoverSet) -> Vec<Arc<[StorageId]>> {
    set.ranges.iter().map(|range| range.group.clone()).collect()
}

/// Returns the children taking part in `range`, in order.
fn range_children(range: &CoverRange) -> Vec<usize> {
    range.members.iter().map(|member| member.child).collect()
}

/// Asserts that with AB and AC and no ABC union yet, an `(A, B, C)` query has to cover
/// A, B and C, with no range to walk: its matches can only be unevaluated.
#[test]
fn absent_union() {
    let (ab, ac) = (group(&[A, B]), group(&[A, C]));
    let storages = [window(A, &[&ab, &ac]), window(B, &[&ab]), window(C, &[&ac])];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].required, 0b111);
    assert!(sets[0].ranges.is_empty());
}

/// Asserts that with AB, AC and their ABC union, an `(A, B, C)` query only walks ABC,
/// AB and AC can't hold a fully evaluated match.
#[test]
fn union_only() {
    let (ab, ac, abc) = (group(&[A, B]), group(&[A, C]), group(&[A, B, C]));
    let storages = [
        window(A, &[&ab, &ac, &abc]),
        window(B, &[&ab, &abc]),
        window(C, &[&ac, &abc]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].required, 0b111);
    assert_eq!(sets[0].ranges.len(), 1);
    assert!(Arc::ptr_eq(&range_groups(&sets[0])[0], &abc));
    assert_eq!(range_children(&sets[0].ranges[0]), [0, 1, 2]);
}

/// Asserts that with AB, AC and ABC, an `(A, B)` query walks AB and ABC
/// using A and B alone, C isn't borrowed.
#[test]
fn projection() {
    let (ab, ac, abc) = (group(&[A, B]), group(&[A, C]), group(&[A, B, C]));
    let storages = [window(A, &[&ab, &ac, &abc]), window(B, &[&ab, &abc])];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].required, 0b11);
    let groups = range_groups(&sets[0]);
    assert_eq!(groups.len(), 2);
    assert!(Arc::ptr_eq(&groups[0], &ab) && Arc::ptr_eq(&groups[1], &abc));
    assert_eq!(range_children(&sets[0].ranges[1]), [0, 1]);
}

/// Asserts that an ABC group alone doesn't cover an `(A, B)` query,
/// AB matches without C sit outside its ranges.
#[test]
fn larger_only() {
    let abc = group(&[A, B, C]);
    let storages = [window(A, &[&abc]), window(B, &[&abc])];
    let children = children(&storages);

    assert!(cover_sets(&children).next().is_none());
}

/// Asserts that disjoint AB and CD give distinct sets, ordered by their first child.
#[test]
fn disjoint() {
    let (ab, cd) = (group(&[A, B]), group(&[C, D]));
    let storages = [
        window(C, &[&cd]),
        window(D, &[&cd]),
        window(A, &[&ab]),
        window(B, &[&ab]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    let required: Vec<u32> = sets.iter().map(|set| set.required).collect();
    assert_eq!(required, [0b0011, 0b1100]);
    assert!(Arc::ptr_eq(&range_groups(&sets[0])[0], &cd));
    assert!(Arc::ptr_eq(&range_groups(&sets[1])[0], &ab));
}

/// Asserts that with AB and ACD, an `(A, B, C)` query covers AB, walking AB and the ABCD
/// union, where C comes along as an extra member and D isn't borrowed.
#[test]
fn extra_member() {
    let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));
    let storages = [
        window(A, &[&ab, &acd, &abcd]),
        window(B, &[&ab, &abcd]),
        window(C, &[&acd, &abcd]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].required, 0b011);
    let groups = range_groups(&sets[0]);
    assert_eq!(groups.len(), 2);
    assert!(Arc::ptr_eq(&groups[0], &ab) && Arc::ptr_eq(&groups[1], &abcd));
    assert_eq!(range_children(&sets[0].ranges[0]), [0, 1]);
    assert_eq!(range_children(&sets[0].ranges[1]), [0, 1, 2]);
}

/// Asserts that overlaps are transitive: AB, BC and CD give a single set.
#[test]
fn transitive() {
    let (ab, bc, cd) = (group(&[A, B]), group(&[B, C]), group(&[C, D]));
    let storages = [
        window(A, &[&ab]),
        window(B, &[&ab, &bc]),
        window(C, &[&bc, &cd]),
        window(D, &[&cd]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].required, 0b1111);
    assert!(sets[0].ranges.is_empty());
}

/// Asserts that a declared group and a larger one containing it are joined,
/// only the larger one's range contains every required member.
#[test]
fn subset() {
    let (ab, abc) = (group(&[A, B]), group(&[A, B, C]));
    let storages = [
        window(A, &[&ab, &abc]),
        window(B, &[&ab, &abc]),
        window(C, &[&abc]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].required, 0b111);
    assert_eq!(sets[0].ranges.len(), 1);
    assert!(Arc::ptr_eq(&range_groups(&sets[0])[0], &abc));
}

/// Asserts that a group with a member held by two children isn't used,
/// we don't pick which one would drive.
#[test]
fn ambiguous_required() {
    let ab = group(&[A, B]);
    let storages = [window(A, &[&ab]), window(A, &[&ab]), window(B, &[&ab])];
    let children = children(&storages);

    assert!(cover_sets(&children).next().is_none());
}

/// Asserts that an extra member held by two children doesn't come along.
#[test]
fn ambiguous_extra() {
    let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));
    let storages = [
        window(A, &[&ab, &acd, &abcd]),
        window(B, &[&ab, &abcd]),
        window(C, &[&acd, &abcd]),
        window(C, &[&acd, &abcd]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets.len(), 1);
    assert_eq!(range_children(&sets[0].ranges[1]), [0, 1]);
}

/// Asserts that groups don't cross `Mixed` boundaries: a member held below a nested `Mixed`
/// doesn't make its parent hold the group.
#[test]
fn nested_boundary() {
    let ab = group(&[A, B]);
    let a = window(A, &[&ab]);
    let children = [a.child(), nested()];

    assert!(cover_sets(&children).next().is_none());
}

/// Asserts that each member reports its own part of a range, in child order,
/// whatever the order of the group's ids.
#[test]
fn member_parts() {
    let ab = group(&[A, B]);
    let mut a = window(A, &[&ab]);
    a.ranges[0].shift = -1;
    a.ranges[0].trust = 1;
    let storages = [window(B, &[&ab]), a];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert_eq!(sets[0].required, 0b11);
    assert_eq!(
        *sets[0].ranges[0].members,
        [
            RangeMember {
                child: 0,
                ring: Ring {
                    start: 0,
                    len: 2,
                    shift: 0,
                },
                trust: 2,
            },
            RangeMember {
                child: 1,
                ring: Ring {
                    start: 0,
                    len: 2,
                    shift: -1,
                },
                trust: 1,
            },
        ]
    );
}

/// Asserts that a range with as many members as fit inline lists them all, in child order.
#[test]
fn members_inline() {
    let abcd = group(&[A, B, C, D]);
    let storages = [
        window(D, &[&abcd]),
        window(C, &[&abcd]),
        window(B, &[&abcd]),
        window(A, &[&abcd]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert!(matches!(
        sets[0].ranges[0].members,
        ScratchVec::Inline { .. }
    ));
    assert_eq!(range_children(&sets[0].ranges[0]), [0, 1, 2, 3]);
}

/// Asserts that a range with more members than fit inline moves them to the heap
/// and still lists them all, in child order.
#[test]
fn members_spilled() {
    let abcde = group(&[A, B, C, D, E]);
    let storages = [
        window(E, &[&abcde]),
        window(D, &[&abcde]),
        window(C, &[&abcde]),
        window(B, &[&abcde]),
        window(A, &[&abcde]),
    ];
    let children = children(&storages);

    let sets: Vec<CoverSet> = cover_sets(&children).collect();

    assert!(matches!(sets[0].ranges[0].members, ScratchVec::Heap(_)));
    assert_eq!(range_children(&sets[0].ranges[0]), [0, 1, 2, 3, 4]);
}

/// Storage `storage` holding a range for each `(group, len, trust)`,
/// back to back and unrotated, followed by `unevaluated` unevaluated components.
fn window_with(
    storage: u64,
    ranges: &[(&Arc<[StorageId]>, usize, usize)],
    unevaluated: usize,
) -> Storage {
    let mut len = 0;
    let ranges: Vec<GroupRange> = ranges
        .iter()
        .map(|&(group, range_len, trust)| {
            let range = GroupRange {
                group: group.clone(),
                start: len,
                len: range_len,
                shift: 0,
                trust,
            };
            len += range_len;

            range
        })
        .collect();
    let unevaluated_start = len;
    len += unevaluated;

    Storage {
        id: StorageId::Custom(storage),
        len,
        unevaluated_start,
        ranges,
    }
}

/// Returns the walks of the first cover set of `children`, placeable or not.
fn first_candidate(children: &[Child<'_>]) -> GroupedCandidate {
    let set = cover_sets(children)
        .next()
        .expect("the children hold a group");

    GroupedCandidate::new(children, set)
}

fn aligned(range: usize, start: usize, len: usize, captains: u32) -> Walk {
    Walk::Aligned {
        range,
        start,
        len,
        captains,
    }
}

fn damaged(range: usize, owner: usize, start: usize, len: usize, captains: u32) -> Walk {
    Walk::Damaged {
        range,
        owner,
        start,
        len,
        captains,
    }
}

fn unevaluated(owner: usize, start: usize, len: usize) -> Walk {
    Walk::Unevaluated { owner, start, len }
}

/// Asserts that an `(A, B)` query walks the aligned part of each range containing A and B,
/// AB then the ABC union, where A and B read directly and C isn't involved.
#[test]
fn walks_ranges() {
    let (ab, ac, abc) = (group(&[A, B]), group(&[A, C]), group(&[A, B, C]));
    let storages = [
        window_with(A, &[(&ab, 2, 2), (&ac, 1, 1), (&abc, 3, 3)], 0),
        window_with(B, &[(&ab, 2, 2), (&abc, 3, 3)], 0),
    ];
    let children = children(&storages);

    let candidate = first_candidate(&children);

    assert_eq!(
        walk_list(&candidate),
        [aligned(0, 0, 2, 0b11), aligned(1, 0, 3, 0b11)]
    );
    // 5 candidates without sailor.
    let alternative = Alternative::local(candidate, 2).unwrap();
    assert_eq!((alternative.candidates, alternative.work), (5, 10));
}

/// Asserts that unevaluated walks come once, after every range walk,
/// longest first and in child order on ties.
#[test]
fn walks_unevaluated_once() {
    let (ab, abc) = (group(&[A, B]), group(&[A, B, C]));
    let storages = [
        window_with(A, &[(&ab, 2, 2), (&abc, 1, 1)], 2),
        window_with(B, &[(&ab, 2, 2), (&abc, 1, 1)], 3),
        window_with(D, &[], 0),
    ];
    let children = children(&storages);

    let candidate = first_candidate(&children);

    assert_eq!(
        walk_list(&candidate),
        [
            aligned(0, 0, 2, 0b11),
            aligned(1, 0, 1, 0b11),
            unevaluated(1, 3, 3),
            unevaluated(0, 3, 2),
        ]
    );
}

/// Asserts that the aligned prefix stops at the lowest trust of the required members,
/// the shortest damaged suffix drives the rest, and a member trusting positions the owner
/// doesn't trust can't read directly there.
#[test]
fn walks_required_trust() {
    let ab = group(&[A, B]);
    let storages = [
        window_with(A, &[(&ab, 5, 1)], 0),
        window_with(B, &[(&ab, 4, 3)], 0),
    ];
    let children = children(&storages);

    let candidate = first_candidate(&children);

    // B's suffix is the shortest, A trusts none of it.
    assert_eq!(
        walk_list(&candidate),
        [aligned(0, 0, 1, 0b11), damaged(0, 1, 1, 3, 0b10)]
    );
}

/// Asserts that on equal damaged suffixes, the first required member in child order owns
/// the walk, and B trusting more than the owner doesn't make it a captain.
#[test]
fn walks_damaged_tie() {
    let ab = group(&[A, B]);
    let storages = [
        window_with(A, &[(&ab, 4, 1)], 0),
        window_with(B, &[(&ab, 4, 3)], 0),
    ];
    let children = children(&storages);

    assert_eq!(
        walk_list(&first_candidate(&children)),
        [aligned(0, 0, 1, 0b11), damaged(0, 0, 1, 3, 0b01)]
    );
}

/// Returns `(A, B, C)` storages with AB, ACD and the ABCD union, D isn't borrowed.
/// C trusts `c_trust` of ABCD's 6 positions, A and B trust all of them.
fn extra_storages(c_trust: usize) -> [Storage; 3] {
    let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));

    [
        window_with(A, &[(&ab, 2, 2), (&acd, 1, 1), (&abcd, 6, 6)], 0),
        window_with(B, &[(&ab, 2, 2), (&abcd, 6, 6)], 0),
        window_with(C, &[(&acd, 1, 1), (&abcd, 6, c_trust)], 0),
    ]
}

/// Asserts that an extra member trusting none of a range is looked up all along,
/// A and B still read directly over the whole range.
#[test]
fn walks_extra_untrusted() {
    assert_eq!(
        walk_list(&first_candidate(&children(&extra_storages(0)))),
        [aligned(0, 0, 2, 0b011), aligned(1, 0, 6, 0b011)]
    );
}

/// Asserts that an extra member reads directly up to its trust, splitting the aligned
/// part there, without shortening A and B's aligned part.
#[test]
fn walks_extra_partial() {
    let candidate = first_candidate(&children(&extra_storages(3)));

    assert_eq!(
        walk_list(&candidate),
        [
            aligned(0, 0, 2, 0b011),
            aligned(1, 0, 3, 0b111),
            aligned(1, 3, 3, 0b011),
        ]
    );
    // AB's 2 and ABCD's last 3 candidates look C up, ABCD's first 3 look nothing up.
    let alternative = Alternative::local(candidate, 3).unwrap();
    assert_eq!(
        (alternative.candidates, alternative.work),
        (8, 2 * 3 + 3 * 2 + 3 * 3)
    );
}

/// Asserts that an extra member trusting a whole range reads directly over all of it.
#[test]
fn walks_extra_trusted() {
    assert_eq!(
        walk_list(&first_candidate(&children(&extra_storages(6)))),
        [aligned(0, 0, 2, 0b011), aligned(1, 0, 6, 0b111)]
    );
}

/// Asserts that a damaged suffix is split at every trust endpoint: members read directly
/// where they and the owner both trust the position, then the owner drives alone.
#[test]
fn walks_damaged_extras() {
    let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));
    let storages = [
        window_with(A, &[(&ab, 0, 0), (&acd, 0, 0), (&abcd, 8, 2)], 0),
        window_with(B, &[(&ab, 0, 0), (&abcd, 6, 5)], 0),
        window_with(C, &[(&acd, 0, 0), (&abcd, 7, 4)], 0),
    ];
    let children = children(&storages);

    let candidate = first_candidate(&children);

    // B has the shortest suffix. C trusts up to 4 and B up to 5, past that B drives alone.
    assert_eq!(
        walk_list(&candidate),
        [
            aligned(1, 0, 2, 0b111),
            damaged(1, 1, 2, 2, 0b110),
            damaged(1, 1, 4, 2, 0b010),
        ]
    );
}

/// Asserts that a cover set without any containing range, AB and AC before their ABC union
/// exists, only walks its required members' unevaluated regions.
#[test]
fn walks_unevaluated_only() {
    let (ab, ac) = (group(&[A, B]), group(&[A, C]));
    let storages = [
        window_with(A, &[(&ab, 0, 0), (&ac, 0, 0)], 3),
        window_with(B, &[(&ab, 0, 0)], 1),
        window_with(C, &[(&ac, 0, 0)], 2),
    ];
    let children = children(&storages);

    let candidate = first_candidate(&children);

    assert_eq!(
        walk_list(&candidate),
        [
            unevaluated(0, 0, 3),
            unevaluated(2, 0, 2),
            unevaluated(1, 0, 1)
        ]
    );
    // Every candidate looks the two other children up.
    let alternative = Alternative::local(candidate, 3).unwrap();
    assert_eq!((alternative.candidates, alternative.work), (6, 6 * 4));
}

/// Asserts that a grouped walk over several ranges can cost more than an ordinary walk
/// with a small captain.
#[test]
fn estimates_ordinary() {
    let (ab, abc) = (group(&[A, B]), group(&[A, B, C]));
    let storages = [
        window_with(A, &[(&ab, 20, 20), (&abc, 30, 30)], 0),
        window_with(B, &[(&ab, 20, 20), (&abc, 30, 30)], 0),
        window_with(9, &[], 3),
    ];
    let children = children(&storages);

    let grouped = Alternative::local(first_candidate(&children), 3).unwrap();
    let ordinary = ordinary_candidate(&children, 3);

    // 50 candidates looking the small storage up against 3 candidates looking A and B up.
    assert_eq!(grouped.work, 50 * 3);
    assert_eq!((ordinary.captain, ordinary.work), (2, Some(3 * 4)));
}

/// Asserts that disjoint cover sets get their own walks and estimates,
/// the smaller group's being cheaper.
#[test]
fn estimates_disjoint() {
    let (ab, cd) = (group(&[A, B]), group(&[C, D]));
    let storages = [
        window_with(A, &[(&ab, 40, 40)], 0),
        window_with(B, &[(&ab, 40, 40)], 0),
        window_with(C, &[(&cd, 10, 10)], 0),
        window_with(D, &[(&cd, 10, 10)], 0),
    ];
    let children = children(&storages);

    let works: Vec<u64> = cover_sets(&children)
        .map(|set| {
            Alternative::local(GroupedCandidate::new(&children, set), 4)
                .unwrap()
                .work
        })
        .collect();

    // Each walk reads its two members and looks the two others up.
    assert_eq!(works, [40 * 4, 10 * 4]);
}

/// Asserts that `pieces` splits only where the captains change, ignores bounds outside
/// the positions and gives nothing for empty positions.
#[test]
fn split_pieces() {
    let captains_at = |position: usize| if position < 4 { 0b11 } else { 0b01 };

    assert_eq!(
        pieces(1..7, [2, 4, 4, 9, 0].into_iter(), captains_at).collect::<Vec<_>>(),
        [(1..4, 0b11), (4..7, 0b01)]
    );
    assert!(pieces(3..3, [3].into_iter(), captains_at).next().is_none());
}

/// Asserts that a range with more members than a handful splits its aligned part at each of
/// their trusts, whatever order the members come in, and only the trusting ones read directly.
#[test]
fn walks_many_bounds() {
    // F isn't borrowed, so C, D and E only come along with AB.
    let (ab, abcdef) = (group(&[A, B]), group(&[A, B, C, D, E, F]));
    let storages = [
        window_with(A, &[(&ab, 2, 2), (&abcdef, 8, 8)], 0),
        window_with(B, &[(&ab, 2, 2), (&abcdef, 8, 8)], 0),
        window_with(C, &[(&abcdef, 8, 6)], 0),
        window_with(D, &[(&abcdef, 8, 2)], 0),
        window_with(E, &[(&abcdef, 8, 4)], 0),
    ];
    let children = children(&storages);

    // D stops trusting at 2, E at 4 and C at 6.
    assert_eq!(
        walk_list(&first_candidate(&children)),
        [
            aligned(0, 0, 2, 0b11),
            aligned(1, 0, 2, 0b11111),
            aligned(1, 2, 2, 0b10111),
            aligned(1, 4, 2, 0b00111),
            aligned(1, 6, 2, 0b00011),
        ]
    );
}
