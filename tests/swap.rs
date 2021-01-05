use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();

    world
        .run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let entity0 = entities.add_entity(&mut u32s, 0);
            let entity1 = entities.add_entity(&mut u32s, 1);

            u32s.apply_mut(entity0, entity1, core::mem::swap);

            assert_eq!(u32s[entity0], 1);
            assert_eq!(u32s[entity1], 0);
        })
        .unwrap();
}

#[test]
fn update() {
    let world = World::new();

    world
        .run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            u32s.update_pack();

            let entity0 = entities.add_entity(&mut u32s, 0);

            u32s.clear_all_inserted();

            let entity1 = entities.add_entity(&mut u32s, 1);

            u32s.apply_mut(entity0, entity1, core::mem::swap);

            assert_eq!(u32s[entity0], 1);
            assert_eq!(u32s[entity1], 0);

            let mut inserted = u32s.inserted().iter();
            assert_eq!(inserted.next(), Some(&0));
            assert_eq!(inserted.next(), None);

            let mut modified = u32s.modified().iter();
            assert_eq!(modified.next(), Some(&1));
            assert_eq!(modified.next(), None);
        })
        .unwrap();
}
