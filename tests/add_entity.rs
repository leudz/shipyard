use shipyard::*;

#[test]
fn no_pack() {
    let world = World::default();
    world
        .run(|mut entities: EntitiesViewMut| {
            entities.add_entity((), ());
        })
        .unwrap();
    world
        .run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
                assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
            },
        )
        .unwrap();
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();
    usizes.update_pack();
    let entity = entities.add_entity(&mut usizes, 0);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], 0);
}

#[test]
fn cleared_update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();
    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 1);
    usizes.clear_inserted_and_modified();
    assert_eq!(usizes.inserted().iter().count(), 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), 1);
    assert_eq!(*usizes.get(entity2).unwrap(), 2);
}

#[test]
fn modified_update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();
    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 1);
    usizes.clear_inserted_and_modified();
    usizes[entity1] = 3;
    let entity2 = entities.add_entity(&mut usizes, 2);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), 3);
    assert_eq!(*usizes.get(entity2).unwrap(), 2);
}

#[test]
fn bulk() {
    let world = World::default();

    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    entities.bulk_add_entity((), (0..1).map(|_| {}));
    let mut new_entities = entities
        .bulk_add_entity((&mut usizes, &mut u32s), (0..2).map(|i| (i as usize, i)))
        .collect::<Vec<_>>()
        .into_iter();

    let mut iter = (&usizes, &u32s).iter().ids();
    assert_eq!(new_entities.next(), iter.next());
    assert_eq!(new_entities.next(), iter.next());
    assert_eq!(new_entities.next(), None);

    entities
        .bulk_add_entity((&mut usizes, &mut u32s), (0..2).map(|i| (i as usize, i)))
        .collect::<Vec<_>>()
        .into_iter();

    assert_eq!(usizes.len(), 4);
}
