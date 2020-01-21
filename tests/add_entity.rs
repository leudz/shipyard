use shipyard::prelude::*;

#[test]
fn no_pack() {
    let world = World::default();
    world.run::<EntitiesMut, _, _>(|mut entities| {
        entities.add_entity((), ());
    });
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
        },
    );
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).tight_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
}

#[test]
fn loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).loose_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
}

#[test]
fn tight_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u64, &mut u32)>();

    (&mut usizes, &mut u64s).tight_pack();
    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&0, &1, &2));
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    let entity = entities.add_entity(&mut usizes, 0);
    assert_eq!(usizes.inserted().len(), 1);
    assert_eq!(usizes[entity], 0);
}
