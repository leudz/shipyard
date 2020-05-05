use shipyard::*;

#[test]
fn no_pack() {
    let world = World::default();
    world.run(|mut entities: EntitiesViewMut| {
        entities.add_entity((), ());
    });
    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
        },
    );
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    (&mut usizes, &mut u32s).tight_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
}

#[test]
fn loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    (&mut usizes, &mut u32s).loose_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
}

#[test]
fn tight_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>();

    (&mut usizes, &mut u64s).tight_pack();
    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&0, &1, &2));
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>();
    usizes.update_pack();
    let entity = entities.add_entity(&mut usizes, 0);
    assert_eq!(usizes.inserted().len(), 1);
    assert_eq!(usizes[entity], 0);
}

#[test]
fn cleared_update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>();
    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 1);
    usizes.clear_inserted_and_modified();
    let entity2 = entities.add_entity(&mut usizes, 2);
    assert_eq!(usizes.inserted().len(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), 1);
    assert_eq!(*usizes.get(entity2).unwrap(), 2);
}

#[test]
fn modified_update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>();
    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 1);
    usizes.clear_inserted_and_modified();
    usizes[entity1] = 3;
    let entity2 = entities.add_entity(&mut usizes, 2);
    assert_eq!(usizes.inserted().len(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), 3);
    assert_eq!(*usizes.get(entity2).unwrap(), 2);
}

#[test]
fn not_all_tight() {
    let world = World::new();

    let (mut entities, mut u32s, mut u16s, mut f32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<u16>, ViewMut<f32>)>();

    (&mut u32s, &mut u16s).tight_pack();

    entities.add_entity((&mut u32s, &mut u16s, &mut f32s), (0, 0, 0.));
    entities.add_entity((&mut f32s, &mut u16s, &mut u32s), (0., 0, 0));

    assert!((&mut u32s, &mut u16s).iter().count() > 0);
}

#[test]
fn not_all_loose() {
    let world = World::new();

    let (mut entities, mut u32s, mut u16s, mut f32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<u16>, ViewMut<f32>)>();

    (&mut u32s, &mut u16s).loose_pack();

    entities.add_entity((&mut u32s, &mut u16s, &mut f32s), (0, 0, 0.));
    entities.add_entity((&mut f32s, &mut u16s, &mut u32s), (0., 0, 0));

    assert!((&mut u32s, &mut u16s).iter().count() > 0);
}
