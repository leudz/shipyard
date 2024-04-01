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

    let entity0 = world.add_entity((USIZE(0usize),));
    let entity1 = world.add_entity((U64(1),));

    let entity10 = world.add_entity(());
    let entity20 = world.add_entity(());

    world.add_component(entity10, (USIZE(10), U64(30)));
    world.add_component(entity20, (USIZE(20),));
    world.add_component(entity20, (U64(50),));

    let (usizes, u64s) = world.borrow::<(View<USIZE>, View<U64>)>().unwrap();

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

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity(());

    world.add_component(entity, (USIZE(1usize),));

    world.run(|usizes: View<USIZE>| {
        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), Some(&USIZE(1)));
        assert_eq!(iter.next(), None);
    });

    world.add_component(entity, (USIZE(2usize),));

    world.run(|usizes: ViewMut<USIZE>| {
        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), Some(&USIZE(2)));
        assert_eq!(iter.next(), None);

        usizes.clear_all_inserted();
    });

    world.add_component(entity, (USIZE(4usize),));

    world.run(|usizes: ViewMut<USIZE>| {
        let mut iter = usizes.modified().iter();
        assert_eq!(iter.next(), Some(&USIZE(4)));
        assert_eq!(iter.next(), None);

        usizes.clear_all_modified();
    });

    world.add_component(entity, (USIZE(5usize),));

    world.run(|usizes: View<USIZE>| {
        let mut iter = usizes.modified().iter();
        assert_eq!(iter.next(), Some(&USIZE(5)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
#[should_panic(expected = "Entity has to be alive to add component to it.")]
fn dead_entity() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity(());
    world.delete_entity(entity);
    world.add_component(entity, (U64(1u64),));

    let u64s = world.borrow::<View<U64>>().unwrap();
    assert!(u64s.get(entity).is_err());
}
