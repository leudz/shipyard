use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity0 = world.add_entity((USIZE(0usize),));
    let entity1 = world.add_entity((U32(1),));

    let entity10 = world.add_entity(());
    let entity20 = world.add_entity(());

    world.add_component(entity10, (USIZE(10), U32(30)));
    world.add_component(entity20, (USIZE(20),));
    world.add_component(entity20, (U32(50),));

    let (usizes, u32s) = world.borrow::<(View<USIZE>, View<U32>)>().unwrap();

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
    impl Component for USIZE {}

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    let entity = world.add_entity(());

    world.add_component(entity, (USIZE(1usize),));

    world.run(|usizes: View<USIZE, { track::All }>| {
        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), Some(&USIZE(1)));
        assert_eq!(iter.next(), None);
    });

    world.add_component(entity, (USIZE(2usize),));

    world.run(|usizes: ViewMut<USIZE, { track::All }>| {
        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), Some(&USIZE(2)));
        assert_eq!(iter.next(), None);

        usizes.clear_all_inserted();
    });

    world.add_component(entity, (USIZE(4usize),));

    world.run(|usizes: ViewMut<USIZE, { track::All }>| {
        let mut iter = usizes.modified().iter();
        assert_eq!(iter.next(), Some(&USIZE(4)));
        assert_eq!(iter.next(), None);

        usizes.clear_all_modified();
    });

    world.add_component(entity, (USIZE(5usize),));

    world.run(|usizes: View<USIZE, { track::All }>| {
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
    impl Component for USIZE {}

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity(());
    world.delete_entity(entity);
    world.add_component(entity, (U32(1u32),));

    let u32s = world.borrow::<View<U32>>().unwrap();
    assert!(u32s.get(entity).is_err());
}
