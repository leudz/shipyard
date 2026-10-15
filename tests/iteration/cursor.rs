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
struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::Modification;
}

#[derive(Debug, PartialEq)]
struct Ins(u32);
impl Component for Ins {
    type Tracking = track::Insertion;
}

struct Zst;
impl Component for Zst {
    type Tracking = track::Untracked;
}

/// Entities `e0` to `e3` own `A(i)`, `e1` and `e3` also own `B(10 + i)`.
/// Then `f0` to `f2` only own `B(20 + i)`.
///
/// `A` is the smaller storage so it captains `(&A, &B)`,
/// rejecting `e0` and `e2` and accepting the last candidate `e3`.
fn world() -> (World, Vec<EntityId>, Vec<EntityId>) {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(A(i))).collect();
    world.add_component(e[1], B(11));
    world.add_component(e[3], B(13));
    let f: Vec<EntityId> = (0..3).map(|i| world.add_entity(B(20 + i))).collect();

    (world, e, f)
}

/// Flattens `A | B` items to `(id, value)`.
fn or_item(item: (EntityId, OneOfTwo<&A, &B>)) -> (EntityId, u32) {
    match item {
        (id, OneOfTwo::One(a)) => (id, a.0),
        (id, OneOfTwo::Two(b)) => (id, b.0),
    }
}

/// Asserts that `WithId` returns the ids of the accepted entities, not of the rejected
/// candidates before them, including when the last candidate is accepted.
#[test]
fn with_id_rejected() {
    let (world, e, _) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut iter = (&a, &b).iter().with_id();

    assert_eq!(
        iter.next().map(|(id, (a, b))| (id, a.0, b.0)),
        Some((e[1], 1, 11))
    );
    assert_eq!(
        iter.next().map(|(id, (a, b))| (id, a.0, b.0)),
        Some((e[3], 3, 13))
    );
    assert!(iter.next().is_none());
}

/// Asserts that `WithId` keeps returning `None` once exhausted.
#[test]
fn with_id_exhausted() {
    let (world, _, _) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut iter = (&a, &b).iter().with_id();
    assert_eq!(iter.by_ref().count(), 2);

    for _ in 0..3 {
        assert!(iter.next().is_none());
    }
}

/// Asserts that `WithId` returns the right ids on both sides of an `Or` transition,
/// skipping the second storage's entities already yielded by the first.
#[test]
fn with_id_or() {
    let (world, e, f) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let items: Vec<_> = (&a | &b).iter().with_id().map(or_item).collect();

    let expected = vec![
        (e[0], 0),
        (e[1], 1),
        (e[2], 2),
        (e[3], 3),
        (f[0], 20),
        (f[1], 21),
        (f[2], 22),
    ];
    assert_eq!(items, expected);
}

/// Asserts that `WithId` over an `Or` keeps returning `None` once exhausted.
#[test]
fn with_id_or_exhausted() {
    let (world, _, _) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut iter = (&a | &b).iter().with_id();
    assert_eq!(iter.by_ref().count(), 7);

    for _ in 0..3 {
        assert!(iter.next().is_none());
    }
}

/// Consumes `prefix` items with `next` then checks `fold` and `WithId::fold`
/// return exactly the items left.
fn prefix_then_fold(prefix: usize) {
    let (world, e, _) = world();
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
    let expected = [(e[1], 1, 11), (e[3], 3, 13)];
    let skipped = prefix.min(expected.len());

    let mut iter = (&a, &b).iter();
    for _ in 0..prefix {
        iter.next();
    }
    let folded = iter.fold(Vec::new(), |mut acc, (a, b)| {
        acc.push((a.0, b.0));
        acc
    });
    let expected_values: Vec<_> = expected[skipped..]
        .iter()
        .map(|&(_, a, b)| (a, b))
        .collect();
    assert_eq!(folded, expected_values);

    let mut iter = (&a, &b).iter().with_id();
    for _ in 0..prefix {
        iter.next();
    }
    let folded = iter.fold(Vec::new(), |mut acc, (id, (a, b))| {
        acc.push((id, a.0, b.0));
        acc
    });
    assert_eq!(folded, &expected[skipped..]);
}

/// Asserts that folding without a prior `next` returns every match.
#[test]
fn prefix_none() {
    prefix_then_fold(0);
}

/// Asserts that folding after one `next` starts at the following match.
#[test]
fn prefix_one() {
    prefix_then_fold(1);
}

/// Asserts that folding after all matches were consumed by `next` returns nothing.
#[test]
fn prefix_all() {
    prefix_then_fold(2);
}

/// Asserts that folding after calling `next` past exhaustion returns nothing.
#[test]
fn prefix_past_end() {
    prefix_then_fold(5);
}

/// Asserts that an `Or` whose first range has no match moves on to the second range,
/// both with `next` and with `fold` after a `next` prefix.
#[test]
fn or_empty_first_range() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..3).map(|i| world.add_entity(Ins(i))).collect();
    world.add_component(e[1], B(11));
    let f = world.add_entity(B(20));
    world.run(|ins: ViewMut<Ins>| ins.clear_all_inserted());

    let (ins, b) = world
        .borrow::<(View<Ins, track::Insertion>, View<B>)>()
        .unwrap();

    let mut iter = (ins.inserted() | &b).iter().with_id();
    let first = iter.next().map(|(id, item)| match item {
        OneOfTwo::One(ins) => (id, ins.0),
        OneOfTwo::Two(b) => (id, b.0),
    });
    assert_eq!(first, Some((e[1], 11)));

    let rest = iter.fold(Vec::new(), |mut acc, (id, item)| {
        match item {
            OneOfTwo::One(ins) => acc.push((id, ins.0)),
            OneOfTwo::Two(b) => acc.push((id, b.0)),
        }
        acc
    });
    assert_eq!(rest, [(f, 20)]);
}

/// Asserts that a segment whose candidates are all rejected yields nothing,
/// both with `next` and with `fold`.
#[test]
fn no_match() {
    let mut world = World::new();
    world.add_entity(A(0));
    world.add_entity(A(1));
    world.add_entity((B(0),));
    world.add_entity((B(1),));
    world.add_entity((B(2),));

    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    assert!((&a, &b).iter().next().is_none());
    assert_eq!((&a, &b).iter().with_id().fold(0, |count, _| count + 1), 0);
}

/// Asserts that mutable outputs can be held simultaneously and that rejected
/// candidates don't produce any output.
#[test]
fn mutable_outputs() {
    let (world, e, _) = world();
    let (mut a, b) = world.borrow::<(ViewMut<A>, View<B>)>().unwrap();

    let outputs: Vec<_> = (&mut a, &b).iter().collect();
    assert_eq!(outputs.len(), 2);
    for (a, _) in outputs {
        a.0 += 100;
    }

    let values: Vec<_> = e.iter().map(|&id| (&a).get(id).unwrap().0).collect();
    assert_eq!(values, [0, 101, 2, 103]);
}

/// Asserts that only the tracked components written through held outputs are flagged as
/// modified, and that rejected candidates aren't flagged.
#[test]
fn tracked_mutation() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(Tracked(i))).collect();
    world.add_component(e[1], B(11));
    world.add_component(e[3], B(13));
    world.add_entity(B(20));
    world.add_entity(B(21));
    world.run(|tracked: ViewMut<Tracked>| tracked.clear_all_modified());

    let (mut tracked, b) = world.borrow::<(ViewMut<Tracked>, View<B>)>().unwrap();

    let outputs: Vec<_> = (&mut tracked, &b).iter().collect();
    for (mut tracked, b) in outputs {
        tracked.0 = b.0;
    }

    let modified: Vec<_> = e.iter().map(|&id| tracked.is_modified(id)).collect();
    assert_eq!(modified, [false, true, false, true]);
    assert_eq!((&tracked).get(e[3]).unwrap().0, 13);
}

/// Asserts that zero-sized components yield the ids of the accepted entities,
/// with `next` and with `fold` after a `next` prefix.
#[test]
fn zero_sized() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..4).map(|_| world.add_entity(Zst)).collect();
    world.add_component(e[0], B(10));
    world.add_component(e[2], B(12));
    world.add_entity(B(20));
    world.add_entity(B(21));

    let (zst, b) = world.borrow::<(View<Zst>, View<B>)>().unwrap();

    let mut iter = (&zst, &b).iter().with_id();
    assert_eq!(iter.next().map(|(id, (_, b))| (id, b.0)), Some((e[0], 10)));
    let rest: Vec<_> = iter.map(|(id, (_, b))| (id, b.0)).collect();
    assert_eq!(rest, [(e[2], 12)]);

    let ids: Vec<_> = zst.iter().ids().collect();
    assert_eq!(ids, e);
}

/// Asserts that parallel iteration with rejected candidates yields every match once,
/// with ids matching the components.
#[cfg(feature = "parallel")]
#[test]
fn parallel_mixed() {
    use rayon::prelude::*;

    let mut world = World::new();
    let ids: Vec<EntityId> = (0..500).map(|i| world.add_entity(A(i))).collect();
    for (i, &id) in ids.iter().enumerate().filter(|(i, _)| i % 3 == 0) {
        world.add_component(id, B(i as u32));
    }
    for i in 0..400 {
        world.add_entity(B(1000 + i));
    }

    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut items: Vec<_> = (&a, &b)
        .par_iter()
        .with_id()
        .map(|(id, (a, b))| (id, a.0, b.0))
        .collect();
    items.sort_unstable_by_key(|&(_, a, _)| a);

    let expected: Vec<_> = (0..500u32)
        .filter(|i| i % 3 == 0)
        .map(|i| (ids[i as usize], i, i))
        .collect();
    assert_eq!(items, expected);
}

/// Asserts that parallel `Or` iteration yields every entity once across the transition.
#[cfg(feature = "parallel")]
#[test]
fn parallel_or() {
    use rayon::prelude::*;

    let mut world = World::new();
    let e: Vec<EntityId> = (0..300).map(|i| world.add_entity(A(i))).collect();
    for &id in e.iter().step_by(2) {
        world.add_component(id, B(0));
    }
    let f: Vec<EntityId> = (0..300).map(|i| world.add_entity(B(1000 + i))).collect();

    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let mut items: Vec<_> = (&a | &b).par_iter().with_id().map(or_item).collect();
    items.sort_unstable_by_key(|&(_, value)| value);

    let expected: Vec<_> = (0..300u32)
        .map(|i| (e[i as usize], i))
        .chain((0..300u32).map(|i| (f[i as usize], 1000 + i)))
        .collect();
    assert_eq!(items, expected);
}

/// Asserts that an `Or` whose current range holds a single candidate doesn't split,
/// even with a long follow-up, and still yields every entity.
#[cfg(feature = "parallel")]
#[test]
fn parallel_single_candidate() {
    use rayon::iter::plumbing::UnindexedProducer;

    let mut world = World::new();
    world.add_entity(A(0));
    for i in 0..100 {
        world.add_entity(B(i));
    }

    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();

    let (left, right) = (&a | &b).iter().split();

    assert!(right.is_none());
    assert_eq!(left.count(), 101);
}
