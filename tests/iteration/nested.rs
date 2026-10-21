//! Grouped walks driven through nested queries, through public construction only.
//!
//! Worlds are grouped with `World::group` and `World::regroup`, then queried as usual.
//! Expected items are looked up entity by entity.\
//! Which path won is checked by the library's `grouping_nested` tests. Here, `size_hint`
//! tells it apart: a nested grouped walk counts the pair's candidates, an ordinary walk
//! counts its captain's.

use super::ways::every_way;
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

/// Adds `C(100 + i)` to the entity of each `(i, id)`.
fn add_c(world: &mut World, ids: &[EntityId]) {
    for (i, &id) in (0..).zip(ids) {
        world.add_component(id, C(100 + i));
    }
}

/// Returns the entities of `ids` owning A, B and C with their values, looked up one by one
/// and sorted by id.
fn abc_values(world: &World, ids: &[EntityId]) -> Vec<(EntityId, (u32, u32, u32))> {
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    sorted(
        ids.iter()
            .filter(|&&id| a.contains(id) && b.contains(id) && c.contains(id))
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

/// 10 grouped pairs owning C, with 5 entities only owning A, 5 only owning B
/// and 4 only owning C.
///
/// C would captain the ordinary walk over its 14 components. Driving through the aligned
/// pair costs 30 against 56.\
/// Returns the pairs.
fn grouped_pair_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 10);
    for i in 0..5 {
        world.add_entity(A(50 + i));
        world.add_entity(B(50 + i));
    }
    add_c(&mut world, &pairs);
    for i in 0..4 {
        world.add_entity(C(200 + i));
    }
    world.regroup();

    (world, pairs)
}

/// Asserts that a grouped pair drives `((A, B), C)` over C's ordinary walk and yields every
/// match in every way.
#[test]
fn pair_drives() {
    let (world, pairs) = grouped_pair_world();
    let expected = abc_values(&world, &pairs);
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    assert_eq!(((&a, &b), &c).iter().size_hint(), (0, Some(10)));

    let items = every_way!(((&a, &b), &c).iter(), |((a, b), c): ((&A, &B), &C)| (
        a.0, b.0, c.0
    ));
    assert_eq!(sorted(items), expected);
}

/// Asserts that a pair driving `(C, (A, B))` from the second position keeps the query's
/// structure, each value belonging to the yielded entity.
#[test]
fn second_position() {
    let (world, pairs) = grouped_pair_world();
    let expected = abc_values(&world, &pairs);
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    assert_eq!((&c, (&a, &b)).iter().size_hint(), (0, Some(10)));

    let items = every_way!((&c, (&a, &b)).iter(), |(c, (a, b)): (&C, (&A, &B))| (
        a.0, b.0, c.0
    ));
    assert_eq!(sorted(items), expected);
}

/// Asserts that a tiny C keeps the ordinary walk of `((A, B), C)`.
#[test]
fn small_captain() {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let pairs = add_ab(&mut world, 0, 10);
    add_c(&mut world, &pairs[4..6]);
    world.regroup();
    let expected = abc_values(&world, &pairs);

    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    assert_eq!(((&a, &b), &c).iter().size_hint(), (0, Some(2)));

    let items = every_way!(((&a, &b), &c).iter(), |((a, b), c): ((&A, &B), &C)| (
        a.0, b.0, c.0
    ));
    assert_eq!(sorted(items), expected);
}

/// Asserts that a pair two levels down drives `(((A, B), C), D)` and yields every match.
#[test]
fn deeper() {
    let (mut world, pairs) = grouped_pair_world();
    for (i, &id) in (0..).zip(&pairs) {
        world.add_component(id, D(i));
    }
    for i in 0..10 {
        world.add_entity(D(100 + i));
    }

    let (a, b, c, d) = world
        .borrow::<(View<A>, View<B>, View<C>, View<D>)>()
        .unwrap();
    assert_eq!((((&a, &b), &c), &d).iter().size_hint(), (0, Some(10)));

    let items = every_way!((((&a, &b), &c), &d).iter(), |(((a, b), c), d): (
        ((&A, &B), &C),
        &D
    )| (a.0, b.0, c.0, d.0));
    let expected: Vec<_> = pairs
        .iter()
        .map(|&id| {
            let values = (
                a.get(id).unwrap().0,
                b.get(id).unwrap().0,
                c.get(id).unwrap().0,
                d.get(id).unwrap().0,
            );

            (id, values)
        })
        .collect();
    assert_eq!(sorted(items), sorted(expected));
}

/// Asserts that with disjoint AB and CD groups, `((A, B), (C, D))` drives through one pair
/// and still finds the entity whose other pair was added after the regroup.
#[test]
fn disjoint_groups() {
    let mut world = World::builder()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    let mut all: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C(i), D(i))))
        .collect();
    let late = world.add_entity((A(4), B(4)));
    world.regroup();
    world.regroup();
    world.add_component(late, (C(4), D(4)));
    all.push(late);

    let (a, b, c, d) = world
        .borrow::<(View<A>, View<B>, View<C>, View<D>)>()
        .unwrap();

    let items = every_way!(((&a, &b), (&c, &d)).iter(), |((a, b), (c, d)): (
        (&A, &B),
        (&C, &D)
    )| (a.0, b.0, c.0, d.0));
    let expected: Vec<_> = (0..).zip(&all).map(|(i, &id)| (id, (i, i, i, i))).collect();
    assert_eq!(sorted(items), sorted(expected));
}

/// Asserts that an AC group doesn't change `((A, B), C)`'s matches, it can't drive the query.
#[test]
fn cross_boundary() {
    let mut world = World::builder().with_group::<(A, C)>().build();
    let all: Vec<EntityId> = (0..10)
        .map(|i| world.add_entity((A(i), B(i), C(i))))
        .collect();
    world.add_entity((A(50), C(50)));
    world.regroup();
    let expected = abc_values(&world, &all);

    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    let items = every_way!(((&a, &b), &c).iter(), |((a, b), c): ((&A, &B), &C)| (
        a.0, b.0, c.0
    ));
    assert_eq!(sorted(items), expected);
}

/// 10 grouped pairs, then 2 more pairs, all owning C.
///
/// The nested pair walks its aligned prefix then A's and B's unevaluated regions,
/// B's rejecting the late pairs A's already yielded. It costs 46 against 48 for C.\
/// Returns the pairs.
fn late_pairs_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 10);
    world.regroup();
    pairs.extend(add_ab(&mut world, 10, 2));
    add_c(&mut world, &pairs);

    (world, pairs)
}

/// Asserts that a nested pair with entities added after the regroup yields each match once
/// in every way.
#[test]
fn late_pairs() {
    let (world, pairs) = late_pairs_world();
    let expected = abc_values(&world, &pairs);
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    assert_eq!(((&a, &b), &c).iter().size_hint(), (0, Some(14)));

    let items = every_way!(((&a, &b), &c).iter(), |((a, b), c): ((&A, &B), &C)| (
        a.0, b.0, c.0
    ));
    assert_eq!(sorted(items), expected);
}

/// Asserts that mutable outputs of a nested pair write each match once.
#[test]
fn nested_mut() {
    let (world, pairs) = late_pairs_world();
    let expected: Vec<_> = abc_values(&world, &pairs)
        .into_iter()
        .map(|(id, (a, b, c))| (id, (a + c, b + c, c)))
        .collect();

    world.run(|(mut a, mut b, c): (ViewMut<A>, ViewMut<B>, View<C>)| {
        ((&mut a, &mut b), &c).iter().for_each(|((a, b), c)| {
            a.0 += c.0;
            b.0 += c.0;
        });
    });

    assert_eq!(abc_values(&world, &pairs), expected);
}

/// Asserts that `World::iter` and `AllStorages::iter` replay the nested walks every time.
#[test]
fn owned() {
    let (world, pairs) = late_pairs_world();
    let expected = abc_values(&world, &pairs);

    let mut iter = world.iter::<((&A, &B), &C)>();
    for _ in 0..2 {
        let items = every_way!(iter.iter(), |((a, b), c): ((&A, &B), &C)| (a.0, b.0, c.0));
        assert_eq!(sorted(items), expected);
    }
    drop(iter);

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let mut iter = all_storages.iter::<((&A, &mut B), &C)>();
    for _ in 0..2 {
        let items = every_way!(iter.iter(), |((a, b), c): ((&A, &mut B), &C)| (
            a.0, b.0, c.0
        ));
        assert_eq!(sorted(items), expected);
    }
}

/// Asserts that the nested walks yield the same ids and values in parallel and sequentially,
/// and that parallel writes land once on each match.
#[cfg(feature = "parallel")]
#[test]
fn parallel() {
    use rayon::prelude::*;

    let mut world = World::builder().with_group::<(A, B)>().build();
    let mut pairs = add_ab(&mut world, 0, 500);
    world.regroup();
    pairs.extend(add_ab(&mut world, 500, 50));
    add_c(&mut world, &pairs);
    let expected = abc_values(&world, &pairs);

    {
        let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
        assert_eq!(((&a, &b), &c).iter().size_hint(), (0, Some(600)));

        let parallel: Vec<_> = ((&a, &b), &c)
            .par_iter()
            .with_id()
            .map(|(id, ((a, b), c))| (id, (a.0, b.0, c.0)))
            .collect();
        assert_eq!(sorted(parallel), expected);

        let parallel: Vec<_> = ((&a, &b), &c).par_iter().map(|((a, _), _)| a.0).collect();
        let sequential: Vec<_> = ((&a, &b), &c).iter().map(|((a, _), _)| a.0).collect();
        assert_eq!(sorted(parallel), sorted(sequential));
    }

    world.run(|(mut a, b, c): (ViewMut<A>, View<B>, View<C>)| {
        ((&mut a, &b), &c)
            .par_iter()
            .for_each(|((a, _), c)| a.0 += c.0);
    });

    let written: Vec<_> = expected
        .into_iter()
        .map(|(id, (a, b, c))| (id, (a + c, b, c)))
        .collect();
    assert_eq!(abc_values(&world, &pairs), written);
}

#[cfg(feature = "proc")]
mod derived {
    use super::{A, B, C, abc_values, late_pairs_world, sorted};
    use crate::iteration::ways::every_way;
    use shipyard::*;

    #[derive(Borrow, IntoIter)]
    #[shipyard(item_name = "Pair")]
    struct PairView<'v> {
        a: View<'v, A>,
        vm_b: ViewMut<'v, B>,
    }

    /// Asserts that a named derived view nested in a tuple forwards its grouped pair:
    /// it drives the query and yields every match in every way.
    #[test]
    fn named_nested() {
        let (world, pairs) = late_pairs_world();
        let expected = abc_values(&world, &pairs);

        let (mut pair, c) = world.borrow::<(PairView, View<C>)>().unwrap();
        assert_eq!((&mut pair, &c).iter().size_hint(), (0, Some(14)));

        let items = every_way!((&mut pair, &c).iter(), |(pair, c): (Pair, &C)| (
            pair.a.0, pair.b.0, c.0
        ));
        assert_eq!(sorted(items), expected);
    }
}
