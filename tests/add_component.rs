use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes, mut u64s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)>()
        .unwrap();

    let entity0 = entities.add_entity(&mut usizes, USIZE(0));
    let entity1 = entities.add_entity(&mut u64s, U64(1));

    let entity10 = entities.add_entity((), ());
    let entity20 = entities.add_entity((), ());
    entities.add_component(entity10, (&mut usizes, &mut u64s), (USIZE(10), U64(30)));
    entities.add_component(entity20, &mut usizes, USIZE(20));
    entities.add_component(entity20, &mut u64s, U64(50));
    assert_eq!(usizes.get(entity0).unwrap(), &USIZE(0));
    assert_eq!(u64s.get(entity1).unwrap(), &U64(1));
    assert_eq!(
        (&usizes, &u64s).get(entity10).unwrap(),
        (&USIZE(10), &U64(30))
    );
    assert_eq!(
        (&usizes, &u64s).get(entity20).unwrap(),
        (&USIZE(20), &U64(50))
    );
    let mut iter = (&usizes, &u64s).iter();
    assert_eq!(iter.next(), Some((&USIZE(10), &U64(30))));
    assert_eq!(iter.next(), Some((&USIZE(20), &U64(50))));
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
    let mut usizes = world.borrow::<ViewMut<USIZE>>().unwrap();

    usizes[entity] = USIZE(3);

    entities.add_component(entity, &mut usizes, USIZE(4));

    let mut iter = usizes.modified().iter();
    assert_eq!(iter.next(), Some(&USIZE(4)));
    assert_eq!(iter.next(), None);

    usizes.clear_all_modified();

    let mut usizes = world.borrow::<ViewMut<USIZE>>().unwrap();

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
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes, mut u64s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)>()
        .unwrap();

    let entity1 = entities.add_entity((), ());
    (&mut usizes, &mut u64s).add_component_unchecked(entity1, (USIZE(0), U64(1)));
    (&mut u64s, &mut usizes).add_component_unchecked(entity1, (U64(3), USIZE(2)));
    assert_eq!((&usizes, &u64s).get(entity1).unwrap(), (&USIZE(2), &U64(3)));
}
