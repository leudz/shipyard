//! Publicly constructed grouped worlds shared by traversal and ownership tests.

use crate::iter::{IntoIter, Mixed, Shiper};
use crate::sparse_set::FullRawWindow;
use crate::{track, Component, EntityId, View, World};
use alloc::vec::Vec;

#[derive(Debug, PartialEq)]
pub(super) struct A(pub(super) u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
pub(super) struct B(pub(super) u32);
impl Component for B {
    type Tracking = track::Untracked;
}

pub(super) const ALIGNED: usize = 20;

/// Twenty aligned pairs, three late pairs found by A, then three found by B.
/// B's last walk interleaves matches with duplicates of A's late pairs while A ends
/// with three unmatched candidates. A's three evaluated ungrouped components gain B
/// after grouping, so membership alone cannot replace region coverage.
///
/// The real planner prefers these walks (76) over the ordinary walk (78).
/// Returned ids are indexed by their A value, independent of storage order.
pub(super) fn coverage_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let mut ids: Vec<_> = (0..ALIGNED as u32)
        .map(|i| world.add_entity((A(i), B(10 + i))))
        .collect();
    let late_b: Vec<_> = (23..26).map(|i| world.add_entity(A(i))).collect();
    world.group::<(A, B)>();
    ids.extend((20..23).map(|i| world.add_entity(A(i))));
    ids.extend(late_b);
    for i in [23, 20, 24, 21, 25, 22] {
        world.add_component(ids[i], B(10 + i as u32));
    }
    for i in 100..103 {
        world.add_entity(A(i));
    }
    (world, ids)
}

pub(super) type PairIter<'a> = Shiper<
    Mixed<(
        FullRawWindow<'a, A, track::Untracked>,
        FullRawWindow<'a, B, track::Untracked>,
    )>,
>;

/// Builds through `iter`, consumes the aligned prefix, and enters the first late
/// segment. Internal entry is used only to position a producer at a split boundary;
/// neither its rows nor its coverage are manufactured by the fixture.
pub(super) fn late_iter<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> PairIter<'a> {
    let mut iter = (a, b).iter();
    assert_eq!(iter.cursor.len(), ALIGNED);
    assert_eq!(iter.schedule.as_ref().unwrap().follow_ups().len(), 2);
    assert_eq!(iter.by_ref().take(ALIGNED).count(), ALIGNED);
    assert!(iter.enter_next_segment());
    assert_eq!(iter.cursor.len(), 6);
    iter
}
