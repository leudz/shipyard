use shipyard::*;

#[test]
fn type_check() {
    #[derive(Debug, PartialEq)]
    struct Life(f32);
    impl Component for Life {
        type Tracking = track::Modification;
    }

    #[derive(Debug, PartialEq)]
    struct Energy(f32);
    impl Component for Energy {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();
    let mut vm_life = world.borrow::<ViewMut<Life>>().unwrap();

    let mut vm_energy = world.borrow::<ViewMut<Energy>>().unwrap();

    let entity = entities.add_entity((&mut vm_life, &mut vm_energy), (Life(0.), Energy(0.)));

    let life: Mut<Life> = (&mut vm_life).get(entity).unwrap();
    let energy: &mut Energy = (&mut vm_energy).get(entity).unwrap();

    assert_eq!(*life, Life(0.));
    assert_eq!(*energy, Energy(0.));
}

#[test]
fn non_packed() {
    #[derive(PartialEq, Eq, Debug)]
    struct U64(u64);
    impl Component for U64 {
        type Tracking = track::Untracked;
    }

    #[derive(PartialEq, Eq, Debug)]
    struct I16(i16);
    impl Component for I16 {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();
    let entity0 = entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    let entity1 = entities.add_entity(&mut u64s, U64(1));
    let entity2 = entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    let entity3 = entities.add_entity(&mut i16s, I16(13));
    let entity4 = entities.add_entity((&mut u64s, &mut i16s), (U64(4), I16(14)));

    assert_eq!(u64s.get(entity0), Ok(&U64(0)));
    assert_eq!(u64s.get(entity1), Ok(&U64(1)));
    assert_eq!(u64s.get(entity2), Ok(&U64(2)));
    assert!(u64s.get(entity3).is_err());
    assert_eq!(u64s.get(entity4), Ok(&U64(4)));

    assert_eq!(i16s.get(entity0), Ok(&I16(10)));
    assert!(i16s.get(entity1).is_err());
    assert_eq!(i16s.get(entity2), Ok(&I16(12)));
    assert_eq!(i16s.get(entity3), Ok(&I16(13)));
    assert_eq!(i16s.get(entity4), Ok(&I16(14)));

    assert_eq!((&u64s, &i16s).get(entity0), Ok((&U64(0), &I16(10))));
    assert!((&u64s, &i16s).get(entity1).is_err());
    assert_eq!((&u64s, &i16s).get(entity2), Ok((&U64(2), &I16(12))));
    assert!((&u64s, &i16s).get(entity3).is_err());
    assert_eq!((&u64s, &i16s).get(entity4), Ok((&U64(4), &I16(14))));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct U64(u64);
    impl Component for U64 {
        type Tracking = track::All;
    }

    #[derive(PartialEq, Eq, Debug)]
    struct I16(i16);
    impl Component for I16 {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    let entity0 = entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    let entity1 = entities.add_entity(&mut u64s, U64(1));
    let entity2 = entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    let entity3 = entities.add_entity(&mut i16s, I16(13));
    let entity4 = entities.add_entity((&mut u64s, &mut i16s), (U64(4), I16(14)));

    assert_eq!(u64s.get(entity0), Ok(&U64(0)));
    assert_eq!(u64s.get(entity1), Ok(&U64(1)));
    assert_eq!(u64s.get(entity2), Ok(&U64(2)));
    assert!(u64s.get(entity3).is_err());
    assert_eq!(u64s.get(entity4), Ok(&U64(4)));

    assert_eq!(i16s.get(entity0), Ok(&I16(10)));
    assert!(i16s.get(entity1).is_err());
    assert_eq!(i16s.get(entity2), Ok(&I16(12)));
    assert_eq!(i16s.get(entity3), Ok(&I16(13)));
    assert_eq!(i16s.get(entity4), Ok(&I16(14)));

    assert_eq!((&u64s, &i16s).get(entity0), Ok((&U64(0), &I16(10))));
    assert!((&u64s, &i16s).get(entity1).is_err());
    assert_eq!((&u64s, &i16s).get(entity2), Ok((&U64(2), &I16(12))));
    assert!((&u64s, &i16s).get(entity3).is_err());
    assert_eq!((&u64s, &i16s).get(entity4), Ok((&U64(4), &I16(14))));
}

#[test]
fn old_id() {
    struct U64(u64);
    impl Component for U64 {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.run(|mut entities: EntitiesViewMut, mut u64s: ViewMut<U64>| {
        let entity = entities.add_entity(&mut u64s, U64(0));

        entities.delete_unchecked(entity);

        let entity1 = entities.add_entity((), ());

        assert!(u64s.get(entity1).is_err());
    });
}
