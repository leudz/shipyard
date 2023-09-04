use shipyard::*;

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct U32(u32);
    impl Component for U32 {}

    let world = World::new();

    world.run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>| {
        let entity0 = entities.add_entity(&mut u32s, U32(0));
        let entity1 = entities.add_entity(&mut u32s, U32(1));

        u32s.apply_mut(entity0, entity1, core::mem::swap);

        assert_eq!(u32s[entity0], U32(1));
        assert_eq!(u32s[entity1], U32(0));
    });
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct U32(u32);
    impl Component for U32 {}

    let world = World::new();

    world.borrow::<ViewMut<U32>>().unwrap().track_all();

    let entity0 = world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32, track::All>| {
            let entity0 = entities.add_entity(&mut u32s, U32(0));

            u32s.clear_all_inserted();

            entity0
        },
    );

    world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32, track::All>| {
            let entity1 = entities.add_entity(&mut u32s, U32(1));

            u32s.apply_mut(entity0, entity1, core::mem::swap);

            assert_eq!(u32s[entity0], U32(1));
            assert_eq!(u32s[entity1], U32(0));

            let mut inserted = u32s.inserted().iter();
            assert_eq!(inserted.next(), Some(&U32(0)));
            assert_eq!(inserted.next(), None);

            let mut modified = u32s.modified().iter();
            assert_eq!(modified.next(), Some(&U32(1)));
            assert_eq!(modified.next(), Some(&U32(0)));
            assert_eq!(modified.next(), None);
        },
    );
}
