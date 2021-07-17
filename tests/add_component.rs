use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Nothing;
}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Nothing;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
        .unwrap();

    let entity0 = entities.add_entity(&mut usizes, USIZE(0));
    let entity1 = entities.add_entity(&mut u32s, U32(1));

    let entity10 = entities.add_entity((), ());
    let entity20 = entities.add_entity((), ());
    entities.add_component(entity10, (&mut usizes, &mut u32s), (USIZE(10), U32(30)));
    entities.add_component(entity20, &mut usizes, USIZE(20));
    entities.add_component(entity20, &mut u32s, U32(50));
    assert_eq!(usizes.get(entity0).unwrap(), &USIZE(0));
    assert_eq!(u32s.get(entity1).unwrap(), &U32(1));
    assert_eq!(
        (&usizes, &u32s).get(entity10).unwrap(),
        (&USIZE(10), &U32(30))
    );
    assert_eq!(
        (&usizes, &u32s).get(entity20).unwrap(),
        (&USIZE(20), &U32(50))
    );
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&USIZE(10), &U32(30))));
    assert_eq!(iter.next(), Some((&USIZE(20), &U32(50))));
    assert_eq!(iter.next(), None);
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();

    let entity = entities.add_entity((), ());

    entities.add_component(entity, &mut usizes, USIZE(1));

    let mut iter = usizes.inserted().iter();
    assert_eq!(iter.next(), Some(&USIZE(1)));
    assert_eq!(iter.next(), None);

    entities.add_component(entity, &mut usizes, USIZE(2));

    let mut iter = usizes.inserted().iter();
    assert_eq!(iter.next(), Some(&USIZE(2)));
    assert_eq!(iter.next(), None);

    usizes.clear_all_inserted();

    usizes[entity] = USIZE(3);

    entities.add_component(entity, &mut usizes, USIZE(4));

    let mut iter = usizes.modified().iter();
    assert_eq!(iter.next(), Some(&USIZE(4)));
    assert_eq!(iter.next(), None);

    usizes.clear_all_modified();

    entities.add_component(entity, &mut usizes, USIZE(5));

    let mut iter = usizes.modified().iter();
    assert_eq!(iter.next(), Some(&USIZE(5)));
    assert_eq!(iter.next(), None);
}

#[test]
fn no_pack_unchecked() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Nothing;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
        .unwrap();

    let entity1 = entities.add_entity((), ());
    (&mut usizes, &mut u32s).add_component_unchecked(entity1, (USIZE(0), U32(1)));
    (&mut u32s, &mut usizes).add_component_unchecked(entity1, (U32(3), USIZE(2)));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&USIZE(2), &U32(3)));
}
