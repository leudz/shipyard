use super::ways::every_way;
use shipyard::iter::{IntoShiperator, Mixed, OneOfTwo};
use shipyard::*;

#[derive(Component, Debug, PartialEq)]
struct Pos(u32);

#[derive(Component, Debug, PartialEq)]
#[track(Modification)]
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

/// Asserts that a named derived iterator forwards bulk consumption to its tuple:
/// `fold` and `WithId::for_each` skip the rejected candidates like `next` does.
#[test]
fn named_bulk() {
    let (world, e) = derive_world();
    let mut motion = world.borrow::<MotionView>().unwrap();

    let fold = (&mut motion).iter().fold(Vec::new(), |mut acc, motion| {
        acc.push((motion.pos.0, motion.vel.0));
        acc
    });
    assert_eq!(fold, [(1, 11), (3, 13)]);

    let mut with_ids = Vec::new();
    (&mut motion)
        .iter()
        .with_id()
        .for_each(|(id, motion)| with_ids.push((id, motion.pos.0, motion.vel.0)));
    assert_eq!(with_ids, [(e[1], 1, 11), (e[3], 3, 13)]);
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

/// `e0` to `e3` own `Pos(i)`, `e1` and `e3` also own `Vel(10 + i)`.
/// Only `e3` owns a `Tag`, `Tag(23)`. Six more entities only own `Vel(0)`.
///
/// `Pos` drives the views below, rejecting `e0` and `e2`. No `Vel` is flagged as modified.
fn tracked_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(Pos(i))).collect();
    for i in [1, 3] {
        world.add_component(e[i], Vel(10 + i as u32));
    }
    world.add_component(e[3], Tag(23));
    for _ in 0..6 {
        world.add_entity(Vel(0));
    }
    world.borrow::<ViewMut<Vel>>().unwrap().clear_all_modified();

    (world, e)
}

/// `_tag` is skipped, entities without a `Tag` are still yielded.
#[derive(Borrow, IntoIter)]
#[shipyard(item_name = "Renamed")]
struct RenamedView<'v> {
    #[shipyard(item_field_name = "position")]
    v_pos: View<'v, Pos>,
    vm_vel: ViewMut<'v, Vel>,
    #[shipyard(item_field_skip)]
    _tag: View<'v, Tag>,
}

/// Asserts that a named derived view yields its renamed fields in every way,
/// ignoring the skipped field's storage, and that reading its `Mut`s flags nothing.
#[test]
fn renamed_skipped() {
    let (world, e) = tracked_world();
    let mut renamed = world.borrow::<RenamedView>().unwrap();

    let items = every_way!((&mut renamed).iter(), |renamed: Renamed| (
        renamed.position.0,
        renamed.vel.0
    ));

    assert_eq!(items, [(e[1], (1, 11)), (e[3], (3, 13))]);
    assert!(!renamed.vm_vel.is_modified(e[1]));
    assert!(!renamed.vm_vel.is_modified(e[3]));
}

/// Asserts that writing through a named derived view's `Mut` in bulk flags only that component.
#[test]
fn named_mut() {
    let (world, e) = tracked_world();
    let mut renamed = world.borrow::<RenamedView>().unwrap();

    (&mut renamed)
        .iter()
        .with_id()
        .for_each(|(id, mut renamed)| {
            if id == e[3] {
                renamed.vel.0 += 100;
            }
        });

    assert!(!renamed.vm_vel.is_modified(e[1]));
    assert!(renamed.vm_vel.is_modified(e[3]));
    assert_eq!((&renamed.vm_vel).get(e[3]).unwrap().0, 113);
}

/// Asserts that a tuple derived view uses the tuple's own `Mixed`, not an iterator of its own.
#[test]
fn tuple_reuses_tuple() {
    let (world, _) = tracked_world();
    let motion = world.borrow::<MotionTuple>().unwrap();

    let (shiperator, _, _) = (&motion).into_shiperator();
    let _: Mixed<_> = shiperator;
}

/// Asserts that a tuple derived view yields the same items in every way.
#[test]
fn tuple_every_way() {
    let (world, e) = tracked_world();
    let motion = world.borrow::<MotionTuple>().unwrap();

    let items = every_way!((&motion).iter(), |(pos, vel): (&Pos, &Vel)| (pos.0, vel.0));

    assert_eq!(items, [(e[1], (1, 11)), (e[3], (3, 13))]);
}

/// Asserts that a mutable named derived view nested in a tuple yields the same items in
/// every way, with the outer storage rejecting `e1`.
#[test]
fn nested_mut() {
    let (world, e) = tracked_world();
    let (mut renamed, tags) = world.borrow::<(RenamedView, View<Tag>)>().unwrap();

    let items = every_way!((&mut renamed, &tags).iter(), |(renamed, tag): (
        Renamed,
        &Tag
    )| (
        renamed.position.0,
        renamed.vel.0,
        tag.0
    ));

    assert_eq!(items, [(e[3], (3, 13, 23))]);
}
