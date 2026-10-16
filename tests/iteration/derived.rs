use shipyard::iter::OneOfTwo;
use shipyard::*;

#[derive(Component, Debug, PartialEq)]
struct Pos(u32);

#[derive(Component, Debug, PartialEq)]
struct Vel(u32);

#[derive(Component, Debug, PartialEq)]
struct Tag(u32);

/// Entities `e0` to `e3` own `Pos(i)`, `e1` and `e3` also own `Vel(10 + i)` and `Tag(20 + i)`.
/// Then two entities only own `Vel` and `Tag`, and five more only own `Tag`.
///
/// `Pos` captains the iterations, rejecting `e0` and `e2`.
/// `Tag` is large enough for derived views to captain when nested with it.
fn derive_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(Pos(i))).collect();
    world.add_component(e[1], (Vel(11), Tag(21)));
    world.add_component(e[3], (Vel(13), Tag(23)));
    world.add_entity((Vel(0), Tag(0)));
    world.add_entity((Vel(0), Tag(0)));
    for _ in 0..5 {
        world.add_entity(Tag(0));
    }

    (world, e)
}

#[derive(Borrow, IntoIter)]
struct MotionView<'v> {
    pos: View<'v, Pos>,
    vel: ViewMut<'v, Vel>,
}

#[derive(Borrow, IntoIter)]
struct MotionTuple<'v>(View<'v, Pos>, View<'v, Vel>);

/// Asserts that a named derived iterator skips rejected candidates and yields
/// the ids of the accepted entities, with `next` then `fold`.
#[test]
fn named_with_id() {
    let (world, e) = derive_world();
    let mut motion = world.borrow::<MotionView>().unwrap();

    let mut iter = (&mut motion).iter().with_id();
    let (id, first) = iter.next().unwrap();
    assert_eq!((id, first.pos.0, first.vel.0), (e[1], 1, 11));

    let rest: Vec<_> = iter
        .map(|(id, motion)| (id, motion.pos.0, motion.vel.0))
        .collect();
    assert_eq!(rest, [(e[3], 3, 13)]);
}

/// Asserts that a tuple derived iterator skips rejected candidates and yields
/// the ids of the accepted entities.
#[test]
fn tuple_with_id() {
    let (world, e) = derive_world();
    let motion = world.borrow::<MotionTuple>().unwrap();

    let items: Vec<_> = (&motion)
        .iter()
        .with_id()
        .map(|(id, (pos, vel))| (id, pos.0, vel.0))
        .collect();
    assert_eq!(items, [(e[1], 1, 11), (e[3], 3, 13)]);
}

/// Asserts that a derived iterator nested in a tuple forwards its captain resolution
/// and returns the ids of the accepted entities.
#[test]
fn nested_with_id() {
    let (world, e) = derive_world();
    let (mut motion, tags) = world.borrow::<(MotionView, View<Tag>)>().unwrap();

    let items: Vec<_> = (&mut motion, &tags)
        .iter()
        .with_id()
        .map(|(id, (motion, tag))| (id, motion.pos.0, motion.vel.0, tag.0))
        .collect();
    assert_eq!(items, [(e[1], 1, 11, 21), (e[3], 3, 13, 23)]);
}

/// Asserts that a named derived iterator as the second storage of an `Or` takes over once
/// the first storage is exhausted, yielding only the entities the first one doesn't have.
#[test]
fn named_or() {
    let (mut world, e) = derive_world();
    let alone = world.add_entity((Pos(5), Vel(15)));
    let (mut motion, tags) = world.borrow::<(MotionView, View<Tag>)>().unwrap();

    let items: Vec<_> = (&tags | &mut motion)
        .iter()
        .with_id()
        .filter_map(|(id, item)| match item {
            OneOfTwo::One(tag) if tag.0 != 0 => Some((id, tag.0)),
            OneOfTwo::One(_) => None,
            OneOfTwo::Two(motion) => Some((id, motion.pos.0 + motion.vel.0)),
        })
        .collect();

    assert_eq!(items, [(e[1], 21), (e[3], 23), (alone, 20)]);
}

/// Asserts that a tuple derived iterator as the second storage of an `Or` takes over once
/// the first storage is exhausted, yielding only the entities the first one doesn't have.
#[test]
fn tuple_or() {
    let (mut world, e) = derive_world();
    let alone = world.add_entity((Pos(5), Vel(15)));
    let (motion, tags) = world.borrow::<(MotionTuple, View<Tag>)>().unwrap();

    let items: Vec<_> = (&tags | &motion)
        .iter()
        .with_id()
        .filter_map(|(id, item)| match item {
            OneOfTwo::One(tag) if tag.0 != 0 => Some((id, tag.0)),
            OneOfTwo::One(_) => None,
            OneOfTwo::Two((pos, vel)) => Some((id, pos.0 + vel.0)),
        })
        .collect();

    assert_eq!(items, [(e[1], 21), (e[3], 23), (alone, 20)]);
}
