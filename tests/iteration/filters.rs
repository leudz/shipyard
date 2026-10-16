//! Tracking filters, `Not`, `Optional` and `Or` consumed in every way.

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
struct Small(u32);
impl Component for Small {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::InsertionAndModification;
}

/// `e0` to `e3` own `Tracked(i)` and `A(i)`, six more entities only own `A`.
///
/// `e0` and `e1` were inserted before the last clear of `Tracked`, `e2` and `e3` after it.
/// Only `e1` was modified since, its `Tracked` is now 11.\
/// `e1` and `e2` own `Small(i)`, `e2` owns `B(12)`.
///
/// Tracking filters cost twice their length: 8 is less than `A`'s 10 so they drive
/// queries with `A`, while `Small`'s 2 drives queries with them.
fn world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let mut e: Vec<EntityId> = (0..2)
        .map(|i| world.add_entity((Tracked(i), A(i))))
        .collect();
    world
        .borrow::<ViewMut<Tracked>>()
        .unwrap()
        .clear_all_inserted();
    e.extend((2..4).map(|i| world.add_entity((Tracked(i), A(i)))));
    for i in 0..6 {
        world.add_entity(A(100 + i));
    }
    world.add_component(e[1], Small(1));
    world.add_component(e[2], (Small(2), B(12)));

    (&mut world.borrow::<ViewMut<Tracked>>().unwrap())
        .get(e[1])
        .unwrap()
        .0 += 10;

    (world, e)
}

fn pair((tracked, a): (&Tracked, &A)) -> (u32, u32) {
    (tracked.0, a.0)
}

fn small_pair((small, tracked): (&Small, &Tracked)) -> (u32, u32) {
    (small.0, tracked.0)
}

/// Asserts that `Inserted` drives a query with a larger storage and only yields
/// the components inserted since the last clear.
#[test]
fn inserted_captain() {
    let (world, e) = world();
    let (tracked, a) = world.borrow::<(View<Tracked>, View<A>)>().unwrap();

    assert_eq!((tracked.inserted(), &a).iter().size_hint(), (0, Some(4)));
    let items = every_way!((tracked.inserted(), &a).iter(), pair);

    assert_eq!(items, [(e[2], (2, 2)), (e[3], (3, 3))]);
}

/// Asserts that `Modified` drives a query with a larger storage and only yields `e1`.
#[test]
fn modified_captain() {
    let (world, e) = world();
    let (tracked, a) = world.borrow::<(View<Tracked>, View<A>)>().unwrap();

    assert_eq!((tracked.modified(), &a).iter().size_hint(), (0, Some(4)));
    let items = every_way!((tracked.modified(), &a).iter(), pair);

    assert_eq!(items, [(e[1], (11, 1))]);
}

/// Asserts that `InsertedOrModified` drives a query with a larger storage and yields
/// every component but `e0`'s.
#[test]
fn inserted_or_modified_captain() {
    let (world, e) = world();
    let (tracked, a) = world.borrow::<(View<Tracked>, View<A>)>().unwrap();

    let items = every_way!((tracked.inserted_or_modified(), &a).iter(), pair);

    assert_eq!(items, [(e[1], (11, 1)), (e[2], (2, 2)), (e[3], (3, 3))]);
}

/// Asserts that `Inserted` looked up by a smaller storage only accepts `e2`.
#[test]
fn inserted_sailor() {
    let (world, e) = world();
    let (small, tracked) = world.borrow::<(View<Small>, View<Tracked>)>().unwrap();

    assert_eq!(
        (&small, tracked.inserted()).iter().size_hint(),
        (0, Some(2))
    );
    let items = every_way!((&small, tracked.inserted()).iter(), small_pair);

    assert_eq!(items, [(e[2], (2, 2))]);
}

/// Asserts that `Modified` looked up by a smaller storage only accepts `e1`.
#[test]
fn modified_sailor() {
    let (world, e) = world();
    let (small, tracked) = world.borrow::<(View<Small>, View<Tracked>)>().unwrap();

    let items = every_way!((&small, tracked.modified()).iter(), small_pair);

    assert_eq!(items, [(e[1], (1, 11))]);
}

/// Asserts that `InsertedOrModified` looked up by a smaller storage accepts `e1` and `e2`.
#[test]
fn inserted_or_modified_sailor() {
    let (world, e) = world();
    let (small, tracked) = world.borrow::<(View<Small>, View<Tracked>)>().unwrap();

    let items = every_way!((&small, tracked.inserted_or_modified()).iter(), small_pair);

    assert_eq!(items, [(e[1], (1, 11)), (e[2], (2, 2))]);
}

/// Asserts that `Not<Inserted>` drives a query with a larger storage and yields
/// the components inserted before the last clear.
#[test]
fn not_inserted_captain() {
    let (world, e) = world();
    let (tracked, a) = world.borrow::<(View<Tracked>, View<A>)>().unwrap();

    let items = every_way!((!tracked.inserted(), &a).iter(), pair);

    assert_eq!(items, [(e[0], (0, 0)), (e[1], (11, 1))]);
}

/// Asserts that `Not<Modified>` drives a query with a larger storage and yields
/// every component but the modified `e1`'s.
#[test]
fn not_modified_captain() {
    let (world, e) = world();
    let (tracked, a) = world.borrow::<(View<Tracked>, View<A>)>().unwrap();

    let items = every_way!((!tracked.modified(), &a).iter(), pair);

    assert_eq!(items, [(e[0], (0, 0)), (e[2], (2, 2)), (e[3], (3, 3))]);
}

/// Asserts that `Not<InsertedOrModified>` drives a query with a larger storage
/// and only yields `e0`.
#[test]
fn not_inserted_or_modified_captain() {
    let (world, e) = world();
    let (tracked, a) = world.borrow::<(View<Tracked>, View<A>)>().unwrap();

    let items = every_way!((!tracked.inserted_or_modified(), &a).iter(), pair);

    assert_eq!(items, [(e[0], (0, 0))]);
}

/// Asserts that `Not<Inserted>` looked up by a smaller storage only accepts `e1`.
#[test]
fn not_inserted_sailor() {
    let (world, e) = world();
    let (small, tracked) = world.borrow::<(View<Small>, View<Tracked>)>().unwrap();

    let items = every_way!((&small, !tracked.inserted()).iter(), small_pair);

    assert_eq!(items, [(e[1], (1, 11))]);
}

/// Asserts that `Not<Modified>` looked up by a smaller storage only accepts `e2`.
#[test]
fn not_modified_sailor() {
    let (world, e) = world();
    let (small, tracked) = world.borrow::<(View<Small>, View<Tracked>)>().unwrap();

    let items = every_way!((&small, !tracked.modified()).iter(), small_pair);

    assert_eq!(items, [(e[2], (2, 2))]);
}

/// Asserts that a plain `Not` looked up by `A` rejects the entities owning the component:
/// `e1` and `e2` own a `Small`.
#[test]
fn not_view() {
    let (world, e) = world();
    let (tracked, small) = world.borrow::<(View<Tracked>, View<Small>)>().unwrap();

    let items = every_way!((&tracked, !&small).iter(), |(tracked, ()): (
        &Tracked,
        ()
    )| tracked.0);

    assert_eq!(items, [(e[0], 0), (e[3], 3)]);
}

/// Asserts that `Optional` yields the present `B` of `e2` and `None` for the other
/// entities of `Small`, without dropping them.
#[test]
fn optional_present_absent() {
    let (world, e) = world();
    let (small, b) = world.borrow::<(View<Small>, View<B>)>().unwrap();

    let items = every_way!((&small, b.as_optional()).iter(), |(small, b): (
        &Small,
        Option<&B>
    )| (
        small.0,
        b.map(|b| b.0)
    ));

    assert_eq!(items, [(e[1], (1, None)), (e[2], (2, Some(12)))]);
}

/// Asserts that a mutable `Optional` can be written through when present
/// and yields `None` when absent, in bulk.
#[test]
fn optional_mut() {
    let (world, e) = world();
    let (small, mut b) = world.borrow::<(View<Small>, ViewMut<B>)>().unwrap();

    let absent = (&small, b.as_optional())
        .iter()
        .fold(0, |absent, (_, b)| match b {
            Some(b) => {
                b.0 += 100;
                absent
            }
            None => absent + 1,
        });

    assert_eq!(absent, 1);
    assert_eq!((&b).get(e[2]).unwrap(), &B(112));
}

/// Asserts that an `Or` whose first branch filters yields the entities the filter rejects
/// from the second branch, and the ones it accepts only once, in every way.
#[test]
fn or_filtered_precedence() {
    let (world, e) = world();
    let (tracked, small) = world.borrow::<(View<Tracked>, View<Small>)>().unwrap();

    let items = every_way!((tracked.inserted() | &small).iter(), |item: OneOfTwo<
        &Tracked,
        &Small,
    >| match item {
        OneOfTwo::One(tracked) => tracked.0,
        OneOfTwo::Two(small) => 100 + small.0,
    });

    // `e2` is inserted, `Small` doesn't yield it a second time.
    assert_eq!(items, [(e[2], 2), (e[3], 3), (e[1], 101)]);
}

/// Asserts that an `Or` looked up by `Small` matches either branch, without depending
/// on which branch drove the last segment: `e2` is inserted, `e1` only has an `A`.
#[test]
fn or_sailor() {
    let (world, e) = world();
    let (small, tracked, a) = world
        .borrow::<(View<Small>, View<Tracked>, View<A>)>()
        .unwrap();

    let items = every_way!(
        (&small, tracked.inserted() | &a).iter(),
        |(small, item): (&Small, OneOfTwo<&Tracked, &A>)| match item {
            OneOfTwo::One(tracked) => (small.0, tracked.0),
            OneOfTwo::Two(a) => (small.0, 100 + a.0),
        },
    );

    assert_eq!(items, [(e[1], (1, 101)), (e[2], (2, 2))]);
}

/// Asserts that a tracking filter nested in a tuple keeps rejecting from inside it,
/// whichever storage drives the outer query.
#[test]
fn nested_filter() {
    let (world, e) = world();
    let (small, tracked, a) = world
        .borrow::<(View<Small>, View<Tracked>, View<A>)>()
        .unwrap();

    let items = every_way!(((tracked.inserted(), &a), &small).iter(), |(
        (tracked, a),
        small,
    ): (
        (&Tracked, &A),
        &Small
    )| (
        tracked.0, a.0, small.0
    ));

    assert_eq!(items, [(e[2], (2, 2, 2))]);
}

/// Asserts that mutable outputs nested in tuples are yielded like shared ones,
/// prefixes included.
#[test]
fn nested_mut() {
    let (world, e) = world();
    let (small, mut tracked, mut a) = world
        .borrow::<(View<Small>, ViewMut<Tracked>, ViewMut<A>)>()
        .unwrap();

    let items = every_way!(((&mut a, &small), &mut tracked).iter(), |(
        (a, small),
        tracked,
    ): (
        (&mut A, &Small),
        Mut<Tracked>
    )| (
        a.0, small.0, tracked.0
    ));

    assert_eq!(items, [(e[1], (1, 1, 11)), (e[2], (2, 2, 2))]);
}

/// Asserts that only traversals that can't reject report an exact length: a lone window does,
/// a tracking filter or an `Or` only bound their candidates.
#[test]
fn exactness() {
    let (world, _) = world();
    let (tracked, a, small) = world
        .borrow::<(View<Tracked>, View<A>, View<Small>)>()
        .unwrap();

    assert_eq!(a.iter().size_hint(), (10, Some(10)));
    assert_eq!(tracked.inserted().iter().size_hint(), (0, Some(4)));
    assert_eq!((&tracked | &small).iter().size_hint(), (0, Some(6)));
}

/// Asserts that parallel iterators only report a length when the traversal can't reject,
/// with or without ids.
#[cfg(feature = "parallel")]
#[test]
fn parallel_exactness() {
    use rayon::iter::ParallelIterator;

    let (world, _) = world();
    let (tracked, a) = world.borrow::<(View<Tracked>, View<A>)>().unwrap();

    assert_eq!(a.par_iter().opt_len(), Some(10));
    assert_eq!(a.par_iter().with_id().opt_len(), Some(10));
    assert_eq!(tracked.inserted().par_iter().with_id().opt_len(), None);
    assert_eq!(tracked.inserted().par_iter().opt_len(), None);
    assert_eq!((tracked.inserted(), &a).par_iter().opt_len(), None);
}
