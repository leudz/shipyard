use shipyard::prelude::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    let entity1 = entities.add_entity((), ());
    entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
    entities.add_component((&mut u32s, &mut usizes), (3, 2), entity1);
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).tight_pack();
    let entity1 = entities.add_entity((), ());
    entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
    entities.add_component((&mut usizes, &mut u32s), (3usize,), entity1);
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&3, &1));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&3, &1)));
    assert_eq!(iter.next(), None);
}

#[test]
fn loose_add_component() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).loose_pack();
    let entity1 = entities.add_entity((), ());
    entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
    entities.add_component((&mut u32s, &mut usizes), (3, 2), entity1);
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&2, &3)));
    assert_eq!(iter.next(), None);
}

#[test]
fn tight_loose_add_component() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u64, &mut u32)>();

    (&mut usizes, &mut u64s).tight_pack();
    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));
    let entity1 = entities.add_entity((), ());
    entities.add_component((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2), entity1);
    entities.add_component((&mut u32s, &mut u64s, &mut usizes), (5, 4, 3), entity1);
    assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&3, &4, &5));
    let mut iter = (&usizes, &u32s, &u64s).iter();
    assert_eq!(iter.next(), Some((&3, &5, &4)));
    assert_eq!(iter.next(), None);
    let mut iter = (&usizes, &u64s).iter();
    assert_eq!(iter.next(), Some((&3, &4)));
    assert_eq!(iter.next(), None);
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();

    usizes.update_pack();
    let e0 = entities.add_entity((), ());
    let e1 = entities.add_entity((), ());
    let e2 = entities.add_entity((), ());

    entities.add_component(&mut usizes, 1, e0);
    entities.add_component(&mut usizes, 2, e1);
    entities.add_component(&mut usizes, 3, e2);

    let mut iter = usizes.inserted().iter();
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), Some(&3));
    assert_eq!(iter.next(), None);

    assert_eq!(usizes.inserted().len(), 3);
}

#[test]
fn not_enough_to_tightly_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s, mut f32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32, &mut f32)>();

    (&mut usizes, &mut u32s).tight_pack();
    entities.add_entity(&mut u32s, 0);
    let entity = entities.add_entity((), ());
    entities.add_component((&mut f32s, &mut u32s, &mut usizes), (1., 1), entity);

    let mut iter = (&u32s).iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    assert_eq!((&u32s, &f32s).get(entity), Ok((&1, &1.)));
}

#[test]
fn not_enough_to_loosely_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s, mut f32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32, &mut f32)>();

    (&mut usizes, &mut u32s).loose_pack();
    entities.add_entity(&mut u32s, 0);
    let entity = entities.add_entity((), ());
    entities.add_component((&mut f32s, &mut u32s, &mut usizes), (1., 1), entity);

    let mut iter = (&u32s).iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    assert_eq!((&u32s, &f32s).get(entity), Ok((&1, &1.)));
}
