use shipyard::prelude::*;

#[test]
fn non_packed() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();
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
fn tight() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();
    (&mut u32s, &mut i16s).tight_pack();
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
fn loose() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();
    (&mut u32s, &mut i16s).loose_pack();
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

    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();
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
fn off_by_one() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesMut, &mut usize)>();
    let entity0 = entities.add_entity(&mut u32s, 0);
    let entity1 = entities.add_entity(&mut u32s, 1);
    let entity2 = entities.add_entity(&mut u32s, 2);

    let window = u32s.as_window(1..);
    assert_eq!(window.len(), 2);
    assert!(window.get(entity0).is_err());
    assert_eq!(window.get(entity1).ok(), Some(&1));
    assert_eq!(window.get(entity2).ok(), Some(&2));
    let window = window.as_window(1..);
    assert_eq!(window.len(), 1);
    assert!(window.get(entity0).is_err());
    assert!(window.get(entity1).is_err());
    assert_eq!(window.get(entity2).ok(), Some(&2));

    let mut window = u32s.as_window_mut(1..);
    assert_eq!(window.len(), 2);
    assert!(window.get(entity0).is_err());
    assert_eq!((&mut window).get(entity1).ok(), Some(&mut 1));
    assert_eq!((&mut window).get(entity2).ok(), Some(&mut 2));
    let mut window = window.as_window_mut(1..);
    assert_eq!(window.len(), 1);
    assert!(window.get(entity0).is_err());
    assert!(window.get(entity1).is_err());
    assert_eq!((&mut window).get(entity2).ok(), Some(&mut 2));
}
