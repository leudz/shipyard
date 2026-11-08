//! Segments of grouped walks over rotated rings and several ranges per storage.
//!
//! Queries are built through their real constructors, then their grouped alternative is
//! installed whether or not it's the cheaper walk.\
//! Fixtures place components in a chosen physical order then describe it with group metadata,
//! each one documents why its metadata is valid.\
//! Expected matches come from looking each entity up, not from the walks under test.

use super::grouping_regions::{assert_skipped, grouped, record, segment, segments};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::iter::{IntoShiperator, Mixed, Shiper};
use crate::sparse_set::{FullRawWindow, Groups};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use alloc::vec::Vec;
use core::ptr;

#[derive(Debug, PartialEq)]
struct A(u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct B(u32);
impl Component for B {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct C(u32);
impl Component for C {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct D(u32);
impl Component for D {
    type Tracking = track::Untracked;
}

/// Iterator over the pair `(A, B)`.
type PairIter<'a> = Shiper<
    Mixed<(
        FullRawWindow<'a, A, track::Untracked>,
        FullRawWindow<'a, B, track::Untracked>,
    )>,
>;

/// Iterator over the triple `(A, B, C)`.
type TripleIter<'a> = Shiper<
    Mixed<(
        FullRawWindow<'a, A, track::Untracked>,
        FullRawWindow<'a, B, track::Untracked>,
        FullRawWindow<'a, C, track::Untracked>,
    )>,
>;

/// Builds the grouped walks of `(&a, &b)`.
fn pair<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> PairIter<'a> {
    grouped((a, b).into_shiperator())
}

/// Builds the grouped walks of `(&a, &b, &c)`.
fn triple<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>, c: &'a View<'_, C>) -> TripleIter<'a> {
    grouped((a, b, c).into_shiperator())
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

/// Returns the sorted entities of `all` `is_match` accepts.
fn matching(all: &[EntityId], is_match: impl Fn(EntityId) -> bool) -> Vec<EntityId> {
    let mut matching: Vec<EntityId> = all.iter().copied().filter(|&id| is_match(id)).collect();
    matching.sort_unstable();

    matching
}

/// Asserts that the walks of `(&a, &b)` yield each entity of `all` owning A and B exactly once,
/// with its own components, through `next` and through `fold`.
fn assert_pairs(a: &View<'_, A>, b: &View<'_, B>, all: &[EntityId]) {
    let check = |id: EntityId, (a_item, b_item): (&A, &B)| {
        assert!(ptr::eq(a_item, a.get(id).unwrap()), "{id:?}'s A");
        assert!(ptr::eq(b_item, b.get(id).unwrap()), "{id:?}'s B");
    };
    let expected = matching(all, |id| a.contains(id) && b.contains(id));

    let mut ids = Vec::new();
    for (id, item) in pair(a, b).with_id() {
        check(id, item);
        ids.push(id);
    }
    ids.sort_unstable();
    assert_eq!(ids, expected);

    let mut ids = pair(a, b)
        .with_id()
        .fold(Vec::new(), |mut ids, (id, item)| {
            check(id, item);
            ids.push(id);
            ids
        });
    ids.sort_unstable();
    assert_eq!(ids, expected);
}

/// Asserts that the walks of `(&a, &b, &c)` yield each entity of `all` owning A, B and C
/// exactly once, with its own components, through `next` and through `fold`.
fn assert_triples(a: &View<'_, A>, b: &View<'_, B>, c: &View<'_, C>, all: &[EntityId]) {
    let check = |id: EntityId, (a_item, b_item, c_item): (&A, &B, &C)| {
        assert!(ptr::eq(a_item, a.get(id).unwrap()), "{id:?}'s A");
        assert!(ptr::eq(b_item, b.get(id).unwrap()), "{id:?}'s B");
        assert!(ptr::eq(c_item, c.get(id).unwrap()), "{id:?}'s C");
    };
    let expected = matching(all, |id| a.contains(id) && b.contains(id) && c.contains(id));

    let mut ids = Vec::new();
    for (id, item) in triple(a, b, c).with_id() {
        check(id, item);
        ids.push(id);
    }
    ids.sort_unstable();
    assert_eq!(ids, expected);

    let mut ids = triple(a, b, c)
        .with_id()
        .fold(Vec::new(), |mut ids, (id, item)| {
            check(id, item);
            ids.push(id);
            ids
        });
    ids.sort_unstable();
    assert_eq!(ids, expected);
}

/// A world with a fully trusted AB group of 4 entities, rotated differently in each member.
///
/// ```text
/// logical:  [e1 e2 e3 e0]
/// A:        [e0 e1 e2 e3]  logical 0 at physical 1, wraps after logical 2
/// B:        [e3 e0 e1 e2]  logical 0 at physical 2, wraps after logical 1
/// ```
///
/// Both members hold the same logical sequence, as group metadata requires.\
/// `shifts` are A's and B's shifts, any shift with these remainders describes the same rings.
fn rotated_world(shifts: (isize, isize)) -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let e: Vec<EntityId> = (0..4).map(|_| world.add_entity(())).collect();

    // Components land at the end of their storage in the order they're added.
    for i in [0, 1, 2, 3] {
        world.add_component(e[i], A(i as u32));
    }
    for i in [3, 0, 1, 2] {
        world.add_component(e[i], B(10 + i as u32));
    }

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        set_regions(&mut a.groups, &[(0, 4, shifts.0, 4)], 4);
        set_regions(&mut b.groups, &[(0, 4, shifts.1, 4)], 4);
    }

    (world, e)
}

/// Asserts that an aligned walk over whole rings starts at its driver A's range start and is
/// only cut at B's, every captain starting each piece at its own physical index, and that no
/// record is needed.
#[test]
fn wrap_points() {
    let (world, all) = rotated_world((1, 2));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 0..3), 0b11, &[Some(0), Some(1)], &[]),
            segment((0, 3..4), 0b11, &[Some(3), Some(0)], &[]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that negative shifts and shifts larger than the length place the walk exactly like
/// their remainders.
#[test]
fn unnormalized_shifts() {
    let (world, all) = rotated_world((-3, 6));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 0..3), 0b11, &[Some(0), Some(1)], &[]),
            segment((0, 3..4), 0b11, &[Some(3), Some(0)], &[]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that a damaged walk crossing its owner's wrap point owns both physical pieces:
/// B's unevaluated walk gets a record for each and rejects `e3`, which A's damaged walk yielded.
/// The aligned prefix ends right at B's wrap point.
///
/// ```text
///             range (logical)       unevaluated
/// A trust 2:  [e1 e2 | e3 e0]       [late]   physical [e0 e1 e2 e3 late], logical 0 at 1
/// B trust 4:  [e1 e2 | e0 late]     [e3]     physical [e0 late e1 e2 e3], logical 0 at 2
/// ```
///
/// Members hold the same entities over the 2 positions they both trust. `e3` and `late` each
/// have a component in a range and the other one unevaluated, every other match is in both
/// ranges.
#[test]
fn damaged_wrap() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let e: Vec<EntityId> = (0..5).map(|_| world.add_entity(())).collect();
    let late = e[4];

    for i in [0, 1, 2, 3, 4] {
        world.add_component(e[i], A(i as u32));
    }
    for i in [0, 4, 1, 2, 3] {
        world.add_component(e[i], B(10 + i as u32));
    }
    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        set_regions(&mut a.groups, &[(0, 4, 1, 2)], 4);
        set_regions(&mut b.groups, &[(0, 4, 2, 4)], 4);
    }

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    assert_eq!(a.dense[4], late);

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 1..3), 0b11, &[Some(1), Some(2)], &[]),
            segment((0, 3..4), 0b01, &[Some(3), None], &[]),
            segment((0, 0..1), 0b01, &[Some(0), None], &[]),
            segment((0, 4..5), 0b01, &[Some(4), None], &[]),
            segment(
                (1, 4..5),
                0b10,
                &[None, Some(4)],
                &[record(0, 3, 4), record(0, 0, 1), record(0, 4, 5)]
            ),
        ]
    );
    assert_pairs(&a, &b, &e);
}

/// A world with AB and ABCD groups, where D was reset, holding:
/// - `p0` and `p1` in AB
/// - `q0`, `q1` and `q2` in ABCD, C trusting only the first 2
///
/// ```text
///            AB        ABCD
/// A:         [p0 p1]   [q0 q1 q2]   unrotated
/// B:         [p1 p0]   [q2 q0 q1]   each logical 0 at its range's physical 1
/// C:                   [q1 q2 q0]   logical 0 at physical 2, trust 2
/// D:                   []           every component unevaluated
/// ```
///
/// A and B hold the same logical sequences in both ranges, C holds ABCD's as well.\
/// No query borrows D, its reset doesn't change what the others' metadata says.
fn projection_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, B, C, D)>()
        .build();
    let p: Vec<EntityId> = (0..2).map(|_| world.add_entity(())).collect();
    let q: Vec<EntityId> = (0..3).map(|_| world.add_entity(())).collect();

    for (i, &id) in [p[0], p[1], q[0], q[1], q[2]].iter().enumerate() {
        world.add_component(id, A(i as u32));
    }
    for (i, &id) in [p[1], p[0], q[2], q[0], q[1]].iter().enumerate() {
        world.add_component(id, B(i as u32));
    }
    for (i, &id) in [q[1], q[2], q[0]].iter().enumerate() {
        world.add_component(id, C(i as u32));
    }
    for (i, &id) in q.iter().enumerate() {
        world.add_component(id, D(i as u32));
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
        set_regions(&mut a.groups, &[(0, 2, 0, 2), (2, 3, 0, 3)], 5);
        set_regions(&mut b.groups, &[(0, 2, 1, 2), (2, 3, 1, 3)], 5);
        set_regions(&mut c.groups, &[(0, 3, 2, 2)], 3);
        set_regions(&mut d.groups, &[(0, 0, 0, 0)], 0);
    }

    let all = p.into_iter().chain(q).collect();

    (world, all)
}

/// Asserts that an `(A, B, C)` query over AB and ABCD walks both ranges, cutting each walk at
/// every captain's wrap point and where C stops being trusted. Pieces never span two ranges.
#[test]
fn ranges_and_extra() {
    let (world, all) = projection_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert_eq!(
        segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]),
        [
            // AB, C is looked up.
            segment((0, 0..1), 0b011, &[Some(0), Some(1), None], &[]),
            segment((0, 1..2), 0b011, &[Some(1), Some(0), None], &[]),
            // ABCD, C reads directly over the 2 positions it trusts.
            segment((0, 2..3), 0b111, &[Some(2), Some(3), Some(2)], &[]),
            segment((0, 3..4), 0b111, &[Some(3), Some(4), Some(0)], &[]),
            segment((0, 4..5), 0b011, &[Some(4), Some(2), None], &[]),
        ]
    );
    assert_triples(&a, &b, &c, &all);
}

/// Asserts that an `(A, B)` query over AB and ABCD walks both ranges from A and B's metadata
/// alone, while C and D aren't borrowed and D was reset.
#[test]
fn projection() {
    let (world, all) = projection_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 0..1), 0b11, &[Some(0), Some(1)], &[]),
            segment((0, 1..2), 0b11, &[Some(1), Some(0)], &[]),
            segment((0, 2..4), 0b11, &[Some(2), Some(3)], &[]),
            segment((0, 4..5), 0b11, &[Some(4), Some(2)], &[]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that a damaged walk with several captains still owns its region: B's unevaluated
/// walk gets the records of both its pieces and rejects `q2`, which the damaged walk yielded.
///
/// With AB and ABCD groups, where AB is empty and D was reset:
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
#[test]
fn damaged_captains() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, B, C, D)>()
        .build();
    let q: Vec<EntityId> = (0..3).map(|_| world.add_entity(())).collect();
    let r = world.add_entity(());

    for (i, &id) in [q[0], q[1], q[2], r].iter().enumerate() {
        world.add_component(id, A(i as u32));
        world.add_component(id, D(i as u32));
    }
    for (i, &id) in [r, q[1], q[0], q[2]].iter().enumerate() {
        world.add_component(id, B(i as u32));
    }
    for (i, &id) in [q[2], q[0], q[1], r].iter().enumerate() {
        world.add_component(id, C(i as u32));
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

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert_eq!(
        segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]),
        [
            segment((0, 0..1), 0b111, &[Some(0), Some(2), Some(1)], &[]),
            segment((0, 1..2), 0b101, &[Some(1), None, Some(2)], &[]),
            segment((0, 2..3), 0b101, &[Some(2), None, Some(0)], &[]),
            segment((0, 3..4), 0b001, &[Some(3), None, None], &[]),
            segment(
                (1, 3..4),
                0b010,
                &[None, Some(3), None],
                &[record(0, 1, 2), record(0, 2, 3), record(0, 3, 4)]
            ),
        ]
    );

    let mut all = q;
    all.push(r);
    assert_triples(&a, &b, &c, &all);
}

/// Builds a world with AB and AC groups through public calls only, where regroup promoted
/// grouped entities into ABC after they gained a component.
///
/// A second promotion shrinks AB, so regroup slides the ABC range after it by one slot in
/// A and B, which rotates it in both.
fn promoted_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let mut all: Vec<EntityId> = (0..6).map(|i| world.add_entity((A(i), B(i)))).collect();
    all.extend((6..10).map(|i| world.add_entity((A(i), C(i)))));
    world.regroup();

    for &i in &[1, 4] {
        world.add_component(all[i], C(i as u32));
    }
    world.add_component(all[7], B(7));
    world.regroup();
    world.add_component(all[0], C(0));
    world.regroup();

    (world, all)
}

/// Asserts that after real promotions into ABC, an `(A, B)` query walks AB and ABC across
/// rings that regroup rotated by the same amount, one segment per range, yielding every match
/// once.
#[test]
fn promoted_pairs() {
    let (world, all) = promoted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    // Regroup leaves every range fully trusted and nothing unevaluated, so there's one aligned
    // walk for AB and one for ABC.
    assert_eq!(a.groups.unevaluated_start, a.dense.len());
    assert_eq!(b.groups.unevaluated_start, b.dense.len());

    // The promotions rotated ABC by the same slot in A and B, they wrap at the same point.
    // Each walk is then a single segment, starting at the start of its range in both members.
    let abc_shift = |groups: &Groups| {
        let abc = groups.ranges.iter().find(|range| range.group.len() == 3);
        abc.unwrap().shift
    };
    assert_eq!((abc_shift(&a.groups), abc_shift(&b.groups)), (1, 1));
    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 0..3), 0b11, &[Some(0), Some(0)], &[]),
            segment((0, 6..10), 0b11, &[Some(6), Some(3)], &[]),
        ]
    );

    assert_pairs(&a, &b, &all);
}

/// Asserts that after real promotions into ABC, an `(A, B, C)` query walks ABC alone:
/// no segment walks or covers A's AB and AC ranges.
#[test]
fn promoted_triples() {
    let (world, all) = promoted_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let segments = segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]);
    assert!(!segments.is_empty());
    for range in &a.groups.ranges {
        if range.group.len() == 2 {
            assert_skipped(&segments, 0, range.start..range.end());
        }
    }

    assert_triples(&a, &b, &c, &all);
}

/// A world with a fully trusted AB group of `len` entities, in the same physical order in both
/// members.
///
/// ```text
/// A: [e0 e1 .. ]
/// B: [e0 e1 .. ]
/// ```
///
/// `shifts` are A's and B's shifts. When they have the same remainder, both members hold the
/// same logical sequence, as group metadata requires.
fn aligned_world(len: usize, shifts: (isize, isize)) -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let e: Vec<EntityId> = (0..len).map(|_| world.add_entity(())).collect();

    for (i, &id) in e.iter().enumerate() {
        world.add_component(id, A(i as u32));
        world.add_component(id, B(10 + i as u32));
    }

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        set_regions(&mut a.groups, &[(0, len, shifts.0, len)], len);
        set_regions(&mut b.groups, &[(0, len, shifts.1, len)], len);
    }

    (world, e)
}

/// Asserts that members wrapping at the same nonzero point make a single segment, installed
/// without plan, walking both members from the start of their range.
#[test]
fn equal_shifts() {
    let (world, all) = aligned_world(4, (1, 1));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let iter = pair(&a, &b);
    assert!(iter.schedule.is_none());
    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [segment((0, 0..4), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that different shifts with the same remainder, one of them negative, make the same
/// single segment as equal shifts.
#[test]
fn equal_remainders() {
    let (world, all) = aligned_world(4, (-3, 5));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let iter = pair(&a, &b);
    assert!(iter.schedule.is_none());
    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [segment((0, 0..4), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that shifts at both ends of `isize` with the same remainder, 1, make a single segment
/// without overflowing.
#[test]
fn extreme_shifts() {
    let (world, all) = aligned_world(4, (isize::MIN + 1, isize::MAX - 2));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let iter = pair(&a, &b);
    assert!(iter.schedule.is_none());
    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [segment((0, 0..4), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that unrotated members still make a single segment.
#[test]
fn zero_shifts() {
    let (world, all) = aligned_world(4, (0, 0));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [segment((0, 0..4), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that a range of a single entity makes a single segment, whatever its shifts.
#[test]
fn singleton() {
    let (world, all) = aligned_world(1, (5, -7));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [segment((0, 0..1), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that an empty range with a nonzero shift isn't walked: only the unevaluated regions
/// are, B's rejecting what A's yielded.
#[test]
fn empty_range() {
    let (world, all) = aligned_world(2, (0, 0));
    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        set_regions(&mut a.groups, &[(0, 0, 3, 0)], 0);
        set_regions(&mut b.groups, &[(0, 0, 3, 0)], 0);
    }
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 0..2), 0b01, &[Some(0), None], &[]),
            segment((1, 0..2), 0b10, &[None, Some(0)], &[record(0, 0, 2)]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that an aligned walk over a trusted prefix keeps its logical positions, even with
/// equal shifts: A trusts 2 positions, the aligned walk covers logical `0..2` and A's damaged
/// walk is cut at its wrap point.
#[test]
fn partial_equal_shifts() {
    let (world, all) = aligned_world(4, (1, 1));
    world.borrow::<ViewMut<'_, A>>().unwrap().groups.ranges[0].trust = 2;
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 1..3), 0b11, &[Some(1), Some(1)], &[]),
            segment((0, 3..4), 0b01, &[Some(3), None], &[]),
            segment((0, 0..1), 0b01, &[Some(0), None], &[]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that a damaged walk over A's whole ring keeps its logical positions: B trusts
/// nothing, so A's damaged walk covers logical `0..4` alone and is cut at A's wrap point.
#[test]
fn damaged_whole() {
    let (world, all) = aligned_world(4, (1, 1));
    world.borrow::<ViewMut<'_, B>>().unwrap().groups.ranges[0].trust = 0;
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 1..4), 0b01, &[Some(1), None], &[]),
            segment((0, 0..1), 0b01, &[Some(0), None], &[]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that mutable outputs of a single rebased segment each write their own entity's
/// components exactly once.
#[test]
fn equal_shifts_mut() {
    let (world, all) = aligned_world(4, (1, 1));
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();

    let iter = grouped((&mut a, &mut b).into_shiperator());
    assert!(iter.schedule.is_none());
    iter.for_each(|(a, b)| {
        a.0 += 1000;
        b.0 += 1000;
    });

    for (i, &id) in (0..).zip(&all) {
        assert_eq!((&a).get(id).unwrap().0, 1000 + i);
        assert_eq!((&b).get(id).unwrap().0, 1010 + i);
    }
}

/// A world with a fully trusted ABC group of `len` entities, each member's logical 0 at its
/// own physical `offsets`.
///
/// Logical position `k` holds `e_k` in every member, as group metadata requires: a member with
/// offset `s` holds it at physical `(s + k) mod len`.
fn shifted_world(len: usize, offsets: [usize; 3]) -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test().with_group::<(A, B, C)>().build();
    let e: Vec<EntityId> = (0..len).map(|_| world.add_entity(())).collect();

    // Physical `j` holds logical `(j - s) mod len`, components are added in physical order.
    let logical = |offset: usize, physical: usize| (physical + len - offset) % len;
    for j in 0..len {
        let k = logical(offsets[0], j);
        world.add_component(e[k], A(k as u32));
    }
    for j in 0..len {
        let k = logical(offsets[1], j);
        world.add_component(e[k], B(k as u32));
    }
    for j in 0..len {
        let k = logical(offsets[2], j);
        world.add_component(e[k], C(k as u32));
    }

    {
        let (mut a, mut b, mut c) = world
            .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
            .unwrap();
        let shift = |offset: usize| isize::try_from(offset).unwrap();
        set_regions(&mut a.groups, &[(0, len, shift(offsets[0]), len)], len);
        set_regions(&mut b.groups, &[(0, len, shift(offsets[1]), len)], len);
        set_regions(&mut c.groups, &[(0, len, shift(offsets[2]), len)], len);
    }

    (world, e)
}

/// Asserts that shifts 10, 10 and 11 over 100 entities make 2 segments: A and B wrap at the
/// same point, C one position later.
#[test]
fn one_late() {
    let (world, all) = shifted_world(100, [10, 10, 11]);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert_eq!(
        segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]),
        [
            segment((0, 0..99), 0b111, &[Some(0), Some(0), Some(1)], &[]),
            segment((0, 99..100), 0b111, &[Some(99), Some(99), Some(0)], &[]),
        ]
    );
    assert_triples(&a, &b, &c, &all);
}

/// Asserts that 3 distinct wrap points make 3 segments.
#[test]
fn distinct_shifts() {
    let (world, all) = shifted_world(4, [1, 2, 3]);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert_eq!(
        segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]),
        [
            segment((0, 0..2), 0b111, &[Some(0), Some(1), Some(2)], &[]),
            segment((0, 2..3), 0b111, &[Some(2), Some(3), Some(0)], &[]),
            segment((0, 3..4), 0b111, &[Some(3), Some(0), Some(1)], &[]),
        ]
    );
    assert_triples(&a, &b, &c, &all);
}

/// Asserts that a wrap point shared by B and C counts once: 2 distinct wrap points make 2
/// segments.
#[test]
fn repeated_shifts() {
    let (world, all) = shifted_world(4, [1, 3, 3]);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert_eq!(
        segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]),
        [
            segment((0, 0..2), 0b111, &[Some(0), Some(2), Some(2)], &[]),
            segment((0, 2..4), 0b111, &[Some(2), Some(0), Some(0)], &[]),
        ]
    );
    assert_triples(&a, &b, &c, &all);
}

/// Asserts that a zero shift on B, which doesn't drive, still gives one segment per distinct
/// wrap point: 2.
#[test]
fn zero_shift_member() {
    let (world, all) = shifted_world(4, [2, 0, 2]);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert_eq!(
        segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]),
        [
            segment((0, 0..2), 0b111, &[Some(0), Some(2), Some(0)], &[]),
            segment((0, 2..4), 0b111, &[Some(2), Some(0), Some(2)], &[]),
        ]
    );
    assert_triples(&a, &b, &c, &all);
}

/// A world with fully trusted AB and ABC groups, holding `p` in AB and `q0`, `q1` and `q2`
/// in ABC.
///
/// ```text
///          AB    ABC
/// A, B:    [p]   [q0 q1 q2]   ABC's logical 0 at its physical 1
/// C:             [.. .. ..]   ABC's logical 0 at physical `c_offset`
/// ```
///
/// Every member holds ABC's logical sequence `[q1 q2 q0]`, as group metadata requires.
/// C's ABC range starts at 0, A's and B's at 1.
fn nested_world(c_offset: usize) -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, B, C)>()
        .build();
    let p = world.add_entity(());
    let q: Vec<EntityId> = (0..3).map(|_| world.add_entity(())).collect();

    for (i, &id) in [p, q[0], q[1], q[2]].iter().enumerate() {
        world.add_component(id, A(i as u32));
        world.add_component(id, B(i as u32));
    }
    // C's physical `j` holds logical `(j - c_offset) mod 3`.
    let sequence = [q[1], q[2], q[0]];
    for j in 0..3 {
        let id = sequence[(j + 3 - c_offset) % 3];
        world.add_component(id, C(j as u32));
    }

    {
        let (mut a, mut b, mut c) = world
            .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
            .unwrap();
        let c_shift = isize::try_from(c_offset).unwrap();
        set_regions(&mut a.groups, &[(0, 1, 0, 1), (1, 3, 1, 3)], 4);
        set_regions(&mut b.groups, &[(0, 1, 0, 1), (1, 3, 1, 3)], 4);
        set_regions(&mut c.groups, &[(0, 3, c_shift, 3)], 3);
    }

    let mut all = q;
    all.push(p);

    (world, all)
}

/// Asserts that members wrapping at the same point from different range starts make a single
/// segment, each captain starting at its own range's start.
#[test]
fn different_starts() {
    let (world, all) = nested_world(1);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let iter = triple(&a, &b, &c);
    assert!(iter.schedule.is_none());
    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense, &c.dense]),
        [segment((0, 1..4), 0b111, &[Some(1), Some(1), Some(0)], &[])]
    );
    assert_triples(&a, &b, &c, &all);
}

/// Asserts that an `(A, B)` query walks ABC as a single segment when A and B wrap at the same
/// point, while C, which it doesn't borrow, wraps elsewhere.
#[test]
fn unqueried_shift() {
    let (world, all) = nested_world(2);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 0..1), 0b11, &[Some(0), Some(0)], &[]),
            segment((0, 1..4), 0b11, &[Some(1), Some(1)], &[]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// A world with AB and ABCD groups, where D was reset, holding `p` in AB and `q0`, `q1` and
/// `q2` in ABCD.
///
/// ```text
///          AB    ABCD
/// A, B:    [p]   [q0 q1 q2]   ABCD's logical 0 at its physical 1
/// C:             [q2 q0 q1]   logical 0 at physical 2
/// D:             []           every component unevaluated
/// ```
///
/// Every member holds ABCD's logical sequence `[q1 q2 q0]`, C trusting all of it.\
/// No query borrows D, its reset doesn't change what the others' metadata says.
fn extra_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, B, C, D)>()
        .build();
    let p = world.add_entity(());
    let q: Vec<EntityId> = (0..3).map(|_| world.add_entity(())).collect();

    for (i, &id) in [p, q[0], q[1], q[2]].iter().enumerate() {
        world.add_component(id, A(i as u32));
        world.add_component(id, B(i as u32));
    }
    for (i, &id) in [q[2], q[0], q[1]].iter().enumerate() {
        world.add_component(id, C(i as u32));
    }
    for (i, &id) in q.iter().enumerate() {
        world.add_component(id, D(i as u32));
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
        set_regions(&mut a.groups, &[(0, 1, 0, 1), (1, 3, 1, 3)], 4);
        set_regions(&mut b.groups, &[(0, 1, 0, 1), (1, 3, 1, 3)], 4);
        set_regions(&mut c.groups, &[(0, 3, 2, 3)], 3);
        set_regions(&mut d.groups, &[(0, 0, 0, 0)], 0);
    }

    let mut all = q;
    all.push(p);

    (world, all)
}

/// Asserts that C, reading ABCD directly without being required, counts as a captain when the
/// walk moves its logical 0: A and B wrap at the same point and C elsewhere, ABCD makes 2
/// segments with C at its own physical indices.
#[test]
fn extra_captain_shift() {
    let (world, all) = extra_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert_eq!(
        segments(&triple(&a, &b, &c), &[&a.dense, &b.dense, &c.dense]),
        [
            // AB, C is looked up.
            segment((0, 0..1), 0b011, &[Some(0), Some(0), None], &[]),
            // ABCD, C reads directly over all of it.
            segment((0, 1..3), 0b111, &[Some(1), Some(1), Some(1)], &[]),
            segment((0, 3..4), 0b111, &[Some(3), Some(3), Some(0)], &[]),
        ]
    );
    assert_triples(&a, &b, &c, &all);
}
