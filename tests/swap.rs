use shipyard::*;

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct U64(u64);
    impl Component for U64 {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.run(|mut entities: EntitiesViewMut, mut u64s: ViewMut<U64>| {
        let entity0 = entities.add_entity(&mut u64s, U64(0));
        let entity1 = entities.add_entity(&mut u64s, U64(1));

        u64s.apply_mut(entity0, entity1, core::mem::swap);

        assert_eq!(u64s[entity0], U64(1));
        assert_eq!(u64s[entity1], U64(0));
    });
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct U64(u64);
    impl Component for U64 {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity0 = world.run(|mut entities: EntitiesViewMut, mut u64s: ViewMut<U64>| {
        let entity0 = entities.add_entity(&mut u64s, U64(0));

        u64s.clear_all_inserted();

        entity0
    });

    world.run(|mut entities: EntitiesViewMut, mut u64s: ViewMut<U64>| {
        let entity1 = entities.add_entity(&mut u64s, U64(1));

        u64s.apply_mut(entity0, entity1, core::mem::swap);

        assert_eq!(u64s[entity0], U64(1));
        assert_eq!(u64s[entity1], U64(0));

        let mut inserted = u64s.inserted().iter();
        assert_eq!(inserted.next(), Some(&U64(0)));
        assert_eq!(inserted.next(), None);

        let mut modified = u64s.modified().iter();
        assert_eq!(modified.next(), Some(&U64(1)));
        assert_eq!(modified.next(), Some(&U64(0)));
        assert_eq!(modified.next(), None);
    });
}
