//! Publicly constructed grouped worlds shared by traversal and ownership tests.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::{IntoIter, Mixed, Shiper};
use crate::sparse_set::{FullRawWindow, Groups};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
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
    let mut world = World::new_for_test();
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

#[derive(Debug, PartialEq)]
pub(super) struct C(pub(super) u32);
impl Component for C {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
pub(super) struct D(pub(super) u32);
impl Component for D {
    type Tracking = track::Untracked;
}

/// Number of pairs in `ring_world`.
pub(super) const RING_PAIRS: usize = 15;

/// AB and AC groups after regroup promoted grouped entities into their ABC union,
/// then two late pairs. Pair `i` owns `A(i)` and `B(100 + i)`:
/// - pairs 0 to 7 start in AB, 0 and 1 gain a C and move to ABC
/// - pairs 8 to 11 own a C from the start and are in ABC
/// - pair 12 starts in AC and moves to ABC once it gains a B
/// - pairs 13 and 14 are added after the regroup, unevaluated in A and B
///
/// 7 AC entities own `A(200 + i)` and 3 B-only entities `B(300 + i)`.
///
/// The real planner prefers its grouped walks (38) over B captaining alone (54), 6 segments:
/// - AB, aligned
/// - ABC, aligned, cut in 3 where regroup left A's and B's wrap points, which differ
/// - A's unevaluated pairs
/// - B's unevaluated pairs, which A's region covers\
/// Returned ids are indexed by pair, independent of storage order.
pub(super) fn ring_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let mut pairs: Vec<EntityId> = (0..8)
        .map(|i| world.add_entity((A(i), B(100 + i))))
        .collect();
    pairs.extend((8..12).map(|i| world.add_entity((A(i), B(100 + i), C(i)))));
    pairs.push(world.add_entity((A(12), C(12))));
    for i in 0..7 {
        world.add_entity((A(200 + i), C(200 + i)));
    }
    for i in 0..3 {
        world.add_entity(B(300 + i));
    }
    world.group::<(A, B)>().group::<(A, C)>();

    world.add_component(pairs[0], C(0));
    world.add_component(pairs[1], C(1));
    world.add_component(pairs[12], B(112));
    world.regroup();

    pairs.extend((13..15).map(|i| world.add_entity((A(i), B(100 + i)))));

    (world, pairs)
}

/// Overwrites every range of a storage, in physical order, with `(start, len, shift, trust)`,
/// and where its unevaluated region starts.
fn set_regions(
    groups: &mut Groups,
    ranges: &[(usize, usize, isize, usize)],
    unevaluated_start: usize,
) {
    assert_eq!(groups.ranges.len(), ranges.len());

    for (range, &(start, len, shift, trust)) in groups.ranges.iter_mut().zip(ranges) {
        range.start = start;
        range.len = len;
        range.shift = shift;
        range.trust = trust;
    }
    groups.unevaluated_start = unevaluated_start;
}

/// AB and ABCD groups with partially trusted, rotated rings, where AB is empty and D was reset.
/// Entity `i` owns `A(i)`, `B(100 + i)`, `C(200 + i)` and `D(i)`, `q0`, `q1`, `q2` then `r`:
///
/// ```text
///             ABCD (logical)      unevaluated
/// A trust 3:  [q0 | q1 q2]        [r]    physical [q0 q1 q2 r], unrotated
/// B trust 1:  [q0 | r q1]         [q2]   physical [r q1 q0 q2], logical 0 at 2
/// C trust 3:  [q0 | q1 q2]        [r]    physical [q2 q0 q1 r], logical 0 at 1
/// ```
///
/// Members hold the same entities over the positions they both trust. `r` and `q2` each have
/// a component in A's or B's range and the other one unevaluated.
///
/// The `(A, B, C)` walks install, in order: an aligned segment where every child drives,
/// two damaged segments split at C's wrap point where A and C drive, A's unevaluated segment,
/// then B's which rejects `q2` thanks to the damaged walk's records.
pub(super) fn trust_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    world.group::<(A, B)>().group::<(A, B, C, D)>();
    let e: Vec<EntityId> = (0..4).map(|_| world.add_entity(())).collect();
    let (q0, q1, q2, r) = (0, 1, 2, 3);

    // Components land at the end of their storage in the order they're added.
    for i in [q0, q1, q2, r] {
        world.add_component(e[i], (A(i as u32), D(i as u32)));
    }
    for i in [r, q1, q0, q2] {
        world.add_component(e[i], B(100 + i as u32));
    }
    for i in [q2, q0, q1, r] {
        world.add_component(e[i], C(200 + i as u32));
    }
    {
        let (mut a, mut b, mut c, mut d) = world
            .borrow::<(
                ViewMut<'_, A>,
                ViewMut<'_, B>,
                ViewMut<'_, C>,
                ViewMut<'_, D>,
            )>()
            .unwrap();
        set_regions(&mut a.groups, &[(0, 0, 0, 0), (0, 3, 0, 3)], 3);
        set_regions(&mut b.groups, &[(0, 0, 0, 0), (0, 3, -1, 1)], 3);
        set_regions(&mut c.groups, &[(0, 3, 1, 3)], 3);
        set_regions(&mut d.groups, &[(0, 0, 0, 0)], 0);
    }

    (world, e)
}
