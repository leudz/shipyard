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
struct Ins(u32);
impl Component for Ins {
    type Tracking = track::Insertion;
}

/// Entities `e0` to `e3` own `A(i)`, `e1` and `e3` also own `B(10 + i)`.
/// Then `f0` to `f2` only own `B(20 + i)`.
fn world() -> (World, Vec<EntityId>, Vec<EntityId>) {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(A(i))).collect();
    world.add_component(e[1], B(11));
    world.add_component(e[3], B(13));
    let f: Vec<EntityId> = (0..3).map(|i| world.add_entity(B(20 + i))).collect();

    (world, e, f)
}

fn value(item: OneOfTwo<&A, &B>) -> u32 {
    match item {
        OneOfTwo::One(a) => a.0,
        OneOfTwo::Two(b) => b.0,
    }
}

fn id_value((id, item): (EntityId, OneOfTwo<&A, &B>)) -> (EntityId, u32) {
    (id, value(item))
}

/// Asserts that an `Or` over two empty storages yields nothing, with `next`, `fold` and `WithId`.
#[test]
fn both_empty() {
    let world = World::new();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    assert!((&a | &b).iter().next().is_none());
    assert_eq!((&a | &b).iter().fold(0, |count, _| count + 1), 0);
    assert!((&a | &b).iter().with_id().next().is_none());
}

/// Asserts that an `Or` whose first storage is empty yields the second storage's entities.
#[test]
fn first_empty() {
    let mut world = World::new();
    let f: Vec<EntityId> = (0..2).map(|i| world.add_entity(B(20 + i))).collect();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let items: Vec<_> = (&a | &b).iter().with_id().map(id_value).collect();

    assert_eq!(items, [(f[0], 20), (f[1], 21)]);
}

/// Asserts that an `Or` whose second storage is empty yields the first storage's entities.
#[test]
fn second_empty() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..2).map(|i| world.add_entity(A(i))).collect();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let items: Vec<_> = (&a | &b).iter().with_id().map(id_value).collect();

    assert_eq!(items, [(e[0], 0), (e[1], 1)]);
}

/// Asserts that entities in both storages are yielded once, by the first storage.
#[test]
fn overlap() {
    let (world, e, f) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let items: Vec<_> = (&a | &b).iter().with_id().map(id_value).collect();

    assert_eq!(
        items,
        [
            (e[0], 0),
            (e[1], 1),
            (e[2], 2),
            (e[3], 3),
            (f[0], 20),
            (f[1], 21),
            (f[2], 22)
        ]
    );
}

/// Asserts that an entity filtered out of the first storage is yielded by the second one,
/// while an entity the first storage yields isn't yielded again.
#[test]
fn filtered_precedence() {
    let mut world = World::new();
    let old = world.add_entity((Ins(0), B(10)));
    world.borrow::<ViewMut<Ins>>().unwrap().clear_all_inserted();
    let new = world.add_entity((Ins(1), B(11)));
    let (ins, b) = world
        .borrow::<(View<Ins, track::Insertion>, View<B>)>()
        .unwrap();

    let items: Vec<_> = (ins.inserted() | &b)
        .iter()
        .with_id()
        .map(|(id, item)| match item {
            OneOfTwo::One(ins) => (id, ins.0),
            OneOfTwo::Two(b) => (id, b.0),
        })
        .collect();

    assert_eq!(items, [(new, 1), (old, 10)]);
}

/// Asserts that an exhausted `Or` keeps returning `None`.
#[test]
fn exhausted() {
    let (world, _, _) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut iter = (&a | &b).iter();
    assert_eq!(iter.by_ref().count(), 7);

    for _ in 0..3 {
        assert!(iter.next().is_none());
    }
}

/// Asserts that folding after a `next` prefix ending exactly on the first storage's last entity
/// yields the second storage's entities.
#[test]
fn prefix_at_boundary() {
    let (world, _, _) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut iter = (&a | &b).iter();
    let prefix: Vec<_> = iter.by_ref().take(4).map(value).collect();
    let rest = iter.fold(Vec::new(), |mut rest, item| {
        rest.push(value(item));
        rest
    });

    assert_eq!(prefix, [0, 1, 2, 3]);
    assert_eq!(rest, [20, 21, 22]);
}

/// Asserts that `WithId` returns the right ids right before and right after the transition,
/// then when folding the rest.
#[test]
fn with_id_boundary() {
    let (world, e, f) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut iter = (&a | &b).iter().with_id();
    let prefix: Vec<_> = iter.by_ref().take(4).map(id_value).collect();
    let after = iter.next().map(id_value);
    let rest = iter.fold(Vec::new(), |mut rest, item| {
        rest.push(id_value(item));
        rest
    });

    assert_eq!(prefix.last(), Some(&(e[3], 3)));
    assert_eq!(after, Some((f[0], 20)));
    assert_eq!(rest, [(f[1], 21), (f[2], 22)]);
}

/// Asserts that an `Or` captaining a tuple yields both storages' entities found in the other view.
#[test]
fn mixed_captain() {
    let mut world = World::new();
    let e0 = world.add_entity((A(0), C(0)));
    let e1 = world.add_entity((B(1), C(1)));
    world.add_entity(A(2));
    for i in 3..10 {
        world.add_entity(C(i));
    }
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    let items: Vec<_> = (&a | &b, &c)
        .iter()
        .with_id()
        .map(|(id, (a_or_b, c))| (id, value(a_or_b), c.0))
        .collect();

    assert_eq!(items, [(e0, 0, 0), (e1, 1, 1)]);
}

/// Asserts that an `Or` acting as sailor in a tuple matches either storage.
#[test]
fn mixed_sailor() {
    let (mut world, e, f) = world();
    let c0 = world.add_entity(C(0));
    world.add_component(e[2], C(2));
    world.add_component(f[1], C(21));
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    let items: Vec<_> = (&c, &a | &b)
        .iter()
        .with_id()
        .map(|(id, (c, a_or_b))| (id, c.0, value(a_or_b)))
        .collect();

    assert_eq!(items, [(e[2], 2, 2), (f[1], 21, 21)]);
    assert!(!items.iter().any(|&(id, _, _)| id == c0));
}

/// Asserts that an `Or` nested in another `Or` keeps its own transition:
/// each entity is yielded once, by the first storage that has it.
#[test]
fn nested() {
    let mut world = World::new();
    let e0 = world.add_entity((A(0), B(0)));
    let e1 = world.add_entity((B(1), C(1)));
    let e2 = world.add_entity(C(2));
    let (a, b, c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();

    let items: Vec<_> = (&a | (&b | &c))
        .iter()
        .with_id()
        .map(|(id, item)| match item {
            OneOfTwo::One(a) => (id, a.0),
            OneOfTwo::Two(OneOfTwo::One(b)) => (id, 10 + b.0),
            OneOfTwo::Two(OneOfTwo::Two(c)) => (id, 20 + c.0),
        })
        .collect();

    assert_eq!(items, [(e0, 0), (e1, 11), (e2, 22)]);
}

/// Asserts that an `Or` captaining a tuple nested in another `Or` keeps its own transition.
#[test]
fn nested_in_mixed() {
    let mut world = World::new();
    let e0 = world.add_entity(A(0));
    let e1 = world.add_entity((B(1), D(1)));
    let e2 = world.add_entity((C(2), D(2)));
    world.add_entity(B(3));
    for i in 4..10 {
        world.add_entity(D(i));
    }
    let (a, b, c, d) = world
        .borrow::<(View<A>, View<B>, View<C>, View<D>)>()
        .unwrap();

    let items: Vec<_> = (&a | (&b | &c, &d))
        .iter()
        .with_id()
        .map(|(id, item)| match item {
            OneOfTwo::One(a) => (id, a.0),
            OneOfTwo::Two((OneOfTwo::One(b), d)) => (id, 10 * d.0 + b.0),
            OneOfTwo::Two((OneOfTwo::Two(c), d)) => (id, 20 * d.0 + c.0),
        })
        .collect();

    assert_eq!(items, [(e0, 0), (e1, 11), (e2, 42)]);
}

/// Asserts that each iterator obtained from the same `IntoIterRef` yields every item.
#[test]
fn replay() {
    let (world, e, _) = world();
    let mut iter = world.iter::<(&A, &B)>();

    let first: Vec<_> = iter
        .iter()
        .with_id()
        .map(|(id, (a, _))| (id, a.0))
        .collect();
    let second: Vec<_> = iter
        .iter()
        .with_id()
        .map(|(id, (a, _))| (id, a.0))
        .collect();

    assert_eq!(first, [(e[1], 1), (e[3], 3)]);
    assert_eq!(second, first);
}

/// Asserts that an `Or` over two mutable views only borrows them for the iteration: writes
/// through either branch land on the right component and both views are usable afterwards.
#[test]
fn mutable() {
    let (world, e, f) = world();
    let (mut a, mut b) = world.borrow::<(ViewMut<A>, ViewMut<B>)>().unwrap();

    for item in (&mut a | &mut b).iter() {
        match item {
            OneOfTwo::One(a) => a.0 += 100,
            OneOfTwo::Two(b) => b.0 += 100,
        }
    }

    assert_eq!(a[e[1]], A(101));
    assert_eq!(b[e[1]], B(11));
    assert_eq!(b[f[0]], B(120));
}
