//! Queries over grouped storages compared with the same queries without group.
//!
//! In this world the ordinary walks are cheaper, grouped walks are tested in `automatic`.
//!
//! Regrouping reorders components, so results are compared sorted by entity
//! against a world built the same way without the group.

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
    type Tracking = track::InsertionAndModification;
}

#[derive(Debug, PartialEq)]
struct C(u32);
impl Component for C {
    type Tracking = track::Untracked;
}

/// The world declares the `(A, B)` group when `grouped` is `true`.
///
/// `e0` to `e5` own `A(i)`, `e1`, `e2` and `e4` own `B(10 + i)`, `e2` and `e5` own `C(20 + i)`.
/// `B` is cleared of its insertions, then the world is regrouped.
///
/// Afterwards, `e6` owns `A(6)` and `B(16)`, `e7` only `B(17)` and `e8` owns `A(8)`, `B(18)`
/// and `C(28)`. `e1`'s `B` is modified last.
fn world(grouped: bool) -> (World, Vec<EntityId>) {
    let mut world = if grouped {
        World::builder().with_group::<(A, B)>().build()
    } else {
        World::new()
    };

    let mut e: Vec<EntityId> = (0..6).map(|i| world.add_entity(A(i))).collect();
    for i in [1, 2, 4] {
        world.add_component(e[i], B(10 + i as u32));
    }
    for i in [2, 5] {
        world.add_component(e[i], C(20 + i as u32));
    }
    world.borrow::<ViewMut<B>>().unwrap().clear_all_inserted();

    world.regroup();

    e.push(world.add_entity((A(6), B(16))));
    e.push(world.add_entity(B(17)));
    e.push(world.add_entity((A(8), B(18), C(28))));
    (&mut world.borrow::<ViewMut<B>>().unwrap())
        .get(e[1])
        .unwrap()
        .0 += 100;

    (world, e)
}

/// Runs `query` on the grouped and ungrouped worlds and asserts they yield the same items,
/// in any order. The grouped query is consumed in every way.
///
/// Returns the items sorted by entity.
macro_rules! same_as_ungrouped {
    (|$views: ident: $views_ty: ty| $iter: expr, $value: expr $(,)?) => {{
        let (grouped, _) = world(true);
        let (ungrouped, _) = world(false);

        let mut grouped_items = {
            #[allow(unused_mut)]
            let mut $views = grouped.borrow::<$views_ty>().unwrap();
            every_way!($iter, $value)
        };
        let mut ungrouped_items = {
            #[allow(unused_mut)]
            let mut $views = ungrouped.borrow::<$views_ty>().unwrap();
            let value = $value;
            $iter
                .with_id()
                .map(|(id, item)| (id, value(item)))
                .collect::<Vec<_>>()
        };

        grouped_items.sort_by_key(|(id, _)| *id);
        ungrouped_items.sort_by_key(|(id, _)| *id);
        assert_eq!(grouped_items, ungrouped_items);

        grouped_items
    }};
}

/// Asserts that grouping really happened: `A`'s grouped members moved in front of it,
/// so the comparisons below don't compare two identical layouts.
#[test]
fn reordered() {
    let (grouped, e) = world(true);
    let (ungrouped, _) = world(false);

    let ids = |world: &World| {
        world
            .borrow::<View<A>>()
            .unwrap()
            .iter()
            .ids()
            .collect::<Vec<_>>()
    };

    assert_eq!(
        ids(&ungrouped),
        [e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[8]]
    );
    // Regroup doesn't promise an order inside a group, only which entities it holds.
    let mut front = ids(&grouped)[..3].to_vec();
    front.sort_unstable();
    assert_eq!(front, [e[1], e[2], e[4]]);
}

/// Asserts that the grouped storages yield every entity owning both components.
#[test]
fn pair() {
    let (_, e) = world(false);

    let items = same_as_ungrouped!(|v: (View<A>, View<B>)| (&v.0, &v.1).iter(), |(a, b): (
        &A,
        &B
    )| (
        a.0, b.0
    ),);

    assert_eq!(
        items,
        [
            (e[1], (1, 111)),
            (e[2], (2, 12)),
            (e[4], (4, 14)),
            (e[6], (6, 16)),
            (e[8], (8, 18))
        ]
    );
}

/// Asserts that mutable outputs of grouped storages, in any order and with an ungrouped
/// third storage, yield every entity owning all three components.
#[test]
fn mut_triple() {
    let (_, e) = world(false);

    let items = same_as_ungrouped!(
        |v: (View<C>, ViewMut<B>, ViewMut<A>)| (&v.0, &mut v.1, &mut v.2).iter(),
        |(c, b, a): (&C, Mut<B>, &mut A)| (c.0, b.0, a.0),
    );

    assert_eq!(items, [(e[2], (22, 12, 2)), (e[8], (28, 18, 8))]);
}

/// Asserts that tracking filters over a grouped storage keep their timestamps:
/// only the components inserted after the clear and `e1`'s modification are yielded.
#[test]
fn tracking() {
    let (_, e) = world(false);

    let inserted = same_as_ungrouped!(
        |v: (View<B>, View<A>)| (v.0.inserted(), &v.1).iter(),
        |(b, a): (&B, &A)| (b.0, a.0),
    );
    let modified = same_as_ungrouped!(
        |v: (View<B>, View<A>)| (v.0.modified(), &v.1).iter(),
        |(b, a): (&B, &A)| (b.0, a.0),
    );
    let not_inserted_or_modified = same_as_ungrouped!(
        |v: (View<B>, View<A>)| (!v.0.inserted_or_modified(), &v.1).iter(),
        |(b, a): (&B, &A)| (b.0, a.0),
    );

    assert_eq!(inserted, [(e[6], (16, 6)), (e[8], (18, 8))]);
    assert_eq!(modified, [(e[1], (111, 1))]);
    assert_eq!(not_inserted_or_modified, [(e[2], (12, 2)), (e[4], (14, 4))]);
}

/// Asserts that `Not` and `Optional` over a grouped storage keep every entity of `A`.
#[test]
fn not_optional() {
    let (_, e) = world(false);

    let not = same_as_ungrouped!(|v: (View<A>, View<B>)| (&v.0, !&v.1).iter(), |(a, ()): (
        &A,
        ()
    )| a.0,);
    let optional = same_as_ungrouped!(
        |v: (View<A>, View<B>)| (&v.0, v.1.as_optional()).iter(),
        |(a, b): (&A, Option<&B>)| (a.0, b.map(|b| b.0)),
    );

    assert_eq!(not, [(e[0], 0), (e[3], 3), (e[5], 5)]);
    assert_eq!(optional.len(), 8);
    assert_eq!(optional[1], (e[1], (1, Some(111))));
    assert_eq!(optional[3], (e[3], (3, None)));
}

/// Asserts that an `Or` over a grouped storage yields each entity once.
#[test]
fn or() {
    let (_, e) = world(false);

    let items = same_as_ungrouped!(
        |v: (View<B>, View<C>)| (v.0.inserted() | &v.1).iter(),
        |item: OneOfTwo<&B, &C>| match item {
            OneOfTwo::One(b) => b.0,
            OneOfTwo::Two(c) => c.0,
        },
    );

    assert_eq!(
        items,
        [(e[2], 22), (e[5], 25), (e[6], 16), (e[7], 17), (e[8], 18)]
    );
}

/// Asserts that nested tuples over grouped storages yield the same items.
#[test]
fn nested() {
    let (_, e) = world(false);

    let items = same_as_ungrouped!(
        |v: (View<A>, View<B>, View<C>)| (&v.2, (&v.0, &v.1)).iter(),
        |(c, (a, b)): (&C, (&A, &B))| (c.0, a.0, b.0),
    );

    assert_eq!(items, [(e[2], (22, 2, 12)), (e[8], (28, 8, 18))]);
}

/// Asserts that `World::iter` over grouped storages yields the same items, every time.
#[test]
fn owned() {
    let (grouped, e) = world(true);
    let mut iter = grouped.iter::<(&A, &mut B)>();

    let mut items = every_way!(iter.iter(), |(a, b): (&A, Mut<B>)| (a.0, b.0));
    items.sort_by_key(|(id, _)| *id);

    assert_eq!(
        items,
        [
            (e[1], (1, 111)),
            (e[2], (2, 12)),
            (e[4], (4, 14)),
            (e[6], (6, 16)),
            (e[8], (8, 18))
        ]
    );
}

#[cfg(feature = "proc")]
#[derive(Borrow, IntoIter)]
struct PairView<'v> {
    a: View<'v, A>,
    vm_b: ViewMut<'v, B>,
}

/// Asserts that a named derived view over grouped storages yields the same items.
#[cfg(feature = "proc")]
#[test]
fn derived() {
    let (_, e) = world(false);

    let items = same_as_ungrouped!(|v: PairView| (&mut v).iter(), |pair: Pair| (
        pair.a.0, pair.b.0
    ),);

    assert_eq!(items.len(), 5);
    assert_eq!(items[0], (e[1], (1, 111)));
}
