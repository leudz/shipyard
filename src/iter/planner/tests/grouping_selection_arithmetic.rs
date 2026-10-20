use super::super::*;
use super::{Storage, local_candidate, walk_list};
use crate::sparse_set::GroupRange;
use alloc::vec;

fn group() -> Arc<[StorageId]> {
    Arc::from([StorageId::Custom(0), StorageId::Custom(1)])
}

/// The storage of the `member`th member of `group`,
/// with a fully trusted range of `range_len` in a storage of `len` components.
fn member(group: &Arc<[StorageId]>, member: usize, range_len: usize, len: usize) -> Storage {
    Storage::new(
        group[member],
        len,
        range_len,
        vec![group_range(group, 0, range_len, 0, range_len)],
    )
}

/// Lays a storage made by `member` out with an unrotated range of `range_len`
/// trusting `trust`, then the unevaluated region from `unevaluated_start` to `len`.
fn set_regions(
    storage: &mut Storage,
    range_len: usize,
    trust: usize,
    unevaluated_start: usize,
    len: usize,
) {
    let range = &mut storage.groups.ranges[0];
    (range.len, range.trust) = (range_len, trust);
    storage.groups.unevaluated_start = unevaluated_start;
    storage.len = len;
}

/// A window child of `len` components without group metadata.
fn ungrouped(len: usize) -> Child<'static> {
    Child {
        can_captain: true,
        can_sailor: true,
        sail_time: len,
        candidates: Some(len),
        metadata: NodeMetadata::Ungrouped,
    }
}

/// Asserts that a walk costs `candidates * (2 + sailors)`.
#[test]
fn walk_formula() {
    assert_eq!(walk_work(0, 3), Some(0));
    assert_eq!(walk_work(10, 0), Some(20));
    assert_eq!(walk_work(10, 2), Some(40));
}

/// Asserts that walks past `u64` are reported as unrepresentable instead of wrapping,
/// right at the boundary.
#[cfg(target_pointer_width = "64")]
#[test]
fn walk_overflow() {
    assert_eq!(walk_work(usize::MAX / 2, 0), Some(u64::MAX - 1));
    assert_eq!(walk_work(usize::MAX / 2 + 1, 0), None);
    assert_eq!(walk_work(usize::MAX / 3, 1), Some(u64::MAX));
    assert_eq!(walk_work(usize::MAX / 3 + 1, 1), None);
}

/// Asserts that a grouped alternative whose walks sum past `u64` is dropped
/// instead of wrapping into a cheap estimate, the small ordinary walk stays.
#[cfg(target_pointer_width = "64")]
#[test]
fn huge_grouped() {
    let group = group();
    let (a, b) = (
        member(&group, 0, usize::MAX / 2, usize::MAX),
        member(&group, 1, usize::MAX / 2, usize::MAX),
    );
    let mut children = [a.child(), b.child(), ungrouped(5)];

    let selection = select(&mut children);

    assert!(selection.alternatives.is_empty());
    assert_eq!(selection.ordinary.captain, 2);
    // 5 candidates, 2 sailors.
    assert_eq!(selection.ordinary.work, Some(20));
}

/// Asserts that a single walk past `u64` sinks a grouped alternative,
/// even when the walks before it fit.
#[cfg(target_pointer_width = "64")]
#[test]
fn huge_unevaluated() {
    let group = group();
    let a = member(&group, 0, 3, usize::MAX);
    let b = member(&group, 1, 3, 3);

    assert!(select(&mut [a.child(), b.child()]).alternatives.is_empty());
}

/// Asserts that an ordinary walk whose estimate doesn't fit in a `u64` still beats
/// a small grouped alternative, unrepresentable comparisons keep ordinary.
#[test]
fn huge_ordinary() {
    let group = group();
    // The smallest sail time drives, but its follow-ups overflow the candidate count.
    let mut overflowing = ungrouped(1);
    overflowing.candidates = None;
    let (a, b) = (member(&group, 0, 3, 3), member(&group, 1, 3, 3));
    let mut children = [overflowing, a.child(), b.child()];

    let selection = select(&mut children);

    assert_eq!(selection.ordinary.captain, 0);
    assert_eq!(selection.ordinary.work, None);
    assert_eq!(selection.cheapest().unwrap().work, 9);
    assert!(selection.winner().is_none());
}

/// Asserts that a fully aligned group is a single aligned walk where the members'
/// lookups are avoided.
#[test]
fn aligned_recipe() {
    let group = group();
    let (a, b) = (member(&group, 0, 4, 4), member(&group, 1, 4, 4));
    let mut children = [a.child(), b.child(), ungrouped(9)];

    let selection = select(&mut children);
    let alternative = selection.cheapest().unwrap();

    assert_eq!(alternative.route, Route::Local { required: 0b011 });
    assert_eq!(
        walk_list(&local_candidate(&children, alternative)),
        [Walk::Aligned {
            range: 0,
            start: 0,
            len: 4,
            captains: 0b011
        }]
    );
    // 4 candidates, 1 sailor.
    assert_eq!((alternative.candidates, alternative.work), (4, 12));
}

/// Asserts that partial trust gives an aligned prefix, a single damaged walk owned by
/// the member with the shortest suffix, then unevaluated walks longest first.
#[test]
fn damaged_recipe() {
    let group = group();
    let mut a = member(&group, 0, 6, 9);
    let mut b = member(&group, 1, 5, 12);
    set_regions(&mut a, 6, 2, 7, 9);
    set_regions(&mut b, 5, 3, 6, 12);
    let mut children = [a.child(), b.child()];

    let selection = select(&mut children);
    let alternative = selection.cheapest().unwrap();

    assert_eq!(
        walk_list(&local_candidate(&children, alternative)),
        [
            Walk::Aligned {
                range: 0,
                start: 0,
                len: 2,
                captains: 0b11
            },
            // B trusts position 2 but A doesn't, B is its only captain.
            Walk::Damaged {
                range: 0,
                owner: 1,
                start: 2,
                len: 3,
                captains: 0b10
            },
            Walk::Unevaluated {
                owner: 1,
                start: 6,
                len: 6
            },
            Walk::Unevaluated {
                owner: 0,
                start: 7,
                len: 2
            },
        ]
    );
    // 2 aligned candidates without sailor, then 11 candidates with one sailor each.
    assert_eq!(alternative.work, 2 * 2 + 11 * 3);
}

/// Asserts that equal damaged suffixes and equal unevaluated regions are owned
/// in child order.
#[test]
fn recipe_ties() {
    let group = group();
    let mut a = member(&group, 0, 4, 6);
    let mut b = member(&group, 1, 4, 6);
    a.groups.ranges[0].trust = 1;
    b.groups.ranges[0].trust = 1;
    let mut children = [a.child(), b.child()];

    let selection = select(&mut children);

    assert_eq!(
        walk_list(&local_candidate(&children, selection.cheapest().unwrap())),
        [
            Walk::Aligned {
                range: 0,
                start: 0,
                len: 1,
                captains: 0b11
            },
            Walk::Damaged {
                range: 0,
                owner: 0,
                start: 1,
                len: 3,
                captains: 0b01
            },
            Walk::Unevaluated {
                owner: 0,
                start: 4,
                len: 2
            },
            Walk::Unevaluated {
                owner: 1,
                start: 4,
                len: 2
            },
        ]
    );
}

/// A range of `group` at `start`, `len` long with `shift` and `trust`.
fn group_range(
    group: &Arc<[StorageId]>,
    start: usize,
    len: usize,
    shift: isize,
    trust: usize,
) -> GroupRange {
    GroupRange {
        group: group.clone(),
        start,
        len,
        shift,
        trust,
    }
}

/// Group metadata of storage 0 with `ranges` and the unevaluated region at `unevaluated_start`.
fn storage_groups(ranges: Vec<GroupRange>, unevaluated_start: usize) -> Groups {
    Groups {
        storage_id: Some(StorageId::Custom(0)),
        ranges,
        unevaluated_start,
    }
}

/// Returns the window metadata of a storage of `len` components with `groups`.
fn metadata(groups: &Groups, len: usize) -> Option<WindowMetadata<'_>> {
    match NodeMetadata::of_storage(groups, len) {
        // SAFE: The handle points to `groups`, borrowed for as long as the returned metadata
        // and unmodified meanwhile.
        NodeMetadata::Window(handle) => Some(unsafe { handle.get(EntityCursor::empty()) }),
        _ => None,
    }
}

/// Asserts that a storage without group reports no metadata.
#[test]
fn metadata_ungrouped() {
    assert!(metadata(&Groups::default(), 5).is_none());
}

/// Asserts that a storage whose metadata doesn't record its own id reports nothing.
#[test]
fn metadata_missing_id() {
    let mut groups = storage_groups(vec![group_range(&group(), 0, 2, 0, 2)], 2);
    groups.storage_id = None;

    assert!(metadata(&groups, 2).is_none());
}

/// Asserts that every range is reported with its group, position, rotation and trust,
/// empty ones included, along with the storage-wide regions.
#[test]
fn metadata_ranges() {
    let ab = group();
    let ac: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(2)]);
    let ad: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(3)]);
    let groups = storage_groups(
        vec![
            group_range(&ab, 0, 2, 1, 2),
            group_range(&ac, 2, 0, -3, 0),
            group_range(&ad, 2, 3, 4, 1),
        ],
        6,
    );

    let window = metadata(&groups, 8).unwrap();

    assert_eq!(window.storage_id(), StorageId::Custom(0));
    assert_eq!((window.len, window.groups.unevaluated_start), (8, 6));
    let rings: Vec<(Ring, usize)> = window
        .groups
        .ranges
        .iter()
        .map(|range| (range.ring(), range.trust))
        .collect();
    assert_eq!(
        rings,
        [
            (
                Ring {
                    start: 0,
                    len: 2,
                    shift: 1
                },
                2
            ),
            (
                Ring {
                    start: 2,
                    len: 0,
                    shift: -3
                },
                0
            ),
            (
                Ring {
                    start: 2,
                    len: 3,
                    shift: 4
                },
                1
            ),
        ]
    );
    for (range, group) in window.groups.ranges.iter().zip([&ab, &ac, &ad]) {
        assert!(Arc::ptr_eq(&range.group, group));
    }
}

/// Asserts that a reset storage reports its emptied ranges, they're still a valid layout.
#[test]
fn metadata_reset() {
    let ac: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(2)]);
    let mut groups = storage_groups(
        vec![
            group_range(&group(), 0, 2, 0, 2),
            group_range(&ac, 2, 3, -1, 3),
        ],
        6,
    );

    groups.reset();
    let window = metadata(&groups, 6).unwrap();

    assert_eq!(window.groups.unevaluated_start, 0);
    assert!(
        window
            .groups
            .ranges
            .iter()
            .all(|range| range.start == 0 && range.len == 0 && range.trust == 0)
    );
}

/// Asserts that layouts outside the storage or out of order aren't trusted.
#[test]
fn metadata_invalid() {
    let ab = group();
    let ac: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(2)]);
    let invalid = [
        // The first range doesn't start the storage.
        (vec![group_range(&ab, 1, 2, 0, 2)], 3, 3),
        // A gap between two ranges.
        (
            vec![group_range(&ab, 0, 2, 0, 2), group_range(&ac, 3, 1, 0, 1)],
            4,
            4,
        ),
        // Trust past the range's length.
        (vec![group_range(&ab, 0, 2, 0, 3)], 2, 2),
        // The ranges overlap the unevaluated region.
        (vec![group_range(&ab, 0, 3, 0, 3)], 2, 3),
        // The unevaluated region starts past the storage's end.
        (vec![group_range(&ab, 0, 2, 0, 2)], 5, 4),
        // The ranges' end overflows.
        (
            vec![
                group_range(&ab, 0, usize::MAX, 0, 0),
                group_range(&ac, usize::MAX, 2, 0, 0),
            ],
            usize::MAX,
            usize::MAX,
        ),
    ];

    for (ranges, unevaluated_start, len) in invalid {
        let groups = storage_groups(ranges, unevaluated_start);

        assert!(metadata(&groups, len).is_none());
    }
}

/// Asserts that a child unable to sail drives and disables grouping.
#[test]
fn forced_captain() {
    let group = group();
    let mut entities = ungrouped(100);
    entities.can_sailor = false;
    let (a, b) = (member(&group, 0, 4, 4), member(&group, 1, 4, 4));
    let mut children = [a.child(), b.child(), entities];

    let selection = select(&mut children);

    assert_eq!(selection.ordinary.captain, 2);
    assert!(selection.alternatives.is_empty());
}

/// A nested `Mixed` child of `lookups` windows whose ordinary walk drives `candidates`
/// candidates for `work`, reporting `alternatives` as `(candidates, work)`.
fn nested(
    candidates: usize,
    work: u64,
    lookups: usize,
    alternatives: &[(usize, u64)],
) -> Child<'static> {
    let alternatives = alternatives
        .iter()
        .map(|&(candidates, work)| Alternative {
            candidates,
            work,
            route: Route::Local { required: 0 },
        })
        .collect();

    Child {
        can_captain: true,
        can_sailor: true,
        sail_time: candidates,
        candidates: Some(candidates),
        // The nested `Mixed` reports its selection like its constructor would.
        metadata: Selection {
            ordinary: OrdinaryCandidate {
                captain: 0,
                work: Some(work),
            },
            alternatives,
            lookups,
            filtered: false,
            windows: ScratchVec::new(),
            nested: Vec::new(),
        }
        .into_node(),
    }
}

/// Asserts that a nested `Mixed` as sailor costs one lookup per window below it,
/// and that its parent reports the sum of its children's lookups.
#[test]
fn nested_lookups() {
    let mut children = [ungrouped(4), nested(100, 300, 3, &[])];

    let selection = select(&mut children);

    assert_eq!(selection.lookups, 4);
    assert_eq!(selection.ordinary.captain, 0);
    // 4 candidates looking 3 windows up.
    assert_eq!(selection.ordinary.work, Some(4 * (2 + 3)));
}

/// Asserts that a nested `Mixed` driving the ordinary walk brings its own work,
/// its parent only adds the lookups of its other children.
#[test]
fn nested_captain() {
    let mut children = [nested(4, 30, 2, &[]), ungrouped(100)];

    let selection = select(&mut children);

    assert_eq!(selection.ordinary.captain, 0);
    assert_eq!(selection.ordinary.work, Some(30 + 4));
    assert!(selection.alternatives.is_empty());
}

/// Asserts that a parent ranks every alternative a child reported with its own lookups:
/// the child's cheaper walk has more candidates and loses once the parent looks 3 windows up
/// for each of them. The child isn't committed to its own cheapest walk.
#[test]
fn parent_reranks() {
    let mut children = [
        nested(1000, 5000, 2, &[(50, 100), (10, 110)]),
        ungrouped(1000),
        ungrouped(1000),
        ungrouped(1000),
    ];

    let selection = select(&mut children);

    let works: Vec<u64> = selection
        .alternatives
        .iter()
        .map(|alternative| alternative.work)
        .collect();
    assert_eq!(works, [100 + 50 * 3, 110 + 10 * 3]);

    // The child's second alternative.
    let winner = selection.winner().unwrap();
    assert_eq!(
        winner.route,
        Route::Nested {
            child: 0,
            alternative: 1
        }
    );
    let nested = selection.nested(0).alternatives[1];
    assert_eq!((nested.candidates, nested.work), (10, 110));
    assert_eq!(winner.candidates, 10);
}

/// Asserts that alternatives move up through every level, each one adding its lookups.
#[test]
fn two_levels() {
    let mut middle_children = [nested(20, 60, 2, &[(10, 20)]), ungrouped(30)];
    let middle = select(&mut middle_children);
    assert_eq!(middle.lookups, 3);
    assert_eq!(middle.alternatives[0].work, 20 + 10);

    let mut root_children = [
        Child {
            metadata: middle.into_node(),
            ..ungrouped(20)
        },
        ungrouped(40),
    ];
    let root = select(&mut root_children);

    let winner = root.winner().unwrap();
    assert_eq!(winner.work, 20 + 10 + 10);
    let Route::Nested {
        child: 0,
        alternative,
    } = winner.route
    else {
        panic!("the middle `Mixed` drives");
    };
    // The middle `Mixed` drives through its own nested `Mixed`.
    assert!(matches!(
        root.nested(0).alternatives[alternative].route,
        Route::Nested { child: 0, .. }
    ));
}

/// Asserts that a filtered child drops every alternative, its siblings' included,
/// and that a child unable to sail drops nested alternatives too.
#[test]
fn nested_disabled() {
    let mut filtered = ungrouped(5);
    filtered.metadata = NodeMetadata::Filtered;
    let mut children = [nested(10, 30, 2, &[(10, 20)]), filtered];
    assert!(select(&mut children).alternatives.is_empty());

    let mut entities = ungrouped(100);
    entities.can_sailor = false;
    let mut children = [nested(10, 30, 2, &[(10, 20)]), entities];
    assert!(select(&mut children).alternatives.is_empty());
}

/// Asserts that a nested alternative whose estimate overflows with its parent's lookups
/// is left out instead of wrapping.
#[test]
fn nested_overflow() {
    let mut children = [
        nested(10, 30, 2, &[(usize::MAX, u64::MAX - 1)]),
        ungrouped(1),
    ];

    assert!(select(&mut children).alternatives.is_empty());
}

/// Asserts that without groups, `choose` reports the ordinary walk `select` ranks in
/// `nested_captain` as the metadata of a `Mixed` without alternatives.
#[test]
fn chosen_ordinary() {
    let mut children = [nested(4, 30, 2, &[]), ungrouped(100)];

    let (captain, metadata) = choose(&mut children);

    assert_eq!(captain, 0);
    assert!(matches!(
        metadata.0,
        NodeMetadata::Mixed {
            lookups: 3,
            ordinary_work: Some(34)
        }
    ));
}

/// Asserts that `choose` reports a query containing a filter as filtered, along with its
/// ordinary captain, even when its other children are grouped.
#[test]
fn chosen_filtered() {
    let group = group();
    let (a, b) = (member(&group, 0, 4, 4), member(&group, 1, 4, 4));
    let mut filtered = ungrouped(2);
    filtered.metadata = NodeMetadata::Filtered;
    let mut children = [a.child(), b.child(), filtered];

    let (captain, metadata) = choose(&mut children);

    assert_eq!(captain, 2);
    assert!(matches!(metadata.0, NodeMetadata::Filtered));
}

/// Asserts that `choose` reports no alternative when a child unable to sail drives,
/// even though its other children are grouped, like `select` in `forced_captain`.
#[test]
fn chosen_forced() {
    let group = group();
    let mut entities = ungrouped(100);
    entities.can_sailor = false;
    let (a, b) = (member(&group, 0, 4, 4), member(&group, 1, 4, 4));
    let mut children = [a.child(), b.child(), entities];

    let (captain, metadata) = choose(&mut children);

    assert_eq!(captain, 2);
    // 100 candidates looking A and B up.
    assert!(matches!(
        metadata.0,
        NodeMetadata::Mixed {
            lookups: 3,
            ordinary_work: Some(400)
        }
    ));
}

/// Asserts that `choose` hands grouped children to `select`: their alternatives are reported
/// to the parent, next to the ordinary captain the constructor installs.
#[test]
fn chosen_grouped() {
    let group = group();
    let (a, b) = (member(&group, 0, 4, 4), member(&group, 1, 4, 4));
    let mut children = [a.child(), b.child(), ungrouped(9)];

    let (captain, metadata) = choose(&mut children);

    assert_eq!(captain, 0);
    let NodeMetadata::GroupedMixed(selection) = metadata.0 else {
        panic!("the aligned group is an alternative");
    };
    assert_eq!(selection.ordinary.captain, 0);
    assert_eq!(
        selection.cheapest().unwrap().route,
        Route::Local { required: 0b011 }
    );
}
