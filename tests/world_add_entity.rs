use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    world.add_entity(());
    let entity1 = world.add_entity((USIZE(0), U32(1)));

    let (usizes, u32s) = world.borrow::<(View<USIZE>, View<U32>)>().unwrap();
    assert_eq!((&usizes, &u32s).get(entity1), Ok((&USIZE(0), &U32(1))));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    let entity = world.add_entity((USIZE(0),));

    let usizes = world.borrow::<View<USIZE, track::All>>().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], USIZE(0));
}

#[test]
fn cleared_update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    let entity1 = world.add_entity((USIZE(1usize),));

    world.run(|usizes: ViewMut<USIZE, track::All>| {
        usizes.clear_all_inserted();
    });

    world.run(|usizes: View<USIZE, track::All>| {
        assert_eq!(usizes.inserted().iter().count(), 0);
    });

    let entity2 = world.add_entity((USIZE(2usize),));

    world.run(|usizes: View<USIZE, track::All>| {
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
        type Tracking = track::Untracked;
    }

    let mut world = World::new();
    world.track_all::<USIZE>();

    let entity1 = world.add_entity((USIZE(1),));

    world.run(|usizes: ViewMut<USIZE, track::All>| {
        usizes.clear_all_inserted_and_modified();
    });

    let entity2 = world.add_entity((USIZE(2usize),));

    world.run(|mut usizes: ViewMut<USIZE, track::All>| {
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

    let mut world = World::new();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (U32(i),)))
        .collect::<Vec<_>>();

    let u32s = world.borrow::<View<U32>>().unwrap();
    let mut iter = u32s.iter();
    assert_eq!(iter.next(), Some(&U32(0)));
    assert_eq!(iter.next(), Some(&U32(1)));
    assert_eq!(iter.next(), Some(&U32(2)));
    assert_eq!(iter.next(), Some(&U32(3)));
    assert_eq!(iter.next(), Some(&U32(4)));
    assert_eq!(iter.next(), None);

    let mut iter = u32s.iter().ids().zip(entities);
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

    let mut world = World::new();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (U32(i), USIZE(i as usize))))
        .collect::<Vec<_>>();

    let (u32s, usizes) = world.borrow::<(View<U32>, View<USIZE>)>().unwrap();
    let mut iter = (&u32s, &usizes).iter();
    assert_eq!(iter.next(), Some((&U32(0), &USIZE(0))));
    assert_eq!(iter.next(), Some((&U32(1), &USIZE(1))));
    assert_eq!(iter.next(), Some((&U32(2), &USIZE(2))));
    assert_eq!(iter.next(), Some((&U32(3), &USIZE(3))));
    assert_eq!(iter.next(), Some((&U32(4), &USIZE(4))));
    assert_eq!(iter.next(), None);

    let mut iter = u32s.iter().ids().zip(entities.clone());
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

    drop((u32s, usizes));

    world.bulk_add_entity((0..5).map(|i| (U32(i), USIZE(i as usize))));

    let (u32s, usizes) = world.borrow::<(View<U32>, View<USIZE>)>().unwrap();
    assert_eq!(u32s.len(), 10);
    assert_eq!(usizes.len(), 10);
}
