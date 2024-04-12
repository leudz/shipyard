use shipyard::*;

#[derive(Debug, PartialEq, Eq)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    world.run(|mut entities: EntitiesViewMut| {
        entities.add_entity((), ());
    });
    world.run(
        |(mut entities, mut usizes, mut u64s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u64s), (USIZE(0), U64(1)));
            assert_eq!((&usizes, &u64s).get(entity1).unwrap(), (&USIZE(0), &U64(1)));
        },
    );
}

#[test]
fn update() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();

    let entity = entities.add_entity(&mut usizes, USIZE(0));
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], USIZE(0));
}

#[test]
fn cleared_update() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();

    let entity1 = entities.add_entity(&mut usizes, USIZE(1));
    usizes.clear_all_inserted_and_modified();

    let mut usizes = world.borrow::<ViewMut<USIZE>>().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 0);
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), USIZE(1));
    assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
}

#[test]
fn modified_update() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();

    let entity1 = entities.add_entity(&mut usizes, USIZE(1));
    usizes.clear_all_inserted_and_modified();

    let mut usizes = world.borrow::<ViewMut<USIZE>>().unwrap();
    usizes[entity1] = USIZE(3);
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), USIZE(3));
    assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
}

#[test]
fn bulk() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut usizes, mut u64s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)>()
        .unwrap();

    entities.bulk_add_entity((), (0..1).map(|_| {}));
    let mut new_entities = entities
        .bulk_add_entity(
            (&mut usizes, &mut u64s),
            (0..2).map(|i| (USIZE(i as usize), U64(i))),
        )
        .collect::<Vec<_>>()
        .into_iter();

    let mut iter = (&usizes, &u64s).iter().ids();
    assert_eq!(new_entities.next(), iter.next());
    assert_eq!(new_entities.next(), iter.next());
    assert_eq!(new_entities.next(), None);

    entities
        .bulk_add_entity(
            (&mut usizes, &mut u64s),
            (0..2).map(|i| (USIZE(i as usize), U64(i))),
        )
        .collect::<Vec<_>>()
        .into_iter();

    assert_eq!(usizes.len(), 4);
}

#[test]
fn bulk_unequal_length() {
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_entity((U64(0),));

    let entity = world
        .bulk_add_entity((0..1).map(|_| (U64(1), USIZE(2))))
        .next()
        .unwrap();

    world.delete_entity(entity);
}
