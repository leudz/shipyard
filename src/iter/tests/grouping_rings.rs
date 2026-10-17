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
use crate::iter::{IntoShiperator, Mixed, Shiper};
use crate::sparse_set::{FullRawWindow, Groups};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
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
    let mut world = World::new();
    world.group::<(A, B)>();
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

/// Asserts that an aligned walk is cut at each member's own wrap point, every captain
/// starting each piece at its own physical index, and that no record is needed.
#[test]
fn wrap_points() {
    let (world, all) = rotated_world((1, 2));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        segments(&pair(&a, &b), &[&a.dense, &b.dense]),
        [
            segment((0, 1..3), 0b11, &[Some(1), Some(2)], &[]),
            segment((0, 3..4), 0b11, &[Some(3), Some(0)], &[]),
            segment((0, 0..1), 0b11, &[Some(0), Some(1)], &[]),
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
            segment((0, 1..3), 0b11, &[Some(1), Some(2)], &[]),
            segment((0, 3..4), 0b11, &[Some(3), Some(0)], &[]),
            segment((0, 0..1), 0b11, &[Some(0), Some(1)], &[]),
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
    let mut world = World::new();
    world.group::<(A, B)>();
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
    let mut world = World::new();
    world.group::<(A, B)>().group::<(A, B, C, D)>();
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
    let mut world = World::new();
    world.group::<(A, B)>().group::<(A, B, C, D)>();
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
fn promoted_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let mut all: Vec<EntityId> = (0..6).map(|i| world.add_entity((A(i), B(i)))).collect();
    all.extend((6..10).map(|i| world.add_entity((A(i), C(i)))));
    world.group::<(A, B)>().group::<(A, C)>();

    for &i in &[1, 4] {
        world.add_component(all[i], C(i as u32));
    }
    world.add_component(all[7], B(7));
    world.regroup();

    (world, all)
}

/// Asserts that after real promotions into ABC, an `(A, B)` query walks AB and ABC across
/// rings that regroup rotated, yielding every match once.
#[test]
fn promoted_pairs() {
    let (world, all) = promoted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    // Regroup leaves every range fully trusted and nothing unevaluated, so there's one aligned
    // walk for AB and one for ABC. Any other segment comes from a cut at a wrap point.
    assert_eq!(a.groups.unevaluated_start, a.dense.len());
    assert_eq!(b.groups.unevaluated_start, b.dense.len());
    assert!(segments(&pair(&a, &b), &[&a.dense, &b.dense]).len() > 2);

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
