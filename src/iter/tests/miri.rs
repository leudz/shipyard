//! Focused Miri scenarios for iteration.
//!
//! Iterators resolve raw pointers into the storages they borrow, segment after segment, and
//! share their plan between split halves. These tests hold yielded references across segment
//! transitions and splits, then use them, so a stale pointer, an aliasing `&mut` or a plan
//! released too early shows up as a wrong value or a Miri error.

use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::iter::plan::NodeState;
use crate::iter::planner::regions::grouped_entities;
use crate::iter::planner::{ConstructionMetadata, NodeMetadata};
use crate::iter::{
    IntoIter, IntoIterRef, IntoShiperator, IterComponent, Mixed, OneOfTwo, RawEntityIdAccess,
    Shiper, Shiperator, is_exact_traversal,
};
use crate::sparse_set::FullRawWindow;
use crate::track;
use crate::views::{AllStoragesView, View, ViewMut};
use crate::world::World;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::cell::Cell;

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

struct Tracked(u32);

impl Component for Tracked {
    type Tracking = track::All;
}

struct Zst;

impl Component for Zst {
    type Tracking = track::Untracked;
}

/// Component aligned more strictly than any of its fields need.
#[repr(align(64))]
struct Aligned(u64);

impl Component for Aligned {
    type Tracking = track::Untracked;
}

/// Counts how many times it's dropped.
#[cfg(feature = "std")]
struct Dropped<'a>(&'a Cell<usize>);

#[cfg(feature = "std")]
impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

/// Iterator over the pair `(A, B)`.
type PairIter<'a> = Shiper<
    Mixed<(
        FullRawWindow<'a, A, track::Untracked>,
        FullRawWindow<'a, B, track::Untracked>,
    )>,
>;

/// Builds the iterator walking the grouped alternative `query`'s constructor reported, even
/// when the ordinary walk is cheaper. Our worlds are too small for grouping to win on its own.
fn grouped<S: Shiperator>(query: (S, RawEntityIdAccess, ConstructionMetadata)) -> Shiper<S> {
    let (mut mixed, _, metadata) = query;
    let NodeMetadata::GroupedMixed(selection) = metadata.0 else {
        panic!("the query has a grouped alternative");
    };
    let alternative = selection.cheapest().unwrap();

    let entities = grouped_entities(&mut mixed, alternative);

    // SAFE: `grouped_entities` installed its first segment on `mixed` and returns that
    // segment's entities, with follow-ups whose rows it built for `mixed`'s layout.
    unsafe { Shiper::new(mixed, entities) }
}

/// Builds `AllStorages::iter`'s wrapper around the grouped alternative `T` reported, like
/// `grouped` does for borrowed queries.
fn grouped_owned<'a, T: IterComponent>(all_storages: &'a AllStorages) -> IntoIterRef<'a, T>
where
    T::Shiperator<'a>: Shiperator,
{
    let current = all_storages.get_current();
    let (mut mixed, all_borrow, borrow, _, metadata) =
        T::into_shiperator(all_storages, None, current).unwrap();
    let NodeMetadata::GroupedMixed(selection) = metadata.0 else {
        panic!("the query has a grouped alternative");
    };
    let alternative = selection.cheapest().unwrap();

    let entities = grouped_entities(&mut mixed, alternative);
    let is_exact_sized = is_exact_traversal(&mixed, &entities);

    IntoIterRef {
        shiperator: mixed,
        _all_borrow: all_borrow,
        _borrow: borrow,
        entities,
        is_exact_sized,
        phantom: core::marker::PhantomData,
    }
}

/// Returns the `(captain_mask, coverage records, candidates)` of every segment of `iter`'s
/// plan, read from the root `Mixed`'s state, or from the child at `nested` for a nested query.
fn route<S: Shiperator>(iter: &Shiper<S>, nested: Option<usize>) -> Vec<(u32, usize, usize)> {
    let plan = iter
        .schedule
        .as_ref()
        .expect("grouped walks have a plan")
        .plan();

    plan.segments()
        .iter()
        .map(|segment| {
            let row = plan.row(segment);
            let NodeState::Mixed {
                captain_mask,
                coverage,
            } = row[nested.map_or(0, |child| child + 1)]
            else {
                panic!("grouped walks drive a Mixed");
            };

            (captain_mask, coverage.len, segment.len())
        })
        .collect()
}

/// `e0`, `e2` and `e4` own `A(i)` and `B(10 + i)`, `e1` only owns `A(1)` and `e3` only `B(13)`.
fn ordinary_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let e = (0..5)
        .map(|i| match i {
            1 => world.add_entity(A(i)),
            3 => world.add_entity(B(10 + i)),
            _ => world.add_entity((A(i), B(10 + i))),
        })
        .collect();

    (world, e)
}

/// An AB group whose ranges have different lengths and are partially trusted.
///
/// ```text
///    range                      ungrouped   unevaluated
/// A: [e0 e1 e2 damaged late_b]              [unevaluated late_a]
///    trust 4
/// B: [e0 e1 e2 damaged]         [late_a]    [late_b unevaluated]
///    trust 3
/// ```
///
/// Both members hold the same entities over the 3 positions they both trust, and every entity
/// with both components evaluated sits in both ranges, as group metadata guarantees.
/// `late_a`'s B was evaluated as ungrouped before it gained an A.\
/// Every entity owns `A(i)` and `B(10 + i)`, in the order returned:
/// `e0`, `e1`, `e2`, `damaged`, `late_b`, `unevaluated` then `late_a`.
fn partial_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    world.group::<(A, B)>();

    // Components land at the end of their storage in the order they're added,
    // we set the regions afterwards.
    let mut e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(10 + i))))
        .collect();
    let late_a = world.add_entity(B(16));
    e.push(world.add_entity((A(4), B(14))));
    e.push(world.add_entity((A(5), B(15))));
    world.add_component(late_a, A(6));
    e.push(late_a);

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        let (a_range, b_range) = (&mut a.groups.ranges[0], &mut b.groups.ranges[0]);
        (a_range.start, a_range.len, a_range.trust) = (0, 5, 4);
        (b_range.start, b_range.len, b_range.trust) = (0, 4, 3);
        a.groups.unevaluated_start = 5;
        b.groups.unevaluated_start = 5;
    }

    (world, e)
}

/// The segments of `partial_world`'s grouped walks: the aligned prefix both members drive, B's
/// damaged suffix alone, then A's unevaluated region covering B's damaged walk, and B's covering
/// A's unevaluated walk.
const PARTIAL_ROUTE: [(u32, usize, usize); 4] =
    [(0b11, 0, 3), (0b10, 0, 1), (0b01, 1, 2), (0b10, 1, 2)];

/// Asserts that `A` and `B` of the entities of `e` hold `A(i + 100)` and `B(10 + i + 100)`:
/// every one was written exactly once.
fn assert_written(world: &World, e: &[EntityId]) {
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    for (i, &entity) in (0..).zip(e) {
        assert_eq!(a.get(entity), Ok(&A(i + 100)), "{entity:?}");
        assert_eq!(b.get(entity), Ok(&B(10 + i + 100)), "{entity:?}");
    }
}

/// Asserts that mutable items of an ordinary traversal can all be held at once while sparse
/// lookups reject the entities missing B, that they're written through afterwards, and that a
/// query over an empty storage yields nothing.
#[test]
fn ordinary_held_mut() {
    let (world, e) = ordinary_world();
    let (mut a, b, c) = world
        .borrow::<(ViewMut<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    assert!((&mut a, &c).iter().next().is_none());

    let items: Vec<(&mut A, &B)> = (&mut a, &b).iter().collect();
    assert_eq!(items.len(), 3);
    for (a, b) in items {
        a.0 += b.0;
    }

    let values: Vec<u32> = [0, 1, 2, 4].map(|i| a.get(e[i]).unwrap().0).into();
    assert_eq!(values, [10, 1, 14, 18]);
}

/// Asserts that an owned wrapper hands each iterator its own reborrowed clone over a grouped plan
/// with coverage: a first iterator dropped inside a covered segment leaves the plan to the next
/// one, whose mutable items are all held then written, and a last one finds every entity again.
#[test]
fn owned_reborrow() {
    let (world, e) = partial_world();

    {
        let all_storages = world.borrow::<AllStoragesView<'_>>().unwrap();
        let mut iter = grouped_owned::<(&mut A, &mut B)>(&all_storages);

        // The aligned prefix and B's damaged suffix hold 4 candidates, the 5th item comes from
        // the first segment reading coverage.
        let mut first = iter.iter();
        assert_eq!(route(&first, None), PARTIAL_ROUTE);
        assert!(first.nth(4).is_some());
        drop(first);

        let items: Vec<(&mut A, &mut B)> = iter.iter().collect();
        assert_eq!(items.len(), e.len());
        for (a, b) in items {
            a.0 += 100;
            b.0 += 100;
        }

        let mut ids: Vec<EntityId> = iter.iter().with_id().map(|(id, _)| id).collect();
        ids.sort_unstable();
        let mut expected = e.clone();
        expected.sort_unstable();
        assert_eq!(ids, expected);
    }

    assert_written(&world, &e);
}

/// Asserts that an `Or` yields A's entities then B's remaining ones, and that references from
/// the first branch stay valid once the iterator switched to the second.
#[test]
fn or_branches() {
    let (world, e) = ordinary_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let items: Vec<(EntityId, OneOfTwo<&A, &B>)> = (&a | &b).iter().with_id().collect();

    let values: Vec<(EntityId, u32)> = items
        .iter()
        .map(|(entity, item)| match item {
            OneOfTwo::One(a) => (*entity, a.0),
            OneOfTwo::Two(b) => (*entity, b.0),
        })
        .collect();
    assert_eq!(
        values,
        [(e[0], 0), (e[1], 1), (e[2], 2), (e[4], 4), (e[3], 13)]
    );
}

/// Asserts that a `Not` filter rejects the entities owning B, and that a mutable `Optional`
/// hands out held `&mut B` for the entities owning one and `None` for the others.
#[test]
fn not_and_optional() {
    let (world, e) = ordinary_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();

    let lonely: Vec<EntityId> = (&a, !&b).iter().with_id().map(|(id, _)| id).collect();
    assert_eq!(lonely, [e[1]]);

    let items: Vec<(&mut A, Option<&mut B>)> = (&mut a, b.as_optional()).iter().collect();
    assert_eq!(items.len(), 4);
    for (a, b) in items {
        match b {
            Some(b) => b.0 += a.0,
            None => a.0 += 100,
        }
    }

    assert_eq!(a.get(e[1]).unwrap().0, 101);
    assert_eq!(b.get(e[4]).unwrap().0, 18);
}

/// Asserts that a tracking filter reads each candidate's timestamp at the right index: only the
/// components inserted after clearing are yielded, writing through them marks exactly those as
/// modified.
#[test]
fn tracking_filter() {
    let mut world = World::new_for_test();
    let old: Vec<EntityId> = (0..2).map(|i| world.add_entity(Tracked(i))).collect();
    world.run(|tracked: ViewMut<'_, Tracked, track::All>| tracked.clear_all_inserted());
    let new: Vec<EntityId> = (2..4).map(|i| world.add_entity(Tracked(i))).collect();

    let mut tracked = world.borrow::<ViewMut<'_, Tracked, track::All>>().unwrap();
    let mut items: Vec<_> = tracked.inserted_mut().iter().collect();
    assert_eq!(items.len(), 2);
    for item in &mut items {
        item.0 += 10;
    }
    drop(items);

    for entity in old {
        assert!(!tracked.is_modified(entity));
    }
    for (i, &entity) in (2..).zip(&new) {
        assert!(tracked.is_modified(entity));
        assert_eq!(tracked.get(entity).unwrap().0, i + 10);
    }
}

/// Asserts that grouped walks over aligned, damaged and unevaluated regions, where coverage
/// rejects what earlier walks own, yield every entity once. Mutable items of every segment are
/// held until the end, then written.
#[test]
fn grouped_held_mut() {
    let (world, e) = partial_world();

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        let iter = grouped((&mut a, &mut b).into_shiperator());
        assert_eq!(route(&iter, None), PARTIAL_ROUTE);

        let items: Vec<(&mut A, &mut B)> = iter.collect();
        assert_eq!(items.len(), e.len());
        for (a, b) in items {
            a.0 += 100;
            b.0 += 100;
        }
    }

    assert_written(&world, &e);
}

/// Asserts that an aligned walk over a ring whose range wraps around in A, but not in C, is cut
/// at A's wrap point, and that items held from the first piece stay valid after the second.
#[test]
fn ring_wrap() {
    let mut world = World::new_for_test();
    world.group::<(A, B)>().group::<(A, C)>();
    // The AC range is filled first. Appending to the AB range, which comes before it in A,
    // then rotates it in A only.
    let e: Vec<EntityId> = (0..3).map(|i| world.add_entity((A(i), C(i)))).collect();
    world.regroup();
    world.add_entity((A(9), B(9)));
    world.regroup();

    {
        let (mut a, c) = world.borrow::<(ViewMut<'_, A>, View<'_, C>)>().unwrap();
        let iter = grouped((&mut a, &c).into_shiperator());
        assert_eq!(route(&iter, None), [(0b11, 0, 1), (0b11, 0, 2)]);

        let items: Vec<(&mut A, &C)> = iter.collect();
        assert_eq!(items.len(), 3);
        for (a, c) in items {
            a.0 += 100 + c.0;
        }
    }

    let a = world.borrow::<View<'_, A>>().unwrap();
    for (i, &entity) in (0..).zip(&e) {
        assert_eq!(a.get(entity), Ok(&A(2 * i + 100)));
    }
}

/// Asserts that a nested pair driving `((A, B), C)` moves through its aligned walk, then its
/// unevaluated walks where its own coverage rejects the duplicate, while every mutable item is
/// held until the end.
#[test]
fn nested_held_mut() {
    let mut world = World::new_for_test();
    let mut e: Vec<EntityId> = (0..3)
        .map(|i| world.add_entity((A(i), B(10 + i), C(i))))
        .collect();
    world.group::<(A, B)>();
    e.push(world.add_entity((A(3), B(13), C(3))));

    {
        let (mut a, mut b, c) = world
            .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, View<'_, C>)>()
            .unwrap();
        let iter = grouped(((&mut a, &mut b), &c).into_shiperator());
        assert_eq!(
            route(&iter, Some(0)),
            [(0b11, 0, 3), (0b01, 0, 1), (0b10, 1, 1)]
        );

        let items: Vec<((&mut A, &mut B), &C)> = iter.collect();
        assert_eq!(items.len(), e.len());
        for ((a, b), c) in items {
            assert_eq!(a.0, c.0);
            a.0 += 100;
            b.0 += 100;
        }
    }

    assert_written(&world, &e);
}

/// Asserts that splitting a partially consumed segment shares the plan between the halves and
/// keeps every item: the right half, holding the follow-ups, is consumed first, and mutable
/// items from both halves and the consumed prefix are held at once, then written.
#[test]
fn split_halves_mut() {
    let (world, e) = partial_world();

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        let mut iter = grouped((&mut a, &mut b).into_shiperator());
        let plan = iter.schedule.as_ref().unwrap().downgrade();

        let mut items: Vec<(&mut A, &mut B)> = iter.by_ref().take(1).collect();
        let (left, right) = iter.split_current_segment();
        let right = right.expect("2 candidates are left in the aligned segment");
        assert_eq!((left.cursor.len(), right.cursor.len()), (1, 1));
        assert_eq!(plan.strong_count(), 2);

        items.extend(right);
        items.extend(left);
        assert!(plan.upgrade().is_none());

        assert_eq!(items.len(), e.len());
        for (a, b) in items {
            a.0 += 100;
            b.0 += 100;
        }
    }

    assert_written(&world, &e);
}

/// Returns `partial_world`'s grouped iterator once it entered A's unevaluated segment, which
/// reads coverage from the plan and is followed by B's.
fn in_covered_segment<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> PairIter<'a> {
    let mut iter = grouped((a, b).into_shiperator());

    assert_eq!(iter.by_ref().take(4).count(), 4);
    assert!(iter.enter_next_segment());
    assert_eq!(route(&iter, None)[2], (0b01, 1, iter.cursor.len()));

    iter
}

/// Returns the sorted ids `iter` yields.
fn ids(iter: PairIter<'_>) -> Vec<EntityId> {
    let mut ids: Vec<EntityId> = iter.with_id().map(|(id, _)| id).collect();
    ids.sort_unstable();
    ids
}

/// Asserts that after splitting a segment reading coverage, either half can be dropped first
/// while the other still reads the plan: the survivor yields its own entities, the coverage
/// still rejecting the duplicate, and the plan is released with it.
#[test]
fn split_drop_either() {
    let (world, e) = partial_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (unevaluated, late_a, late_b) = (e[5], e[6], e[4]);

    for drop_left in [false, true] {
        let (left, right) = in_covered_segment(&a, &b).split_current_segment_at(1);
        let plan = left.schedule.as_ref().unwrap().downgrade();
        assert_eq!(plan.strong_count(), 2);

        let (discarded, survivor, mut expected) = if drop_left {
            (left, right, Vec::from([late_a, late_b]))
        } else {
            (right, left, Vec::from([unevaluated]))
        };
        drop(discarded);
        assert_eq!(plan.strong_count(), 1);

        expected.sort_unstable();
        assert_eq!(ids(survivor), expected);
        assert!(plan.upgrade().is_none());
    }
}

/// Asserts that every bulk consumer goes through all segments and yields the same entities as
/// `next`, mutable `for_each` writing each match once.
#[test]
fn bulk_consumers() {
    let (world, e) = partial_world();
    let mut expected = e.clone();
    expected.sort_unstable();

    {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let query = || grouped((&a, &b).into_shiperator());

        let mut folded = query()
            .with_id()
            .fold(Vec::new(), |mut ids, (id, (a_item, b_item))| {
                assert_eq!(a_item.0 + 10, b_item.0);
                ids.push(id);
                ids
            });
        folded.sort_unstable();
        assert_eq!(folded, expected);

        let mut visited = Vec::new();
        query().with_id().for_each(|(id, _)| visited.push(id));
        visited.sort_unstable();
        assert_eq!(visited, expected);

        assert_eq!(query().fold(0, |count, _| count + 1), e.len());
    }

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        grouped((&mut a, &mut b).into_shiperator()).for_each(|(a, b)| {
            a.0 += 100;
            b.0 += 100;
        });
    }

    assert_written(&world, &e);
}

/// Asserts that dropping an iterator partway through a segment that reads coverage releases its
/// plan exactly once.
#[test]
fn early_drop() {
    let (world, _) = partial_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let mut iter = in_covered_segment(&a, &b);
    let plan = iter.schedule.as_ref().unwrap().downgrade();
    assert!(iter.next().is_some());

    drop(iter);
    assert!(plan.upgrade().is_none());
}

/// Asserts that a `fold` callback panicking in a later segment drops the accumulator's items,
/// the callback's capture and the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn fold_panic() {
    let (world, _) = partial_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let item_drops = Cell::new(0);
    let capture_drops = Cell::new(0);
    let iter = grouped((&a, &b).into_shiperator());
    let plan = iter.schedule.as_ref().unwrap().downgrade();
    let panic_on = A(5);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let capture = Dropped(&capture_drops);
        let item_drops = &item_drops;

        iter.fold(Vec::new(), move |mut acc, (a, _)| {
            let _ = &capture;
            if *a == panic_on {
                panic!("fold callback panicked");
            }
            acc.push(Dropped(item_drops));
            acc
        })
    }));

    assert!(result.is_err());
    // `unevaluated`, holding `A(5)`, is the first entity of A's unevaluated segment. The 4
    // entities of the aligned and damaged segments were pushed and dropped with the accumulator.
    assert_eq!(item_drops.get(), 4);
    assert_eq!(capture_drops.get(), 1);
    assert!(plan.upgrade().is_none());
}

/// Asserts that a `for_each` callback with ids panicking in a later segment drops its capture
/// and the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn for_each_panic() {
    let (world, e) = partial_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let capture_drops = Cell::new(0);
    let iter = grouped((&a, &b).into_shiperator());
    let plan = iter.schedule.as_ref().unwrap().downgrade();
    let late_b = e[4];

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let capture = Dropped(&capture_drops);

        iter.with_id().for_each(move |(id, _)| {
            let _ = &capture;
            if id == late_b {
                panic!("for_each callback panicked");
            }
        });
    }));

    assert!(result.is_err());
    assert_eq!(capture_drops.get(), 1);
    assert!(plan.upgrade().is_none());
}

/// Asserts that grouped walks hand out zero-sized and over-aligned mutable items, every write
/// landing on an aligned component of the right entity.
#[test]
fn zero_sized_and_aligned() {
    let mut world = World::new_for_test();
    world.group::<(Zst, Aligned)>();
    let e: Vec<EntityId> = (0..3)
        .map(|i| world.add_entity((Zst, Aligned(i))))
        .collect();
    world.regroup();

    {
        let (mut zst, mut aligned) = world
            .borrow::<(ViewMut<'_, Zst>, ViewMut<'_, Aligned>)>()
            .unwrap();
        let iter = grouped((&mut zst, &mut aligned).into_shiperator());
        assert_eq!(route(&iter, None), [(0b11, 0, 3)]);

        let items: Vec<(&mut Zst, &mut Aligned)> = iter.collect();
        for (_, aligned) in items {
            assert_eq!(core::ptr::from_ref(&*aligned) as usize % 64, 0);
            aligned.0 += 100;
        }
    }

    let aligned = world.borrow::<View<'_, Aligned>>().unwrap();
    for (i, &entity) in (0..).zip(&e) {
        assert_eq!(aligned.get(entity).unwrap().0, i + 100);
    }
}

/// Component that is neither `Send` nor `Sync`.
#[cfg(feature = "thread_local")]
struct Local(u32, core::marker::PhantomData<alloc::rc::Rc<()>>);

#[cfg(feature = "thread_local")]
impl Component for Local {
    type Tracking = track::Untracked;
}

/// Asserts the owned iteration of a `!Send + !Sync` component: `World::iter` over
/// `NonSendSync<&mut Local>` keeps its storage borrowed, its mutable items can all be held and
/// written, and the storage is released once the iterator is dropped.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_owned() {
    use crate::borrow::NonSendSync;
    use crate::views::EntitiesViewMut;

    let world = World::new_for_test();
    let e: Vec<EntityId> = world.run(
        |mut entities: EntitiesViewMut<'_>,
         mut locals: NonSendSync<ViewMut<'_, Local>>,
         mut a: ViewMut<'_, A>| {
            (0..3)
                .map(|i| {
                    let components = (Local(i, core::marker::PhantomData), A(i));
                    entities.add_entity((&mut *locals, &mut a), components)
                })
                .collect()
        },
    );

    {
        let mut iter = world.iter::<(NonSendSync<&mut Local>, &A)>();
        let items: Vec<(&mut Local, &A)> = (&mut iter).into_iter().collect();
        assert_eq!(items.len(), 3);
        for (local, a) in items {
            local.0 += 100 + a.0;
        }
        assert!(world.borrow::<NonSendSync<View<'_, Local>>>().is_err());
    }

    let locals = world.borrow::<NonSendSync<View<'_, Local>>>().unwrap();
    for (i, &entity) in (0..).zip(&e) {
        assert_eq!((&*locals).get(entity).unwrap().0, 2 * i + 100);
    }
}

/// Asserts that a whole-traversal split cutting inside a later segment shares the plan and keeps
/// every item: the right half, a clone that entered A's covered unevaluated walk partway, is
/// consumed first, and mutable items from both halves and the consumed prefix are held at once.
#[test]
fn split_remaining_mut() {
    let (world, e) = partial_world();

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        let mut iter = grouped((&mut a, &mut b).into_shiperator());
        let plan = iter.schedule.as_ref().unwrap().downgrade();

        // 1 aligned candidate is left, then B's damaged walk and A's and B's unevaluated walks.
        let mut items: Vec<(&mut A, &mut B)> = iter.by_ref().take(2).collect();
        let (left, right) = iter.split_remaining();
        let right = right.expect("6 candidates are left");
        assert_eq!(left.cursor.len(), 1);
        assert_eq!(right.cursor.local_index(), 1);
        assert_eq!(right.shiperator.coverage.len(), 1);
        assert_eq!(plan.strong_count(), 2);

        items.extend(right);
        items.extend(left);
        assert!(plan.upgrade().is_none());

        assert_eq!(items.len(), e.len());
        for (a, b) in items {
            a.0 += 100;
            b.0 += 100;
        }
    }

    assert_written(&world, &e);
}

/// Splits `partial_world`'s grouped walks over their whole traversal once 2 items are consumed:
/// the cut falls inside A's unevaluated walk, which reads coverage, and both halves walk part of it.
fn split_in_covered_segment<'a>(
    a: &'a View<'_, A>,
    b: &'a View<'_, B>,
) -> (PairIter<'a>, PairIter<'a>) {
    let mut iter = grouped((a, b).into_shiperator());
    assert_eq!(iter.by_ref().take(2).count(), 2);

    let (left, right) = iter.split_remaining();
    let right = right.expect("6 candidates are left");
    assert_eq!(right.shiperator.coverage.len(), 1);

    (left, right)
}

/// Asserts that after a whole-traversal split inside a segment reading coverage, either half can
/// be dropped first while the other still reads the plan: the survivor yields its own entities,
/// the coverage still rejecting the duplicate, and the plan is released with it.
#[test]
fn split_remaining_drop_either() {
    let (world, _) = partial_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (left, right) = split_in_covered_segment(&a, &b);
    let (left_alone, right_alone) = (ids(left), ids(right));

    for drop_left in [false, true] {
        let (left, right) = split_in_covered_segment(&a, &b);
        let plan = left.schedule.as_ref().unwrap().downgrade();

        let (survivor, expected) = if drop_left {
            drop(left);
            (ids(right), &right_alone)
        } else {
            drop(right);
            (ids(left), &left_alone)
        };

        assert_eq!(&survivor, expected, "drop left {drop_left}");
        assert!(plan.upgrade().is_none());
    }
}
