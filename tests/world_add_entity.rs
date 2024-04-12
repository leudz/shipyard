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

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_entity(());
    let entity1 = world.add_entity((USIZE(0), U64(1)));

    let (usizes, u64s) = world.borrow::<(View<USIZE>, View<U64>)>().unwrap();
    assert_eq!((&usizes, &u64s).get(entity1), Ok((&USIZE(0), &U64(1))));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity((USIZE(0),));

    let usizes = world.borrow::<View<USIZE>>().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], USIZE(0));
}

#[test]
fn cleared_update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity1 = world.add_entity((USIZE(1usize),));

    world.run(|usizes: ViewMut<USIZE>| {
        usizes.clear_all_inserted_and_modified();
    });

    world.run(|usizes: View<USIZE>| {
        assert_eq!(usizes.inserted().iter().count(), 0);
    });

    let entity2 = world.add_entity((USIZE(2usize),));

    world.run(|usizes: View<USIZE>| {
        assert_eq!(usizes.inserted().iter().count(), 1);
        assert_eq!(*usizes.get(entity1).unwrap(), USIZE(1));
        assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
    });
}

#[test]
fn modified_update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity1 = world.add_entity((USIZE(1),));

    world.run(|usizes: ViewMut<USIZE>| {
        usizes.clear_all_inserted_and_modified();
    });

    let entity2 = world.add_entity((USIZE(2usize),));

    world.run(|mut usizes: ViewMut<USIZE>| {
        usizes[entity1] = USIZE(3);
        assert_eq!(usizes.inserted().iter().count(), 1);
        assert_eq!(*usizes.get(entity1).unwrap(), USIZE(3));
        assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
    });
}

#[test]
fn bulk_single() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (U64(i),)))
        .collect::<Vec<_>>();

    let u64s = world.borrow::<View<U64>>().unwrap();
    let mut iter = u64s.iter();
    assert_eq!(iter.next(), Some(&U64(0)));
    assert_eq!(iter.next(), Some(&U64(1)));
    assert_eq!(iter.next(), Some(&U64(2)));
    assert_eq!(iter.next(), Some(&U64(3)));
    assert_eq!(iter.next(), Some(&U64(4)));
    assert_eq!(iter.next(), None);

    let mut iter = u64s.iter().ids().zip(entities);
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next(), None);
}

#[test]
fn bulk() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (U64(i), USIZE(i as usize))))
        .collect::<Vec<_>>();

    let (u64s, usizes) = world.borrow::<(View<U64>, View<USIZE>)>().unwrap();
    let mut iter = (&u64s, &usizes).iter();
    assert_eq!(iter.next(), Some((&U64(0), &USIZE(0))));
    assert_eq!(iter.next(), Some((&U64(1), &USIZE(1))));
    assert_eq!(iter.next(), Some((&U64(2), &USIZE(2))));
    assert_eq!(iter.next(), Some((&U64(3), &USIZE(3))));
    assert_eq!(iter.next(), Some((&U64(4), &USIZE(4))));
    assert_eq!(iter.next(), None);

    let mut iter = u64s.iter().ids().zip(entities.clone());
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next(), None);

    let mut iter = usizes.iter().ids().zip(entities);
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next(), None);

    drop((u64s, usizes));

    world.bulk_add_entity((0..5).map(|i| (U64(i), USIZE(i as usize))));

    let (u64s, usizes) = world.borrow::<(View<U64>, View<USIZE>)>().unwrap();
    assert_eq!(u64s.len(), 10);
    assert_eq!(usizes.len(), 10);
}
