//! Grouped walks picked by the normal constructors, through public construction only.
//!
//! Worlds are grouped with `World::group` and `World::regroup`, then queried as usual.
//! Expected items are looked up entity by entity.\
//! Which walk won is checked by the library's `grouping_automatic` tests. Here, `size_hint`
//! tells it apart when it can: a grouped walk counts its own candidates, an ordinary walk
//! counts its captain's, and only a fully aligned group knows its exact length.

use super::ways::every_way;
use shipyard::advanced::Mut;
use shipyard::iter::OneOfTwo;
use shipyard::*;

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

#[derive(Debug, PartialEq)]
struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::Modification;
}

struct Zst0;
impl Component for Zst0 {
    type Tracking = track::Untracked;
}

struct Zst1;
impl Component for Zst1 {
    type Tracking = track::Untracked;
}

fn sorted<T: Ord>(mut items: Vec<T>) -> Vec<T> {
    items.sort_unstable();
    items
}

/// Adds `count` entities owning `A(start + i)` and `B(start + i)`.
fn add_ab(world: &mut World, start: u32, count: u32) -> Vec<EntityId> {
    (start..start + count)
        .map(|i| world.add_entity((A(i), B(i))))
        .collect()
}

/// Returns `ids` with their A and B values, looked up one by one and sorted by id.
fn pair_values(world: &World, ids: &[EntityId]) -> Vec<(EntityId, (u32, u32))> {
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    sorted(
        ids.iter()
            .map(|&id| (id, (a.get(id).unwrap().0, b.get(id).unwrap().0)))
            .collect(),
    )
}

/// Runs `(&a, &b)` in every way and returns its sorted items.
fn pair_items(world: &World) -> Vec<(EntityId, (u32, u32))> {
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    sorted(every_way!((&a, &b).iter(), |(a, b): (&A, &B)| (a.0, b.0)))
}

/// 10 pairs and 2 A-only entities are grouped, then a pair and 2 B-only entities are added.
///
/// The grouped walks are the aligned pairs, then A's and B's unevaluated regions.
/// They cost 32 against 39 for A or B captaining alone.
///
/// Returns the pairs.
fn follow_ups_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    world.add_entity(A(100));
    world.add_entity(A(101));
    world.regroup();
    pairs.extend(add_ab(&mut world, 10, 1));
    world.add_entity(B(100));
    world.add_entity(B(101));

    (world, pairs)
}

/// Asserts that a regrouped pair yields every pair in every way, as a single exact walk.
#[test]
fn aligned() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 6);
    world.add_entity(A(100));
    world.add_entity(B(100));
    world.regroup();

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    // Captaining alone, A or B would count 7 candidates and reject one.
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
    assert_eq!((&a, &b).iter().size_hint(), (6, Some(6)));
}

/// Asserts that mutable outputs of a regrouped pair reach every pair in every way,
/// and that outputs held all at once each write their own entity's components.
#[test]
fn aligned_mut() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 6);
    let lone = world.add_entity(A(100));
    world.regroup();
    let expected = pair_values(&world, &pairs);

    let (mut a, mut b) = world.borrow::<(ViewMut<A>, ViewMut<B>)>().unwrap();
    let items = every_way!((&mut a, &mut b).iter(), |(a, b): (&mut A, &mut B)| (
        a.0, b.0
    ));
    assert_eq!(sorted(items), expected);

    let held: Vec<(&mut A, &mut B)> = (&mut a, &mut b).iter().collect();
    for (a, b) in held {
        a.0 += 1000;
        b.0 += 2000;
    }
    drop((a, b));

    let written: Vec<_> = expected
        .iter()
        .map(|&(id, (a, b))| (id, (a + 1000, b + 2000)))
        .collect();
    assert_eq!(pair_values(&world, &pairs), written);
    assert_eq!(world.get::<&A>(lone).unwrap().0, 100);
}

/// Asserts that an ABC query with an AB group and a larger C yields the pairs owning a C.
#[test]
fn sailor() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 4);
    world.add_component(pairs[1], C(1));
    world.add_component(pairs[3], C(3));
    for i in 0..5 {
        world.add_entity(C(10 + i));
    }
    world.regroup();

    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    let items = every_way!((&a, &b, &c).iter(), |(a, b, c): (&A, &B, &C)| (
        a.0, b.0, c.0
    ));

    assert_eq!(
        sorted(items),
        [(pairs[1], (1, 1, 1)), (pairs[3], (3, 3, 3))]
    );
}

/// Asserts that an ABC query with an AB group and a single C yields that C's pair.
#[test]
fn small_captain() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 4);
    world.add_component(pairs[2], C(2));
    world.regroup();

    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    let items = every_way!((&a, &b, &c).iter(), |(a, b, c): (&A, &B, &C)| (
        a.0, b.0, c.0
    ));

    assert_eq!(items, [(pairs[2], (2, 2, 2))]);
}

/// Returns the sorted items of the ABCD query on `world`.
fn abcd_items(world: &World) -> Vec<(EntityId, u32)> {
    let (a, b, c, d) = world
        .borrow::<(View<A>, View<B>, View<C>, View<D>)>()
        .unwrap();

    sorted(every_way!((&a, &b, &c, &d).iter(), |(a, b, c, d): (
        &A,
        &B,
        &C,
        &D
    )| a.0
        + b.0
        + c.0
        + d.0))
}

/// Adds 4 entities owning `A(i)`, `B(i)`, `C(i)` and `D(i)` and groups AB and CD.
fn abcd_world() -> (World, Vec<(EntityId, u32)>) {
    let mut world = World::builder()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    let all = (0..4)
        .map(|i| (world.add_entity((A(i), B(i), C(i), D(i))), 4 * i))
        .collect();
    world.regroup();
    world.regroup();

    (world, all)
}

/// Asserts that an ABCD query yields the same entities whichever of its AB and CD groups
/// has entities left in its unevaluated regions.
#[test]
fn two_groups() {
    let (mut world, all) = abcd_world();
    add_ab(&mut world, 10, 2);
    assert_eq!(abcd_items(&world), all);

    let (mut world, all) = abcd_world();
    world.add_entity((C(10), D(10)));
    world.add_entity((C(11), D(11)));
    assert_eq!(abcd_items(&world), all);
}

/// Asserts that a group of other storages doesn't change what an AB query yields.
#[test]
fn unrelated_group() {
    let mut world = World::builder().with_group::<(C, D)>().build();
    let pairs = add_ab(&mut world, 0, 4);
    world.add_entity(A(100));
    world.add_entity((C(0), D(0)));
    world.regroup();

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Asserts that an AB query with only an ABC group yields the pairs lacking C,
/// which sit outside the group range.
#[test]
fn projection() {
    let mut world = World::builder().with_group::<(A, B, C)>().build();
    let pairs = add_ab(&mut world, 0, 5);
    for &id in &pairs[..3] {
        world.add_component(id, C(0));
    }
    world.regroup();

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Returns `ids` with their A, B and C values, looked up one by one and sorted by id.
fn triple_values(world: &World, ids: &[EntityId]) -> Vec<(EntityId, (u32, u32, u32))> {
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    sorted(
        ids.iter()
            .map(|&id| {
                let values = (
                    a.get(id).unwrap().0,
                    b.get(id).unwrap().0,
                    c.get(id).unwrap().0,
                );

                (id, values)
            })
            .collect(),
    )
}

/// Runs `(&a, &b, &c)` in every way and returns its sorted items.
fn triple_items(world: &World) -> Vec<(EntityId, (u32, u32, u32))> {
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    sorted(every_way!((&a, &b, &c).iter(), |(a, b, c): (
        &A,
        &B,
        &C
    )| (a.0, b.0, c.0)))
}

/// Entities of `overlap_world`.
struct Overlap {
    world: World,
    /// Entities owning A and B.
    pairs: Vec<EntityId>,
    /// Entities owning A, B and C.
    triples: Vec<EntityId>,
}

/// AB and AC groups holding 8 AB, 8 AC and 4 ABC entities, along with 3 B-only ones.
/// Then two AB entities gain a C and an AC entity gains a B.
///
/// After `regroup`, the 3 entities gaining a component are promoted into ABC, moving the
/// ranges around. Otherwise their new component is left unevaluated.\
/// Entity `i` owns `A(i)`, `B(100 + i)` and `C(200 + i)`, among its components.
fn overlap_world(regroup: bool) -> Overlap {
    let mut world = World::builder()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let ab: Vec<_> = (0..8)
        .map(|i| world.add_entity((A(i), B(100 + i))))
        .collect();
    let ac: Vec<_> = (8..16)
        .map(|i| world.add_entity((A(i), C(200 + i))))
        .collect();
    let abc: Vec<_> = (16..20)
        .map(|i| world.add_entity((A(i), B(100 + i), C(200 + i))))
        .collect();
    for i in 20..23 {
        world.add_entity(B(100 + i));
    }
    world.regroup();

    world.add_component(ab[0], C(200));
    world.add_component(ab[1], C(201));
    world.add_component(ac[0], B(108));
    if regroup {
        world.regroup();
    }

    let pairs = [&ab[..], &ac[..1], &abc[..]].concat();
    let triples = [&ab[..2], &ac[..1], &abc[..]].concat();

    Overlap {
        world,
        pairs,
        triples,
    }
}

/// Asserts that with AB and AC groups, pair and triple queries yield every match in every way,
/// whether the promoted entities were regrouped yet or not.
#[test]
fn overlapping() {
    for regroup in [false, true] {
        let overlap = overlap_world(regroup);

        assert_eq!(
            pair_items(&overlap.world),
            pair_values(&overlap.world, &overlap.pairs),
            "regroup: {regroup}"
        );
        assert_eq!(
            triple_items(&overlap.world),
            triple_values(&overlap.world, &overlap.triples),
            "regroup: {regroup}"
        );
    }
}

/// Asserts that after regroup, an AB query walks AB and the ABC union: 13 candidates
/// in several segments, where B captaining alone would count 16.
#[test]
fn overlapping_hint() {
    let overlap = overlap_world(true);
    let (a, b) = overlap.world.borrow::<(View<A>, View<B>)>().unwrap();

    assert_eq!((&a, &b).iter().size_hint(), (0, Some(13)));
}

/// Asserts that after regroup, an ABC query walks the ABC union alone: 7 candidates,
/// where C captaining alone would count 14.
#[test]
fn union_hint() {
    let overlap = overlap_world(true);
    let (a, b, c) = overlap
        .world
        .borrow::<(View<A>, View<B>, View<C>)>()
        .unwrap();

    assert_eq!((&a, &b, &c).iter().size_hint().1, Some(7));
}

/// Asserts that mutable outputs over AB and ABC, held all at once, each write their own
/// entity's components exactly once.
#[test]
fn overlapping_held_mut() {
    let overlap = overlap_world(true);
    let expected = pair_values(&overlap.world, &overlap.pairs);

    let (mut a, mut b) = overlap.world.borrow::<(ViewMut<A>, ViewMut<B>)>().unwrap();
    let held: Vec<(&mut A, &mut B)> = (&mut a, &mut b).iter().collect();
    assert_eq!(held.len(), expected.len());
    for (a, b) in held {
        a.0 += 1000;
        b.0 += 2000;
    }
    drop((a, b));

    let written: Vec<_> = expected
        .iter()
        .map(|&(id, (a, b))| (id, (a + 1000, b + 2000)))
        .collect();
    assert_eq!(pair_values(&overlap.world, &overlap.pairs), written);
}

/// Asserts that mutable outputs over AB, the ABC union and late unevaluated pairs reach every
/// pair in every way, `next` prefixes followed by bulk consumption included.
#[test]
fn overlapping_mut_every_way() {
    let mut overlap = overlap_world(true);
    let mut pairs = overlap.pairs.clone();
    pairs.extend(add_ab(&mut overlap.world, 500, 2));
    let expected = pair_values(&overlap.world, &pairs);

    let (mut a, mut b) = overlap.world.borrow::<(ViewMut<A>, ViewMut<B>)>().unwrap();
    let items = every_way!((&mut a, &mut b).iter(), |(a, b): (&mut A, &mut B)| (
        a.0, b.0
    ));

    assert_eq!(sorted(items), expected);
}

/// Asserts that stopping inside a ring or an unevaluated walk leaves the mutable
/// iterator resumable, with each pair updated exactly once across both traversals.
#[test]
fn overlapping_resume() {
    let mut overlap = overlap_world(true);
    overlap.pairs.extend(add_ab(&mut overlap.world, 500, 2));
    let expected = pair_values(&overlap.world, &overlap.pairs);
    let stops = [overlap.pairs[3], overlap.pairs[10], overlap.pairs[13]];

    for stop in stops {
        let (mut a, mut b) = overlap.world.borrow::<(ViewMut<A>, ViewMut<B>)>().unwrap();
        let mut iter = (&mut a, &mut b).iter().with_id();
        assert_eq!(iter.size_hint(), (0, Some(17)));
        let mut seen = Vec::new();
        let result = iter.try_for_each(|(id, (a, b))| {
            seen.push(id);
            a.0 += 1;
            b.0 += 1;
            if id == stop { Err(()) } else { Ok(()) }
        });
        assert_eq!(result, Err(()));
        iter.for_each(|(id, (a, b))| {
            seen.push(id);
            a.0 += 1;
            b.0 += 1;
        });
        assert_eq!(sorted(seen), sorted(overlap.pairs.clone()));
    }

    let written: Vec<_> = expected
        .into_iter()
        .map(|(id, (a, b))| (id, (a + 3, b + 3)))
        .collect();
    assert_eq!(pair_values(&overlap.world, &overlap.pairs), written);
}

/// Asserts that a bulk callback panic in AB, ABC or an unevaluated walk leaves the
/// owned iterator reusable from its first segment and releases borrows when dropped.
#[cfg(feature = "std")]
#[test]
fn overlapping_unwind() {
    let mut overlap = overlap_world(true);
    overlap.pairs.extend(add_ab(&mut overlap.world, 500, 2));
    let expected = pair_values(&overlap.world, &overlap.pairs);
    let mut owned = overlap.world.iter::<(&mut A, &mut B)>();

    for stop in [overlap.pairs[3], overlap.pairs[10], overlap.pairs[13]] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let iter = owned.iter();
            assert_eq!(iter.size_hint(), (0, Some(17)));
            iter.with_id().for_each(|(id, _)| {
                if id == stop {
                    panic!("callback panic at {stop:?}");
                }
            });
        }));
        assert!(result.is_err());

        let items = owned
            .iter()
            .with_id()
            .map(|(id, (a, b))| (id, (a.0, b.0)))
            .collect();
        assert_eq!(sorted(items), expected);
    }
    drop(owned);
    assert!(overlap.world.borrow::<(ViewMut<A>, ViewMut<B>)>().is_ok());
}

/// Asserts that with AB and ACD groups, an ABC query yields the entities of their ABCD union
/// in every way. It walks AB and ABCD: 11 candidates, where B or C alone would count 13.
#[test]
fn extra_captain() {
    let mut world = World::builder()
        .with_group::<(A, B)>()
        .with_group::<(A, C, D)>()
        .build();
    add_ab(&mut world, 0, 6);
    for i in 0..6 {
        world.add_entity((A(i), C(i), D(i)));
    }
    let triples: Vec<_> = (0..5)
        .map(|i| world.add_entity((A(i), B(i), C(i), D(i))))
        .collect();
    for i in 0..2 {
        world.add_entity(B(i));
        world.add_entity(C(i));
    }
    world.regroup();

    assert_eq!(triple_items(&world), triple_values(&world, &triples));

    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    assert_eq!((&a, &b, &c).iter().size_hint(), (0, Some(11)));
}

/// Asserts that walks over AB and ABC yield the same items in parallel, each entity once.
/// They span several segments, so the parallel iterator reports no exact length.
#[cfg(feature = "parallel")]
#[test]
fn overlapping_parallel() {
    use rayon::prelude::*;

    let overlap = overlap_world(true);
    let (a, b) = overlap.world.borrow::<(View<A>, View<B>)>().unwrap();
    assert_eq!((&a, &b).par_iter().opt_len(), None);

    let parallel: Vec<_> = (&a, &b)
        .par_iter()
        .with_id()
        .map(|(id, (a, b))| (id, (a.0, b.0)))
        .collect();
    assert_eq!(
        sorted(parallel),
        pair_values(&overlap.world, &overlap.pairs)
    );
}

/// Asserts that an empty grouped pair yields nothing.
#[test]
fn empty() {
    let world = World::builder().with_group::<(A, B)>().build();

    assert_eq!(pair_items(&world), []);
}

/// Asserts that a group of zero sized components yields every pair in every way.
#[test]
fn zero_sized() {
    let mut world = World::builder().with_group::<(Zst0, Zst1)>().build();
    let pairs: Vec<EntityId> = (0..4).map(|_| world.add_entity((Zst0, Zst1))).collect();
    world.add_entity(Zst0);
    world.add_entity(Zst1);
    world.regroup();

    let (z0, z1) = world.borrow::<(View<Zst0>, View<Zst1>)>().unwrap();
    let items = every_way!((&z0, &z1).iter(), |_: (&Zst0, &Zst1)| ());

    assert_eq!(
        sorted(items),
        sorted(pairs.into_iter().map(|id| (id, ())).collect())
    );
    assert_eq!((&z0, &z1).iter().size_hint(), (4, Some(4)));
}

/// Mutable ZST outputs can stay alive across an aligned prefix and late coverage.
#[test]
fn zero_sized_held_mut() {
    let mut world = World::builder().with_group::<(Zst0, Zst1)>().build();
    let mut ids: Vec<_> = (0..10).map(|_| world.add_entity((Zst0, Zst1))).collect();
    world.regroup();
    ids.push(world.add_entity((Zst0, Zst1)));
    let (mut z0, mut z1) = world.borrow::<(ViewMut<Zst0>, ViewMut<Zst1>)>().unwrap();
    let iter = (&mut z0, &mut z1).iter();
    // 10 aligned candidates and the late pair walked by each member.
    assert_eq!(iter.size_hint(), (0, Some(12)));
    let held = iter.with_id().fold(Vec::new(), |mut acc, item| {
        acc.push(item);
        acc
    });
    let mut found = Vec::new();
    for (id, (z0, z1)) in held {
        *z0 = Zst0;
        *z1 = Zst1;
        found.push(id);
    }
    assert_eq!(sorted(found), sorted(ids));
}

/// Asserts that pairs added after a regroup are yielded once each, along with the regrouped ones.
#[test]
fn new_entities() {
    let (world, pairs) = follow_ups_world();

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    // 10 aligned candidates, then one in A's unevaluated region and 3 in B's.
    // Captaining alone, A or B would count 13 candidates.
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
    assert_eq!((&a, &b).iter().size_hint(), (0, Some(14)));
}

/// Asserts that an entity whose A was evaluated as ungrouped before it gained B is yielded.
#[test]
fn a_ungrouped_then_b() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    let late = world.add_entity(A(10));
    world.regroup();
    world.add_component(late, B(10));
    pairs.push(late);

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Asserts that an entity whose B was evaluated as ungrouped before it gained A is yielded.
#[test]
fn b_ungrouped_then_a() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    let late = world.add_entity(B(10));
    world.regroup();
    world.add_component(late, A(10));
    pairs.push(late);

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Asserts that after removals damage both members of AB, the remaining pairs are yielded,
/// and still are after a new pair and a full regroup.
#[test]
fn removal_then_regroup() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    world.regroup();
    world.remove::<A>(pairs.remove(0));
    world.remove::<B>(pairs.remove(4));

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    pairs.extend(add_ab(&mut world, 10, 1));
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    world.regroup();
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
    assert_eq!((&a, &b).iter().size_hint(), (9, Some(9)));
}

/// Asserts that deleting grouped components, one after the other without regroup,
/// leaves the remaining pairs yielded, before and after a regroup.
#[test]
fn deletion() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    world.regroup();
    world.delete_component::<B>(pairs.remove(7));
    world.delete_component::<A>(pairs.remove(2));
    world.delete_component::<(A, B)>(pairs.remove(0));

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    world.regroup();
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Asserts that deleting grouped entities, without regroup, leaves the remaining pairs yielded,
/// before and after a regroup.
#[test]
fn entity_deletion() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    let lone = world.add_entity(A(100));
    world.regroup();
    world.delete_entity(pairs.remove(9));
    world.delete_entity(pairs.remove(3));
    world.delete_entity(lone);

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    world.regroup();
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Asserts that `retain` and `retain_mut` over a grouped storage leave the kept pairs yielded,
/// with the values `retain_mut` wrote, before and after a regroup.
#[test]
fn retain() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 12);
    world.regroup();

    world.retain::<A>(|_, a| a.0 % 3 != 0);
    world.retain_mut::<B>(|_, mut b| {
        b.0 += 100;
        b.0 % 2 == 0
    });

    let kept: Vec<EntityId> = (0..12)
        .filter(|i| i % 3 != 0 && i % 2 == 0)
        .map(|i| pairs[i])
        .collect();
    let expected: Vec<_> = kept
        .iter()
        .map(|&id| {
            let i = pairs.iter().position(|&pair| pair == id).unwrap() as u32;
            (id, (i, i + 100))
        })
        .collect();
    assert_eq!(pair_items(&world), expected);

    world.regroup();
    assert_eq!(pair_items(&world), expected);
}

/// Asserts that a newer generation replacing a grouped pair's components one at a time is only
/// yielded once it owns both, the orphaned components never are, before and after a regroup.
#[test]
fn replacement() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    world.regroup();
    let orphan = pairs.remove(4);
    world
        .borrow::<EntitiesViewMut>()
        .unwrap()
        .delete_unchecked(orphan);

    // The new entity's A replaces the orphan's, the orphan keeps its B.
    let reused = world.add_entity(A(99));
    assert_eq!(reused.index(), orphan.index());
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
    assert!(world.get::<&B>(orphan).is_ok());

    world.add_component(reused, B(99));
    pairs.push(reused);
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    world.regroup();
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Asserts that mutable outputs of a pair damaged by removals, held all at once, each write
/// their own entity's components once.
#[test]
fn removal_mut() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    world.regroup();
    world.remove::<A>(pairs.remove(8));
    world.remove::<B>(pairs.remove(1));
    let expected = pair_values(&world, &pairs);

    let (mut a, mut b) = world.borrow::<(ViewMut<A>, ViewMut<B>)>().unwrap();
    let held: Vec<(&mut A, &mut B)> = (&mut a, &mut b).iter().collect();
    for (a, b) in held {
        a.0 += 1000;
        b.0 += 2000;
    }
    drop((a, b));

    let written: Vec<_> = expected
        .iter()
        .map(|&(id, (a, b))| (id, (a + 1000, b + 2000)))
        .collect();
    assert_eq!(pair_values(&world, &pairs), written);
}

/// Asserts that removals, deletions and a retain over a thread-local storage in a group leave
/// the remaining pairs yielded, before and after a regroup.
#[cfg(feature = "thread_local")]
#[test]
fn thread_local_removal() {
    use shipyard::borrow::NonSend;

    let mut world = World::builder().with_group::<(NonSend<A>, B)>().build();
    let pairs: Vec<EntityId> = world.run(
        |mut entities: EntitiesViewMut, mut a: NonSend<ViewMut<A>>, mut b: ViewMut<B>| {
            (0..10)
                .map(|i| entities.add_entity((&mut *a, &mut b), (A(i), B(i))))
                .collect()
        },
    );
    world.regroup();

    world.run(|mut a: NonSend<ViewMut<A>>| {
        (&mut *a).remove(pairs[2]);
        a.retain(|_, a| a.0 != 6);
    });
    world.delete_entity(pairs[8]);

    let items = |world: &World| {
        world.run(|a: NonSend<View<A>>, b: View<B>| {
            sorted(
                (&*a, &b)
                    .iter()
                    .with_id()
                    .map(|(id, (a, b))| (id, (a.0, b.0)))
                    .collect::<Vec<_>>(),
            )
        })
    };
    let expected: Vec<_> = [0, 1, 3, 4, 5, 7, 9]
        .into_iter()
        .map(|i| (pairs[i], (i as u32, i as u32)))
        .collect();
    assert_eq!(items(&world), expected);

    world.regroup();
    assert_eq!(items(&world), expected);
}

/// Asserts that an entity reusing a deleted pair's index is yielded with its new generation,
/// before and after a regroup, and the deleted one never is.
#[test]
fn generation_reuse() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    world.regroup();
    world.delete_entity(pairs[0]);
    let reused = world.add_entity((A(99), B(99)));
    assert_eq!(reused.index(), pairs[0].index());
    pairs[0] = reused;

    assert_eq!(pair_items(&world), pair_values(&world, &pairs));

    world.regroup();
    assert_eq!(pair_items(&world), pair_values(&world, &pairs));
}

/// Asserts that writing through `Mut` in every segment of the grouped walks flags exactly
/// the written components, reading flags nothing.
#[test]
fn tracked_mutation() {
    let mut world = World::builder().with_group::<(A, Tracked)>().build();
    let mut pairs: Vec<EntityId> = (0..10)
        .map(|i| world.add_entity((A(i), Tracked(i))))
        .collect();
    world.add_entity(A(100));
    world.regroup();
    pairs.push(world.add_entity((A(10), Tracked(10))));
    world.add_entity(Tracked(100));
    world
        .borrow::<ViewMut<Tracked>>()
        .unwrap()
        .clear_all_modified();

    let (a, mut tracked) = world.borrow::<(View<A>, ViewMut<Tracked>)>().unwrap();
    let items = every_way!((&a, &mut tracked).iter(), |(a, t): (&A, Mut<Tracked>)| (
        a.0, t.0
    ));
    assert_eq!(items.len(), 11);
    assert!(pairs.iter().all(|&id| !tracked.is_modified(id)));

    // `pairs[3]` is in the aligned segment. `pairs[10]` is in `Tracked`'s unevaluated region,
    // walked before A's since it's longer, so `Tracked` drives when it's written.
    (&a, &mut tracked).iter().for_each(|(a, mut t)| {
        if a.0 == 3 || a.0 == 10 {
            t.0 += 100;
        }
    });

    for (i, &id) in (0..).zip(&pairs) {
        let written = i == 3 || i == 10;
        assert_eq!(tracked.is_modified(id), written, "{i}");
        assert_eq!(
            (&tracked).get(id).unwrap().0,
            if written { 100 + i } else { i }
        );
    }
}

/// Asserts that filters and `Or` in a query over a grouped pair keep every match.
#[test]
fn filters() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 4);
    world.add_entity(A(100));
    world.add_component(pairs[0], C(0));
    world.add_entity(D(0));
    world.regroup();

    let (a, b, c, d) = world
        .borrow::<(View<A>, View<B>, View<C>, View<D>)>()
        .unwrap();

    let not = every_way!((&a, &b, !&c).iter(), |(a, _, ()): (&A, &B, ())| a.0);
    assert_eq!(sorted(not), [(pairs[1], 1), (pairs[2], 2), (pairs[3], 3)]);

    let optional = every_way!((&a, &b, c.as_optional()).iter(), |(a, _, c): (
        &A,
        &B,
        Option<&C>
    )| (a.0, c.is_some()));
    assert_eq!(optional.len(), 4);

    let or = every_way!((&a, &b, &c | &d).iter(), |(a, _, cd): (
        &A,
        &B,
        OneOfTwo<&C, &D>
    )| (
        a.0,
        matches!(cd, OneOfTwo::One(_))
    ));
    assert_eq!(or, [(pairs[0], (0, true))]);
}

/// Asserts that a grouped pair nested in a query yields its matches.
#[test]
fn nested() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 4);
    world.add_entity(A(100));
    for &id in &pairs[1..] {
        world.add_component(id, C(0));
    }
    world.add_entity(C(100));
    world.regroup();

    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    let items = every_way!(((&a, &b), &c).iter(), |((a, b), _): ((&A, &B), &C)| (
        a.0, b.0
    ));

    assert_eq!(sorted(items), pair_values(&world, &pairs[1..]));
}

/// Asserts that `World::iter` and `AllStorages::iter` yield the grouped walks every time.
#[test]
fn owned() {
    let (world, pairs) = follow_ups_world();
    let expected = pair_values(&world, &pairs);

    let mut iter = world.iter::<(&A, &B)>();
    for _ in 0..2 {
        let items = every_way!(iter.iter(), |(a, b): (&A, &B)| (a.0, b.0));
        assert_eq!(sorted(items), expected);
    }
    drop(iter);

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let mut iter = all_storages.iter::<(&A, &mut B)>();
    for _ in 0..2 {
        let items = every_way!(iter.iter(), |(a, b): (&A, &mut B)| (a.0, b.0));
        assert_eq!(sorted(items), expected);
    }
}

/// 500 pairs, 50 A-only and 50 B-only entities are grouped, then 100 more pairs are added.
///
/// The grouped walks cost 1600 against 1950 for A captaining alone.
#[cfg(feature = "parallel")]
fn parallel_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 500);
    for i in 0..50 {
        world.add_entity(A(1000 + i));
        world.add_entity(B(1000 + i));
    }
    world.regroup();
    pairs.extend(add_ab(&mut world, 500, 100));

    (world, pairs)
}

/// Asserts that the grouped walks yield the same ids and values in parallel and
/// sequentially, each entity once.
#[cfg(feature = "parallel")]
#[test]
fn parallel() {
    use rayon::prelude::*;

    let (world, pairs) = parallel_world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
    assert_eq!((&a, &b).iter().size_hint(), (0, Some(700)));

    let parallel: Vec<_> = (&a, &b)
        .par_iter()
        .with_id()
        .map(|(id, (a, b))| (id, (a.0, b.0)))
        .collect();
    assert_eq!(sorted(parallel), pair_values(&world, &pairs));

    let parallel: Vec<_> = (&a, &b).par_iter().map(|(a, b)| (a.0, b.0)).collect();
    let sequential: Vec<_> = (&a, &b).iter().map(|(a, b)| (a.0, b.0)).collect();
    assert_eq!(sorted(parallel), sorted(sequential));
}

/// Asserts that parallel mutable outputs of the grouped walks write every pair once.
#[cfg(feature = "parallel")]
#[test]
fn parallel_mut() {
    use rayon::prelude::*;

    let (world, pairs) = parallel_world();
    let expected: Vec<_> = pair_values(&world, &pairs)
        .into_iter()
        .map(|(id, (a, b))| (id, (a + b, b)))
        .collect();

    world.run(|(mut a, b): (ViewMut<A>, View<B>)| {
        (&mut a, &b).par_iter().for_each(|(a, b)| a.0 += b.0);
    });

    assert_eq!(pair_values(&world, &pairs), expected);
}

#[cfg(feature = "proc")]
mod derived {
    use super::sorted;
    use crate::iteration::ways::every_way;
    use shipyard::*;

    #[derive(Component, Debug, PartialEq)]
    struct Pos(u32);

    #[derive(Component, Debug, PartialEq)]
    #[track(Modification)]
    struct Vel(u32);

    #[derive(Component, Debug, PartialEq)]
    struct Tag(u32);

    /// `_tag` is skipped, entities without a `Tag` are still yielded.
    #[derive(Borrow, IntoIter)]
    #[shipyard(item_name = "Motion")]
    struct MotionView<'v> {
        #[shipyard(item_field_name = "position")]
        v_pos: View<'v, Pos>,
        vm_vel: ViewMut<'v, Vel>,
        #[shipyard(item_field_skip)]
        _tag: View<'v, Tag>,
    }

    #[derive(Borrow, IntoIter)]
    struct MotionTuple<'v>(View<'v, Pos>, View<'v, Vel>);

    /// Every entity owning a `Pos` and a `Vel`, by segment of the grouped walks.
    struct Matches {
        /// `Pos(i)` and `Vel(10 + i)`, in the group range.
        aligned: Vec<EntityId>,
        /// `Pos(100)`, evaluated as ungrouped before gaining `Vel(110)`.
        /// Found by `Vel`'s unevaluated walk, which looks `Pos` up.
        vel_walk: EntityId,
        /// `Vel(111)`, evaluated as ungrouped before gaining `Pos(101)`.
        /// Found by `Pos`'s unevaluated walk, which looks the tracked `Vel` up.
        pos_walk: EntityId,
    }

    impl Matches {
        fn all(&self) -> Vec<EntityId> {
            let mut all = self.aligned.clone();
            all.push(self.vel_walk);
            all.push(self.pos_walk);
            all
        }
    }

    /// 10 entities own `Pos` and `Vel` and get grouped, along with a `Pos`-only and
    /// a `Vel`-only entity which later gain the missing component. Two more `Pos`-only and
    /// `Vel`-only entities never match. Every entity owns a `Tag`.
    ///
    /// The unevaluated walks start past the group range and the ungrouped components.\
    /// The grouped walks cost 26 against 39 for `Pos` captaining alone.
    fn world() -> (World, Matches) {
        let mut world = World::builder().with_group::<(Pos, Vel)>().build();

        let aligned = (0..10)
            .map(|i| world.add_entity((Pos(i), Vel(10 + i), Tag(i))))
            .collect();
        let vel_walk = world.add_entity((Pos(100), Tag(0)));
        let pos_walk = world.add_entity((Vel(111), Tag(0)));
        world.add_entity((Pos(200), Tag(0)));
        world.add_entity((Vel(201), Tag(0)));
        world.regroup();
        world.add_component(vel_walk, Vel(110));
        world.add_component(pos_walk, Pos(101));
        world.borrow::<ViewMut<Vel>>().unwrap().clear_all_modified();

        let matches = Matches {
            aligned,
            vel_walk,
            pos_walk,
        };

        (world, matches)
    }

    /// Returns `ids` with their `Pos` and `Vel` values, looked up one by one and sorted by id.
    fn values(world: &World, ids: &[EntityId]) -> Vec<(EntityId, (u32, u32))> {
        let (pos, vel) = world.borrow::<(View<Pos>, View<Vel>)>().unwrap();

        sorted(
            ids.iter()
                .map(|&id| (id, (pos.get(id).unwrap().0, vel.get(id).unwrap().0)))
                .collect(),
        )
    }

    /// Asserts that a named derived view over the grouped walks yields every match in every
    /// way, and that reading its `Mut`s flags nothing.
    #[test]
    fn named_reads() {
        let (world, matches) = world();
        let expected = values(&world, &matches.all());

        let mut motion = world.borrow::<MotionView>().unwrap();
        // 10 aligned candidates and one in each unevaluated region.
        // Captaining alone, `Pos` would count 13 candidates.
        assert_eq!((&mut motion).iter().size_hint(), (0, Some(12)));

        let items = every_way!((&mut motion).iter(), |motion: Motion| (
            motion.position.0,
            motion.vel.0
        ));

        assert_eq!(sorted(items), expected);
        for id in matches.all() {
            assert!(!motion.vm_vel.is_modified(id));
        }
    }

    /// Ways of consuming a derived iterator.
    #[derive(Clone, Copy, Debug)]
    enum Way {
        Next,
        Fold,
        ForEach,
        IdNext,
        IdFold,
        IdForEach,
    }

    const WAYS: [Way; 6] = [
        Way::Next,
        Way::Fold,
        Way::ForEach,
        Way::IdNext,
        Way::IdFold,
        Way::IdForEach,
    ];

    /// Consumes `motion`'s iterator with `next` for `prefix` items, then in `way`,
    /// adding 1000 to the `Vel` of the item whose `Pos` is `target`.
    fn write(motion: &mut MotionView<'_>, way: Way, prefix: usize, target: u32) {
        let write = |mut motion: Motion| {
            if motion.position.0 == target {
                motion.vel.0 += 1000;
            }
        };

        match way {
            Way::Next | Way::Fold | Way::ForEach => {
                let mut iter = (&mut *motion).iter();
                (&mut iter).take(prefix).for_each(&write);

                match way {
                    Way::Next => {
                        for motion in iter {
                            write(motion);
                        }
                    }
                    Way::Fold => iter.fold((), |(), motion| write(motion)),
                    _ => iter.for_each(&write),
                }
            }
            Way::IdNext | Way::IdFold | Way::IdForEach => {
                let mut iter = (&mut *motion).iter().with_id();
                for (_, motion) in (&mut iter).take(prefix) {
                    write(motion);
                }

                match way {
                    Way::IdNext => {
                        for (_, motion) in iter {
                            write(motion);
                        }
                    }
                    Way::IdFold => iter.fold((), |(), (_, motion)| write(motion)),
                    _ => iter.for_each(|(_, motion)| write(motion)),
                }
            }
        }
    }

    /// Asserts that writing through one item's `Mut`, in every way and after every interesting
    /// `next` prefix, flags only that entity's `Vel`.
    ///
    /// The targets cover both members driving the aligned segment, the untracked `Pos`
    /// driving with `Vel` looked up, and the tracked `Vel` driving with `Pos` looked up.
    /// The last two walks start past index 0, a write at a segment local index would land
    /// on another entity.
    #[test]
    fn named_writes() {
        let (_, matches) = world();
        let targets = [
            (matches.aligned[4], 4),
            (matches.pos_walk, 101),
            (matches.vel_walk, 100),
        ];

        for way in WAYS {
            for prefix in [0, 1, 5, 10, 11, 12, 13] {
                for (target, target_pos) in targets {
                    let (world, matches) = world();
                    let mut expected = values(&world, &matches.all());
                    for (id, (_, vel)) in &mut expected {
                        if *id == target {
                            *vel += 1000;
                        }
                    }

                    let mut motion = world.borrow::<MotionView>().unwrap();
                    write(&mut motion, way, prefix, target_pos);

                    let context = format!("{way:?} after {prefix}, writing {target_pos}");
                    for id in matches.all() {
                        assert_eq!(motion.vm_vel.is_modified(id), id == target, "{context}");
                    }
                    drop(motion);
                    assert_eq!(values(&world, &matches.all()), expected, "{context}");
                }
            }
        }
    }

    /// Asserts that a tuple derived view over the grouped walks yields every match in every way.
    #[test]
    fn tuple() {
        let (world, matches) = world();
        let expected = values(&world, &matches.all());

        let motion = world.borrow::<MotionTuple>().unwrap();
        let items = every_way!((&motion).iter(), |(pos, vel): (&Pos, &Vel)| (pos.0, vel.0));

        assert_eq!(sorted(items), expected);
    }
}
