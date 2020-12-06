use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity0 = entities.add_entity(&mut usizes, 0);
    let entity1 = entities.add_entity(&mut u32s, 1);

    let entity10 = entities.add_entity((), ());
    let entity20 = entities.add_entity((), ());
    entities
        .try_add_component(entity10, (&mut usizes, &mut u32s), (10, 30))
        .unwrap();
    entities
        .try_add_component(entity20, &mut usizes, 20)
        .unwrap();
    entities.try_add_component(entity20, &mut u32s, 50).unwrap();
    assert_eq!(usizes.get(entity0).unwrap(), &0);
    assert_eq!(u32s.get(entity1).unwrap(), &1);
    assert_eq!((&usizes, &u32s).get(entity10).unwrap(), (&10, &30));
    assert_eq!((&usizes, &u32s).get(entity20).unwrap(), (&20, &50));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&10, &30)));
    assert_eq!(iter.next(), Some((&20, &50)));
    assert_eq!(iter.next(), None);
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();

    usizes.update_pack();
    let entity = entities.add_entity((), ());

    entities.try_add_component(entity, &mut usizes, 1).unwrap();

    let mut iter = usizes.inserted().iter();
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    entities.try_add_component(entity, &mut usizes, 2).unwrap();

    let mut iter = usizes.inserted().iter();
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), None);

    usizes.clear_inserted();

    usizes[entity] = 3;

    entities.try_add_component(entity, &mut usizes, 4).unwrap();

    let mut iter = usizes.modified().iter();
    assert_eq!(iter.next(), Some(&4));
    assert_eq!(iter.next(), None);

    usizes.clear_modified();

    entities.try_add_component(entity, &mut usizes, 5).unwrap();

    let mut iter = usizes.modified().iter();
    assert_eq!(iter.next(), Some(&5));
    assert_eq!(iter.next(), None);
}

#[test]
fn no_pack_unchecked() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity1 = entities.add_entity((), ());
    (&mut usizes, &mut u32s).add_component_unchecked(entity1, (0, 1));
    (&mut u32s, &mut usizes).add_component_unchecked(entity1, (3, 2));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
}
