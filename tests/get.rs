use shipyard::*;

#[test]
fn non_packed() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();
    let entity0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    let entity1 = entities.add_entity(&mut u32s, 1);
    let entity2 = entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    let entity3 = entities.add_entity(&mut i16s, 13);
    let entity4 = entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    assert_eq!(u32s.get(entity0), Ok(&0));
    assert_eq!(u32s.get(entity1), Ok(&1));
    assert_eq!(u32s.get(entity2), Ok(&2));
    assert!(u32s.get(entity3).is_err());
    assert_eq!(u32s.get(entity4), Ok(&4));

    assert_eq!(i16s.get(entity0), Ok(&10));
    assert!(i16s.get(entity1).is_err());
    assert_eq!(i16s.get(entity2), Ok(&12));
    assert_eq!(i16s.get(entity3), Ok(&13));
    assert_eq!(i16s.get(entity4), Ok(&14));

    assert_eq!((&u32s, &i16s).get(entity0), Ok((&0, &10)));
    assert!((&u32s, &i16s).get(entity1).is_err());
    assert_eq!((&u32s, &i16s).get(entity2), Ok((&2, &12)));
    assert!((&u32s, &i16s).get(entity3).is_err());
    assert_eq!((&u32s, &i16s).get(entity4), Ok((&4, &14)));
}

#[test]
fn update() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();
    u32s.update_pack();
    i16s.update_pack();
    let entity0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    let entity1 = entities.add_entity(&mut u32s, 1);
    let entity2 = entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    let entity3 = entities.add_entity(&mut i16s, 13);
    let entity4 = entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    assert_eq!(u32s.get(entity0), Ok(&0));
    assert_eq!(u32s.get(entity1), Ok(&1));
    assert_eq!(u32s.get(entity2), Ok(&2));
    assert!(u32s.get(entity3).is_err());
    assert_eq!(u32s.get(entity4), Ok(&4));

    assert_eq!(i16s.get(entity0), Ok(&10));
    assert!(i16s.get(entity1).is_err());
    assert_eq!(i16s.get(entity2), Ok(&12));
    assert_eq!(i16s.get(entity3), Ok(&13));
    assert_eq!(i16s.get(entity4), Ok(&14));

    assert_eq!((&u32s, &i16s).get(entity0), Ok((&0, &10)));
    assert!((&u32s, &i16s).get(entity1).is_err());
    assert_eq!((&u32s, &i16s).get(entity2), Ok((&2, &12)));
    assert!((&u32s, &i16s).get(entity3).is_err());
    assert_eq!((&u32s, &i16s).get(entity4), Ok((&4, &14)));
}

#[test]
fn old_id() {
    let world = World::new();

    world
        .run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let entity = entities.add_entity(&mut u32s, 0);

            entities.delete_unchecked(entity);

            let entity1 = entities.add_entity((), ());

            assert!(u32s.get(entity1).is_err());
        })
        .unwrap();
}
