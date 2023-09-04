use shipyard::*;

#[test]
fn type_check() {
    #[derive(Debug, PartialEq)]
    struct Life(f32);
    impl Component for Life {}

    #[derive(Debug, PartialEq)]
    struct Energy(f32);
    impl Component for Energy {}

    let world = World::new();

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();
    let mut vm_life = world.borrow::<ViewMut<Life>>().unwrap();

    let mut vm_energy = world.borrow::<ViewMut<Energy>>().unwrap();

    let entity = entities.add_entity((&mut vm_life, &mut vm_energy), (Life(0.), Energy(0.)));

    let life: Mut<Life> = (&mut vm_life).get(entity).unwrap();
    let energy: Mut<Energy> = (&mut vm_energy).get(entity).unwrap();

    assert_eq!(*life, Life(0.));
    assert_eq!(*energy, Energy(0.));
}

#[test]
fn non_packed() {
    #[derive(PartialEq, Eq, Debug)]
    struct U32(u32);
    impl Component for U32 {}

    #[derive(PartialEq, Eq, Debug)]
    struct I16(i16);
    impl Component for I16 {}

    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32>, ViewMut<I16>)>()
        .unwrap();
    let entity0 = entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    let entity1 = entities.add_entity(&mut u32s, U32(1));
    let entity2 = entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    let entity3 = entities.add_entity(&mut i16s, I16(13));
    let entity4 = entities.add_entity((&mut u32s, &mut i16s), (U32(4), I16(14)));

    assert_eq!(u32s.get(entity0), Ok(&U32(0)));
    assert_eq!(u32s.get(entity1), Ok(&U32(1)));
    assert_eq!(u32s.get(entity2), Ok(&U32(2)));
    assert!(u32s.get(entity3).is_err());
    assert_eq!(u32s.get(entity4), Ok(&U32(4)));

    assert_eq!(i16s.get(entity0), Ok(&I16(10)));
    assert!(i16s.get(entity1).is_err());
    assert_eq!(i16s.get(entity2), Ok(&I16(12)));
    assert_eq!(i16s.get(entity3), Ok(&I16(13)));
    assert_eq!(i16s.get(entity4), Ok(&I16(14)));

    assert_eq!((&u32s, &i16s).get(entity0), Ok((&U32(0), &I16(10))));
    assert!((&u32s, &i16s).get(entity1).is_err());
    assert_eq!((&u32s, &i16s).get(entity2), Ok((&U32(2), &I16(12))));
    assert!((&u32s, &i16s).get(entity3).is_err());
    assert_eq!((&u32s, &i16s).get(entity4), Ok((&U32(4), &I16(14))));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct U32(u32);
    impl Component for U32 {}

    #[derive(PartialEq, Eq, Debug)]
    struct I16(i16);
    impl Component for I16 {}

    let mut world = World::new();
    world.track_all::<(U32, I16)>();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32>, ViewMut<I16>)>()
        .unwrap();

    let entity0 = entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    let entity1 = entities.add_entity(&mut u32s, U32(1));
    let entity2 = entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    let entity3 = entities.add_entity(&mut i16s, I16(13));
    let entity4 = entities.add_entity((&mut u32s, &mut i16s), (U32(4), I16(14)));

    assert_eq!(u32s.get(entity0), Ok(&U32(0)));
    assert_eq!(u32s.get(entity1), Ok(&U32(1)));
    assert_eq!(u32s.get(entity2), Ok(&U32(2)));
    assert!(u32s.get(entity3).is_err());
    assert_eq!(u32s.get(entity4), Ok(&U32(4)));

    assert_eq!(i16s.get(entity0), Ok(&I16(10)));
    assert!(i16s.get(entity1).is_err());
    assert_eq!(i16s.get(entity2), Ok(&I16(12)));
    assert_eq!(i16s.get(entity3), Ok(&I16(13)));
    assert_eq!(i16s.get(entity4), Ok(&I16(14)));

    assert_eq!((&u32s, &i16s).get(entity0), Ok((&U32(0), &I16(10))));
    assert!((&u32s, &i16s).get(entity1).is_err());
    assert_eq!((&u32s, &i16s).get(entity2), Ok((&U32(2), &I16(12))));
    assert!((&u32s, &i16s).get(entity3).is_err());
    assert_eq!((&u32s, &i16s).get(entity4), Ok((&U32(4), &I16(14))));
}
#[test]
fn old_id() {
    struct U32(u32);
    impl Component for U32 {}

    let world = World::new();

    world.run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>| {
        let entity = entities.add_entity(&mut u32s, U32(0));

        entities.delete_unchecked(entity);

        let entity1 = entities.add_entity((), ());

        assert!(u32s.get(entity1).is_err());
    });
}
